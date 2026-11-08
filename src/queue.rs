use crate::downloader::DownloadControl;
use std::sync::{Arc, Condvar, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskState {
    Queued,
    Downloading,
    Paused,
    Exporting,
}

enum Phase {
    Pending,
    Running(Arc<DownloadControl>),
    Exporting,
    Finished,
}

struct Task {
    id: String,
    paused: bool,
    phase: Phase,
}

#[derive(Default)]
struct QueueState {
    tasks: Vec<Task>,
    cancelled: bool,
}

pub struct QueueTask {
    pub id: String,
    pub control: Arc<DownloadControl>,
}

/// One book transfers at a time. Pausing cancels that attempt, retaining its
/// partial files, so another book can run without leaving a connection parked.
#[derive(Default)]
pub struct DownloadQueue {
    state: Mutex<QueueState>,
    changed: Condvar,
}

impl DownloadQueue {
    pub fn new(ids: impl IntoIterator<Item = String>) -> Self {
        Self {
            state: Mutex::new(QueueState {
                tasks: ids
                    .into_iter()
                    .map(|id| Task {
                        id,
                        paused: false,
                        phase: Phase::Pending,
                    })
                    .collect(),
                cancelled: false,
            }),
            changed: Condvar::new(),
        }
    }

    pub fn next_task(&self) -> Option<QueueTask> {
        let mut state = self.state.lock().expect("download queue poisoned");
        loop {
            if state.cancelled {
                return None;
            }
            if let Some(task) = Self::start_next(&mut state) {
                return Some(task);
            }
            if state
                .tasks
                .iter()
                .all(|task| matches!(task.phase, Phase::Finished))
            {
                return None;
            }
            state = self.changed.wait(state).expect("download queue poisoned");
        }
    }

    fn start_next(state: &mut QueueState) -> Option<QueueTask> {
        if state.cancelled
            || state
                .tasks
                .iter()
                .any(|task| matches!(task.phase, Phase::Running(_) | Phase::Exporting))
        {
            return None;
        }
        let task = state
            .tasks
            .iter_mut()
            .find(|task| !task.paused && matches!(task.phase, Phase::Pending))?;
        let control = Arc::new(DownloadControl::default());
        task.phase = Phase::Running(control.clone());
        Some(QueueTask {
            id: task.id.clone(),
            control,
        })
    }

    pub fn try_next_task(&self) -> Option<QueueTask> {
        Self::start_next(&mut self.state.lock().expect("download queue poisoned"))
    }

    pub fn task_state(&self, id: &str) -> Option<TaskState> {
        let state = self.state.lock().expect("download queue poisoned");
        let task = state.tasks.iter().find(|task| task.id == id)?;
        match &task.phase {
            Phase::Finished => None,
            Phase::Exporting => Some(TaskState::Exporting),
            _ if task.paused => Some(TaskState::Paused),
            Phase::Running(control) if !control.is_cancelled() => Some(TaskState::Downloading),
            _ => Some(TaskState::Queued),
        }
    }

    pub fn set_paused(&self, id: Option<&str>, paused: bool) {
        let mut state = self.state.lock().expect("download queue poisoned");
        for task in &mut state.tasks {
            if id.is_some_and(|id| task.id != id)
                || matches!(task.phase, Phase::Exporting | Phase::Finished)
            {
                continue;
            }
            task.paused = paused;
            if paused && let Phase::Running(control) = &task.phase {
                control.cancel();
            }
        }
        self.changed.notify_all();
    }

    pub fn has_paused(&self) -> bool {
        self.state
            .lock()
            .expect("download queue poisoned")
            .tasks
            .iter()
            .any(|task| task.paused && matches!(task.phase, Phase::Pending | Phase::Running(_)))
    }

    pub fn has_unpaused(&self) -> bool {
        self.state
            .lock()
            .expect("download queue poisoned")
            .tasks
            .iter()
            .any(|task| !task.paused && matches!(task.phase, Phase::Pending | Phase::Running(_)))
    }

    /// Commit the transfer before export. Pauses cannot interrupt an export
    /// or remove its encrypted input while the exporter is reading it.
    pub fn begin_export(&self, attempt: &QueueTask) -> bool {
        let mut state = self.state.lock().expect("download queue poisoned");
        if state.cancelled || attempt.control.is_cancelled() {
            return false;
        }
        let task = state
            .tasks
            .iter_mut()
            .find(|task| task.id == attempt.id)
            .expect("queued task");
        task.phase = Phase::Exporting;
        true
    }

    pub fn requeue(&self, id: &str) {
        let mut state = self.state.lock().expect("download queue poisoned");
        if let Some(task) = state.tasks.iter_mut().find(|task| task.id == id) {
            task.phase = Phase::Pending;
        }
        self.changed.notify_all();
    }

    pub fn finish(&self, id: &str) {
        let mut state = self.state.lock().expect("download queue poisoned");
        if let Some(task) = state.tasks.iter_mut().find(|task| task.id == id) {
            task.phase = Phase::Finished;
        }
        self.changed.notify_all();
    }

    pub fn cancel(&self) {
        let mut state = self.state.lock().expect("download queue poisoned");
        state.cancelled = true;
        for task in &state.tasks {
            if let Phase::Running(control) = &task.phase {
                control.cancel();
            }
        }
        self.changed.notify_all();
    }

    pub fn is_cancelled(&self) -> bool {
        self.state
            .lock()
            .expect("download queue poisoned")
            .cancelled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn pausing_one_book_allows_the_next_and_resumes_with_a_fresh_attempt() {
        let queue = DownloadQueue::new(["a".into(), "b".into(), "c".into()]);
        queue.set_paused(Some("b"), true);
        let first = queue.next_task().unwrap();
        assert_eq!(first.id, "a");
        queue.set_paused(Some("a"), true);
        assert!(first.control.is_cancelled());
        assert!(!queue.begin_export(&first));
        queue.requeue(&first.id);
        let third = queue.next_task().unwrap();
        assert_eq!(third.id, "c");
        assert!(queue.begin_export(&third));
        // Export is committed; pause does not interrupt it.
        queue.set_paused(None, true);
        assert_eq!(queue.task_state("c"), Some(TaskState::Exporting));
        queue.finish("c");
        queue.set_paused(Some("a"), false);
        let resumed = queue.next_task().unwrap();
        assert_eq!(resumed.id, "a");
        assert!(!resumed.control.is_cancelled());
        assert_eq!(queue.task_state("b"), Some(TaskState::Paused));
        queue.finish("a");
        queue.set_paused(None, false);
        assert_eq!(queue.next_task().unwrap().id, "b");
        queue.finish("b");
        assert!(queue.next_task().is_none());
    }

    #[test]
    fn cancelling_a_fully_paused_queue_wakes_the_scheduler() {
        let queue = Arc::new(DownloadQueue::new(["a".into()]));
        queue.set_paused(None, true);
        let other = queue.clone();
        let (tx, rx) = mpsc::channel();
        let worker = thread::spawn(move || tx.send(other.next_task().is_none()).unwrap());
        assert!(rx.recv_timeout(Duration::from_millis(30)).is_err());
        queue.cancel();
        assert!(rx.recv_timeout(Duration::from_secs(2)).unwrap());
        worker.join().unwrap();
    }
}
