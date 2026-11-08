use super::*;
use std::collections::HashMap;

pub(super) fn start_queue(
    session: Session,
    books: Vec<Book>,
    stems: HashMap<String, String>,
    settings: AppSettings,
    queue: Arc<DownloadQueue>,
    sender: EventSender,
    locale: Locale,
) {
    thread::spawn(move || {
        let books: HashMap<_, _> = books.into_iter().map(|book| (book.id(), book)).collect();
        while let Some(attempt) = queue.next_task() {
            let id = &attempt.id;
            let book = &books[id];
            let stem = &stems[id];
            let raw_path = settings.output_directory.join(format!("{stem}.books.epub"));
            sender.send(UiEvent::Started { id: id.clone() });
            let progress_sender = sender.clone();
            let progress_id = id.clone();
            // A completed source may remain after a paused handoff to export
            // or a failed export. Reuse it instead of downloading it again.
            let downloaded = if raw_path.is_file() {
                let total = std::fs::metadata(&raw_path)
                    .map(|meta| meta.len())
                    .unwrap_or(0);
                sender.send(UiEvent::Progress {
                    id: id.clone(),
                    progress: DownloadProgress {
                        received: total,
                        total,
                        bytes_per_second: 0.0,
                    },
                });
                Ok(raw_path.clone())
            } else {
                downloader::download_legacy(
                    &session,
                    book,
                    &raw_path,
                    settings.download_workers(),
                    &settings.proxy,
                    attempt.control.clone(),
                    move |progress| {
                        progress_sender.send(UiEvent::Progress {
                            id: progress_id.clone(),
                            progress,
                        })
                    },
                )
            };
            if queue.is_cancelled() {
                break;
            }
            if !queue.begin_export(&attempt) {
                queue.requeue(id);
                sender.send(UiEvent::TaskChanged { id: id.clone() });
                continue;
            }
            let result = downloaded.and_then(|encrypted| {
                sender.send(UiEvent::TaskChanged { id: id.clone() });
                export::finish_download(
                    &encrypted,
                    &settings.output_directory.join(format!("{stem}.epub")),
                    settings.auto_decrypt,
                    settings.keep_encrypted_original,
                    std::slice::from_ref(&session.email),
                )
            });
            queue.finish(id);
            match result {
                Ok(exported) => sender.send(UiEvent::Finished {
                    id: id.clone(),
                    path: exported.path,
                    decrypted: exported.decrypted,
                    cleanup_warning: exported.cleanup_warning,
                }),
                Err(error) => sender.send(UiEvent::Failed {
                    id: id.clone(),
                    message: format!("{error:#}"),
                }),
            }
        }
        sender.send(UiEvent::QueueDone);
        sender.send(UiEvent::Message(
            i18n::text(locale, Text::QueueFinished).to_string(),
        ));
    });
}
