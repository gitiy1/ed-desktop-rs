use super::*;
use std::path::Path;

impl AppView {
    pub(super) fn reveal_download(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(row) = self
            .rows
            .iter()
            .find(|row| row.book.id() == id && !row.deleting)
        else {
            return;
        };
        let DownloadState::Finished { path, .. } = &row.state else {
            return;
        };
        let result = self.file_actions.reveal(path, cx);
        let message = result
            .err()
            .map(|error| format!("{}: {error:#}", self.text(Text::RevealFailed)));
        if let Some(row) = self.row_mut(id) {
            row.file_action_error = message.clone();
        }
        if let Some(message) = message {
            self.status = message;
        }
        cx.notify();
    }

    pub(super) fn delete_download(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(row) = self.row_mut(id) else { return };
        if row.deleting {
            return;
        }
        let DownloadState::Finished { path, .. } = &row.state else {
            return;
        };
        let path = path.clone();
        row.deleting = true;
        row.file_action_error = None;
        let id = id.to_string();
        let epoch = self.epoch;
        let backend = self.file_actions.clone();
        let task_path = path.clone();
        let operation = cx
            .background_executor()
            .spawn(async move { backend.delete(&task_path) });
        cx.spawn(async move |view, cx| {
            let result = operation.await;
            let _ = view.update(cx, |this, cx| {
                if this.epoch == epoch {
                    this.handle_event(UiEvent::FileDeleted { id, path, result }, cx);
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn file_deleted(&mut self, id: &str, path: &Path, result: Result<DeleteOutcome>) {
        let prefix = self.text(Text::DeleteFailed);
        let Some(row) = self.row_mut(id) else { return };
        if !row.deleting
            || !matches!(&row.state, DownloadState::Finished { path: current, .. } if current == path)
        {
            return;
        }
        row.deleting = false;
        match result {
            Ok(outcome) => {
                row.state = DownloadState::Deleted(outcome);
                row.file_action_error = None;
                row.received = 0;
                row.total = 0;
                row.speed = 0.0;
            }
            Err(error) => {
                let message = format!("{prefix}: {error:#}");
                row.file_action_error = Some(message.clone());
                self.status = message;
            }
        }
    }
}
