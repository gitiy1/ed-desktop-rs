mod actions;
mod demo;
mod jobs;
mod settings_view;
mod shell;
#[cfg(test)]
mod tests;
mod views;
mod widgets;

pub use shell::run;

use crate::api::{self, Book, Session};
use crate::downloader::{self, DownloadProgress};
use crate::export;
use crate::filenames::export_stems;
use crate::i18n::{self, LanguagePreference, Locale, Text};
use crate::queue::{DownloadQueue, TaskState};
use crate::settings::{self, AppSettings, ProxyMode, ProxyProtocol};
use crate::theme::{Palette, ThemePreference};
use anyhow::Result;
use async_channel::{Receiver, Sender};
use gpui::{
    App, Application, Bounds, ClickEvent, Context, FocusHandle, IntoElement, KeyDownEvent, Render,
    SharedString, Styled, Window, WindowAppearance, WindowBounds, WindowOptions, div, prelude::*,
    px, relative, rgb, size,
};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FocusedField {
    Email,
    Password,
    Output,
    Workers,
    ProxyAddress,
    Search,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ActivePage {
    #[default]
    Library,
    Downloads,
    Settings,
}

#[derive(Clone, Copy)]
enum NavItem {
    Library,
    Downloads,
    Settings,
}

#[derive(Clone)]
struct BookRow {
    book: Book,
    selected: bool,
    state: DownloadState,
    received: u64,
    total: u64,
    speed: f64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum DownloadState {
    Ready,
    Queued,
    Downloading,
    Paused,
    Exporting,
    Finished {
        path: PathBuf,
        decrypted: bool,
        cleanup_warning: Option<String>,
    },
    Failed(String),
}

impl DownloadState {
    fn label(&self, locale: Locale) -> String {
        match self {
            Self::Ready => i18n::text(locale, Text::Ready).to_string(),
            Self::Queued => i18n::text(locale, Text::Queued).to_string(),
            Self::Downloading => i18n::text(locale, Text::Downloading).to_string(),
            Self::Paused => i18n::text(locale, Text::Paused).to_string(),
            Self::Exporting => i18n::text(locale, Text::Exporting).to_string(),
            Self::Finished {
                path,
                decrypted,
                cleanup_warning,
            } => {
                let label = if *decrypted {
                    i18n::text(locale, Text::DecryptedFile)
                } else {
                    i18n::text(locale, Text::EncryptedFile)
                };
                if let Some(warning) = cleanup_warning {
                    return format!("{}: {warning}", i18n::text(locale, Text::CleanupFailed));
                }
                format!(
                    "{label}: {}",
                    path.file_name().unwrap_or_default().to_string_lossy()
                )
            }
            Self::Failed(message) => format!("{}: {message}", i18n::text(locale, Text::Failed)),
        }
    }

    fn is_active(&self) -> bool {
        matches!(
            self,
            Self::Queued | Self::Downloading | Self::Paused | Self::Exporting
        )
    }
}

struct InputFieldSpec {
    field: FocusedField,
    label: &'static str,
    value: String,
    placeholder: &'static str,
    focused: bool,
    cursor: usize,
    masked: bool,
}

impl BookRow {
    fn new(book: Book, _locale: Locale) -> Self {
        Self {
            book,
            selected: false,
            state: DownloadState::Ready,
            received: 0,
            total: 0,
            speed: 0.0,
        }
    }

    fn progress(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.received as f32 / self.total as f32).clamp(0.0, 1.0)
        }
    }
}

enum UiEvent {
    LoginDone(Result<(Session, Vec<Book>)>),
    BooksDone(Result<Vec<Book>>),
    Started {
        id: String,
    },
    Progress {
        id: String,
        progress: DownloadProgress,
    },
    Finished {
        id: String,
        path: PathBuf,
        decrypted: bool,
        cleanup_warning: Option<String>,
    },
    Failed {
        id: String,
        message: String,
    },
    TaskChanged {
        id: String,
    },
    QueueDone,
    Message(String),
}

#[derive(Clone)]
struct EventSender {
    epoch: u64,
    channel: Sender<(u64, UiEvent)>,
}

impl EventSender {
    fn send(&self, event: UiEvent) {
        let _ = self.channel.send_blocking((self.epoch, event));
    }
}

pub struct AppView {
    email: String,
    password: String,
    output_text: String,
    workers_text: String,
    proxy_address: String,
    search_text: String,
    settings: AppSettings,
    session: Option<Session>,
    rows: Vec<BookRow>,
    queued_download_ids: HashSet<String>,
    page: ActivePage,
    download_control: Option<Arc<DownloadQueue>>,
    status: String,
    focused_field: FocusedField,
    cursor: usize,
    select_all: bool,
    focus_handle: FocusHandle,
    events_tx: Sender<(u64, UiEvent)>,
    events_rx: Receiver<(u64, UiEvent)>,
    epoch: u64,
    listener_started: bool,
    system_appearance: WindowAppearance,
    appearance_subscription: Option<gpui::Subscription>,
    busy: bool,
    persist_settings: bool,
    demo: bool,
}

impl Drop for AppView {
    fn drop(&mut self) {
        if let Some(control) = &self.download_control {
            control.cancel();
        }
        self.events_tx.close();
    }
}

impl AppView {
    fn event_sender(&self) -> EventSender {
        EventSender {
            epoch: self.epoch,
            channel: self.events_tx.clone(),
        }
    }
    fn locale(&self) -> Locale {
        self.settings.language.resolve()
    }

    fn text(&self, key: Text) -> &'static str {
        i18n::text(self.locale(), key)
    }

    fn palette(&self) -> Palette {
        self.settings.theme.palette(self.system_appearance)
    }

    fn system_appearance_changed(&mut self, appearance: WindowAppearance, cx: &mut Context<Self>) {
        self.system_appearance = appearance;
        if self.settings.theme == ThemePreference::System {
            cx.notify();
        }
    }

    fn new(cx: &mut Context<Self>) -> Self {
        let settings = settings::load();
        let status = i18n::text(settings.language.resolve(), Text::SignInPrompt).to_string();
        let (events_tx, events_rx) = async_channel::unbounded();
        Self {
            email: std::env::var("ED_EMAIL").unwrap_or_default(),
            password: std::env::var("ED_PASSWORD").unwrap_or_default(),
            output_text: settings.output_directory.to_string_lossy().into_owned(),
            workers_text: settings.workers.to_string(),
            proxy_address: settings.proxy.address.clone(),
            search_text: String::new(),
            settings,
            session: None,
            rows: Vec::new(),
            queued_download_ids: HashSet::new(),
            page: ActivePage::Library,
            download_control: None,
            status,
            focused_field: FocusedField::Email,
            cursor: std::env::var("ED_EMAIL")
                .map(|value| value.chars().count())
                .unwrap_or(0),
            select_all: false,
            focus_handle: cx.focus_handle(),
            events_tx,
            events_rx,
            epoch: 0,
            listener_started: false,
            system_appearance: cx.window_appearance(),
            appearance_subscription: None,
            busy: false,
            persist_settings: true,
            demo: false,
        }
    }

    fn start_listener(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.listener_started {
            return;
        }
        self.listener_started = true;
        self.system_appearance = window.appearance();
        self.appearance_subscription =
            Some(cx.observe_window_appearance(window, |this, window, cx| {
                this.system_appearance_changed(window.appearance(), cx);
            }));
        let receiver = self.events_rx.clone();
        cx.spawn_in(window, async move |view, cx| {
            while let Ok((epoch, event)) = receiver.recv().await {
                if view
                    .update(cx, |this, cx| {
                        if this.epoch == epoch {
                            this.handle_event(event);
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }

    fn handle_event(&mut self, event: UiEvent) {
        match event {
            UiEvent::LoginDone(Ok((session, books))) => {
                self.session = Some(session);
                self.password.clear();
                self.focused_field = FocusedField::Search;
                self.cursor = 0;
                self.select_all = false;
                self.replace_books(books);
                self.busy = false;
                self.page = ActivePage::Library;
                self.status = self.books_loaded_message();
            }
            UiEvent::LoginDone(Err(error)) => {
                self.busy = false;
                self.status = format!("{}: {error:#}", self.text(Text::SignInFailed));
            }
            UiEvent::BooksDone(Ok(books)) => {
                self.replace_books(books);
                self.busy = false;
                self.status = self.books_loaded_message();
            }
            UiEvent::BooksDone(Err(error)) => {
                self.busy = false;
                self.status = format!("{}: {error:#}", self.text(Text::RefreshFailed));
            }
            UiEvent::Started { id } => {
                self.sync_task_state(&id);
                if let Some(row) = self.row_mut(&id) {
                    row.speed = 0.0;
                }
            }
            UiEvent::Progress { id, progress } => {
                if let Some(row) = self.row_mut(&id) {
                    row.received = progress.received;
                    row.total = progress.total;
                    row.speed = if row.state == DownloadState::Downloading {
                        progress.bytes_per_second
                    } else {
                        0.0
                    };
                }
            }
            UiEvent::Finished {
                id,
                path,
                decrypted,
                cleanup_warning,
            } => {
                if let Some(row) = self.row_mut(&id) {
                    row.state = DownloadState::Finished {
                        path,
                        decrypted,
                        cleanup_warning,
                    };
                    row.received = row.total;
                    row.speed = 0.0;
                }
            }
            UiEvent::Failed { id, message } => {
                if let Some(row) = self.row_mut(&id) {
                    row.state = DownloadState::Failed(message);
                    row.speed = 0.0;
                }
            }
            UiEvent::TaskChanged { id } => self.sync_task_state(&id),
            UiEvent::QueueDone => {
                self.busy = false;
                self.download_control = None;
            }
            UiEvent::Message(message) => self.status = message,
        }
    }

    fn sync_task_state(&mut self, id: &str) {
        let state = self
            .download_control
            .as_ref()
            .and_then(|queue| queue.task_state(id));
        if let Some(state) = state
            && let Some(row) = self.row_mut(id)
        {
            row.state = match state {
                TaskState::Queued => DownloadState::Queued,
                TaskState::Downloading => DownloadState::Downloading,
                TaskState::Paused => DownloadState::Paused,
                TaskState::Exporting => DownloadState::Exporting,
            };
            if row.state != DownloadState::Downloading {
                row.speed = 0.0;
            }
        }
    }

    fn sync_queue_states(&mut self) {
        for id in self.queued_download_ids.clone() {
            self.sync_task_state(&id);
        }
    }

    fn row_mut(&mut self, id: &str) -> Option<&mut BookRow> {
        self.rows.iter_mut().find(|row| row.book.id() == id)
    }

    fn books_loaded_message(&self) -> String {
        match self.locale() {
            Locale::English => format!("{} {}.", self.rows.len(), self.text(Text::BooksLoaded)),
            Locale::SimplifiedChinese => {
                format!("{}{}。", self.rows.len(), self.text(Text::BooksLoaded))
            }
        }
    }

    fn replace_books(&mut self, books: Vec<Book>) {
        let locale = self.locale();
        let old: std::collections::HashMap<String, BookRow> = self
            .rows
            .drain(..)
            .map(|row| (row.book.id(), row))
            .collect();
        self.rows = books
            .into_iter()
            .map(|book| {
                if let Some(row) = old.get(&book.id()) {
                    BookRow {
                        book,
                        selected: row.selected,
                        state: row.state.clone(),
                        received: row.received,
                        total: row.total,
                        speed: row.speed,
                    }
                } else {
                    BookRow::new(book, locale)
                }
            })
            .collect();
    }
}

fn cover_color(id: &str) -> gpui::Rgba {
    let palette = [0x597d71, 0x9a7754, 0x6f7f9a, 0x8d6b78, 0x7e865c];
    let index = id.bytes().fold(0_usize, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(byte as usize)
    });
    rgb(palette[index % palette.len()])
}

fn format_bytes(value: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut amount = value as f64;
    let mut unit = 0;
    while amount >= 1024.0 && unit < UNITS.len() - 1 {
        amount /= 1024.0;
        unit += 1;
    }
    format!("{amount:.1} {}", UNITS[unit])
}

fn format_speed(value: f64) -> String {
    format!("{}/s", format_bytes(value.max(0.0) as u64))
}

fn char_to_byte_index(value: &str, character_index: usize) -> usize {
    value
        .char_indices()
        .nth(character_index)
        .map(|(byte_index, _)| byte_index)
        .unwrap_or(value.len())
}
