use crate::api::{self, Book, Session};
use crate::downloader::{self, DownloadProgress};
use crate::lcp;
use crate::settings::{self, AppSettings};
use anyhow::Result;
use async_channel::{Receiver, Sender};
use gpui::{
    App, Application, Bounds, ClickEvent, Context, FocusHandle, IntoElement, KeyDownEvent, Render,
    SharedString, Styled, Window, WindowBounds, WindowOptions, div, prelude::*, px, relative, rgb,
    size,
};
use std::path::{Path, PathBuf};
use std::thread;

#[derive(Clone, Copy, PartialEq, Eq)]
enum FocusedField {
    Email,
    Password,
    Output,
    Workers,
}

#[derive(Clone)]
struct BookRow {
    book: Book,
    selected: bool,
    status: String,
    received: u64,
    total: u64,
    speed: f64,
}

impl BookRow {
    fn new(book: Book) -> Self {
        Self {
            book,
            selected: false,
            status: "Ready".to_string(),
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
    },
    Failed {
        id: String,
        message: String,
    },
    QueueDone,
    Message(String),
}

pub struct AppView {
    email: String,
    password: String,
    output_text: String,
    workers_text: String,
    settings: AppSettings,
    session: Option<Session>,
    rows: Vec<BookRow>,
    status: String,
    focused_field: FocusedField,
    cursor: usize,
    select_all: bool,
    focus_handle: FocusHandle,
    events_tx: Sender<UiEvent>,
    events_rx: Receiver<UiEvent>,
    listener_started: bool,
    busy: bool,
}

impl AppView {
    fn new(cx: &mut Context<Self>) -> Self {
        let settings = settings::load();
        let (events_tx, events_rx) = async_channel::unbounded();
        Self {
            email: std::env::var("ED_EMAIL").unwrap_or_default(),
            password: std::env::var("ED_PASSWORD").unwrap_or_default(),
            output_text: settings.output_directory.to_string_lossy().into_owned(),
            workers_text: settings.workers.to_string(),
            settings,
            session: None,
            rows: Vec::new(),
            status: "Sign in to load your purchased books.".to_string(),
            focused_field: FocusedField::Email,
            cursor: std::env::var("ED_EMAIL")
                .map(|value| value.chars().count())
                .unwrap_or(0),
            select_all: false,
            focus_handle: cx.focus_handle(),
            events_tx,
            events_rx,
            listener_started: false,
            busy: false,
        }
    }

    fn start_listener(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.listener_started {
            return;
        }
        self.listener_started = true;
        let receiver = self.events_rx.clone();
        cx.spawn_in(window, async move |view, cx| {
            while let Ok(event) = receiver.recv().await {
                if view
                    .update(cx, |this, cx| {
                        this.handle_event(event);
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
                if self.focused_field == FocusedField::Password {
                    self.cursor = 0;
                }
                self.replace_books(books);
                self.busy = false;
                self.status = format!("Signed in. {} purchased books loaded.", self.rows.len());
            }
            UiEvent::LoginDone(Err(error)) => {
                self.busy = false;
                self.status = format!("Sign in failed: {error:#}");
            }
            UiEvent::BooksDone(Ok(books)) => {
                self.replace_books(books);
                self.busy = false;
                self.status = format!("{} purchased books loaded.", self.rows.len());
            }
            UiEvent::BooksDone(Err(error)) => {
                self.busy = false;
                self.status = format!("Could not load books: {error:#}");
            }
            UiEvent::Started { id } => {
                if let Some(row) = self.row_mut(&id) {
                    row.status = "Downloading".to_string();
                    row.received = 0;
                    row.total = 0;
                    row.speed = 0.0;
                }
            }
            UiEvent::Progress { id, progress } => {
                if let Some(row) = self.row_mut(&id) {
                    row.received = progress.received;
                    row.total = progress.total;
                    row.speed = progress.bytes_per_second;
                }
            }
            UiEvent::Finished {
                id,
                path,
                decrypted,
            } => {
                if let Some(row) = self.row_mut(&id) {
                    row.status = if decrypted {
                        format!(
                            "Ready: {}",
                            path.file_name().unwrap_or_default().to_string_lossy()
                        )
                    } else {
                        format!(
                            "Downloaded: {}",
                            path.file_name().unwrap_or_default().to_string_lossy()
                        )
                    };
                    row.received = row.total;
                    row.speed = 0.0;
                }
            }
            UiEvent::Failed { id, message } => {
                if let Some(row) = self.row_mut(&id) {
                    row.status = format!("Failed: {message}");
                    row.speed = 0.0;
                }
            }
            UiEvent::QueueDone => self.busy = false,
            UiEvent::Message(message) => self.status = message,
        }
    }

    fn row_mut(&mut self, id: &str) -> Option<&mut BookRow> {
        self.rows.iter_mut().find(|row| row.book.id() == id)
    }

    fn replace_books(&mut self, books: Vec<Book>) {
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
                        status: row.status.clone(),
                        received: row.received,
                        total: row.total,
                        speed: row.speed,
                    }
                } else {
                    BookRow::new(book)
                }
            })
            .collect();
    }

    fn click_field(&mut self, field: FocusedField, window: &mut Window, cx: &mut Context<Self>) {
        self.focused_field = field;
        self.cursor = self.current_value().chars().count();
        self.select_all = false;
        window.focus(&self.focus_handle);
        cx.notify();
    }

    fn handle_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let key = event.keystroke.key.to_ascii_lowercase();
        if event.keystroke.modifiers.secondary() {
            match key.as_str() {
                "a" => {
                    self.cursor = self.current_value().chars().count();
                    self.select_all = true;
                    cx.notify();
                    return;
                }
                "v" => {
                    if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                        self.insert_text(&text);
                        self.sync_settings();
                        cx.notify();
                    }
                    return;
                }
                _ => {}
            }
        }
        match key.as_str() {
            "backspace" => {
                self.delete_selection_or_char(false);
                self.sync_settings();
                cx.notify();
                return;
            }
            "delete" => {
                self.delete_selection_or_char(true);
                self.sync_settings();
                cx.notify();
                return;
            }
            "left" | "arrowleft" => {
                self.cursor = if self.select_all {
                    0
                } else {
                    self.cursor.saturating_sub(1)
                };
                self.select_all = false;
                cx.notify();
                return;
            }
            "right" | "arrowright" => {
                self.cursor = if self.select_all {
                    self.current_value().chars().count()
                } else {
                    (self.cursor + 1).min(self.current_value().chars().count())
                };
                self.select_all = false;
                cx.notify();
                return;
            }
            "home" => {
                self.cursor = 0;
                self.select_all = false;
                cx.notify();
                return;
            }
            "end" => {
                self.cursor = self.current_value().chars().count();
                self.select_all = false;
                cx.notify();
                return;
            }
            "tab" => {
                self.focused_field = match self.focused_field {
                    FocusedField::Email => FocusedField::Password,
                    FocusedField::Password => FocusedField::Output,
                    FocusedField::Output => FocusedField::Workers,
                    FocusedField::Workers => FocusedField::Email,
                };
                self.cursor = self.current_value().chars().count();
                self.select_all = false;
                cx.notify();
                return;
            }
            "enter" if self.focused_field == FocusedField::Password => {
                self.start_login(cx);
                return;
            }
            _ => {}
        }
        if event.keystroke.modifiers.modified() {
            return;
        }
        if let Some(text) = event.keystroke.key_char.as_ref() {
            let text: &str = text.as_ref();
            if text.chars().any(|character| character.is_control()) {
                return;
            }
            self.insert_text(text);
            self.sync_settings();
            cx.notify();
        }
    }

    fn current_value(&self) -> &str {
        match self.focused_field {
            FocusedField::Email => &self.email,
            FocusedField::Password => &self.password,
            FocusedField::Output => &self.output_text,
            FocusedField::Workers => &self.workers_text,
        }
    }

    fn current_value_mut(&mut self) -> &mut String {
        match self.focused_field {
            FocusedField::Email => &mut self.email,
            FocusedField::Password => &mut self.password,
            FocusedField::Output => &mut self.output_text,
            FocusedField::Workers => &mut self.workers_text,
        }
    }

    fn insert_text(&mut self, text: &str) {
        if self.focused_field == FocusedField::Workers
            && !text.chars().all(|character| character.is_ascii_digit())
        {
            return;
        }
        let selected = self.select_all;
        let cursor = self.cursor.min(self.current_value().chars().count());
        self.cursor = cursor;
        let value = self.current_value_mut();
        if selected {
            value.clear();
        }
        let byte_index = char_to_byte_index(value, if selected { 0 } else { cursor });
        value.insert_str(byte_index, text);
        self.cursor = if selected { 0 } else { cursor } + text.chars().count();
        self.select_all = false;
    }

    fn delete_selection_or_char(&mut self, forward: bool) {
        if self.select_all {
            self.current_value_mut().clear();
            self.cursor = 0;
            self.select_all = false;
            return;
        }
        let cursor = self.cursor.min(self.current_value().chars().count());
        self.cursor = cursor;
        let value = self.current_value_mut();
        let length = value.chars().count();
        if forward {
            if cursor < length {
                let start = char_to_byte_index(value, cursor);
                let end = char_to_byte_index(value, cursor + 1);
                value.replace_range(start..end, "");
            }
        } else if cursor > 0 {
            let start = char_to_byte_index(value, cursor - 1);
            let end = char_to_byte_index(value, cursor);
            value.replace_range(start..end, "");
            self.cursor -= 1;
        }
    }

    fn sync_settings(&mut self) {
        if !self.output_text.trim().is_empty() {
            self.settings.output_directory = PathBuf::from(self.output_text.trim());
        }
        if let Ok(workers) = self.workers_text.parse::<usize>() {
            self.settings.workers = workers.clamp(1, 16);
        }
        settings::save(&self.settings);
    }

    fn start_login(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.sync_settings();
        self.busy = true;
        self.status = "Signing in and loading your bookshelf…".to_string();
        let email = self.email.clone();
        let password = self.password.clone();
        let sender = self.events_tx.clone();
        cx.notify();
        thread::spawn(move || {
            let version = api::latest_version();
            let result = api::login(&email, &password, version)
                .and_then(|session| api::purchased_books(&session).map(|books| (session, books)));
            let _ = sender.send_blocking(UiEvent::LoginDone(result));
        });
    }

    fn refresh_books(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.session.clone() else {
            self.status = "Sign in before refreshing the bookshelf.".to_string();
            cx.notify();
            return;
        };
        if self.busy {
            return;
        }
        self.busy = true;
        self.status = "Refreshing purchased books…".to_string();
        let sender = self.events_tx.clone();
        cx.notify();
        thread::spawn(move || {
            let _ = sender.send_blocking(UiEvent::BooksDone(api::purchased_books(&session)));
        });
    }

    fn toggle_all(&mut self, cx: &mut Context<Self>) {
        let should_select = self.rows.iter().any(|row| !row.selected);
        for row in &mut self.rows {
            row.selected = should_select;
        }
        cx.notify();
    }

    fn toggle_book(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(row) = self.row_mut(id) {
            row.selected = !row.selected;
        }
        cx.notify();
    }

    fn toggle_decrypt(&mut self, cx: &mut Context<Self>) {
        self.settings.auto_decrypt = !self.settings.auto_decrypt;
        settings::save(&self.settings);
        cx.notify();
    }

    fn choose_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let folder = rfd::FileDialog::new()
            .set_title("Choose download folder")
            .set_directory(&self.settings.output_directory)
            .pick_folder();
        if let Some(folder) = folder {
            self.output_text = folder.to_string_lossy().into_owned();
            self.focused_field = FocusedField::Output;
            self.cursor = self.output_text.chars().count();
            self.select_all = false;
            window.focus(&self.focus_handle);
            self.sync_settings();
            cx.notify();
        }
    }

    fn start_downloads(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            self.status = "Wait for the current download queue to finish.".to_string();
            cx.notify();
            return;
        }
        let Some(session) = self.session.clone() else {
            self.status = "Sign in before downloading books.".to_string();
            cx.notify();
            return;
        };
        let books: Vec<Book> = self
            .rows
            .iter_mut()
            .filter(|row| row.selected && !row.status.starts_with("Downloading"))
            .map(|row| {
                row.status = "Queued".to_string();
                row.received = 0;
                row.total = 0;
                row.book.clone()
            })
            .collect();
        if books.is_empty() {
            self.status = "Select at least one book.".to_string();
            cx.notify();
            return;
        }
        self.sync_settings();
        let output = self.settings.output_directory.clone();
        let workers = self.settings.workers;
        let auto_decrypt = self.settings.auto_decrypt;
        let sender = self.events_tx.clone();
        self.busy = true;
        self.status = format!(
            "Queued {} books with {workers} download workers.",
            books.len()
        );
        cx.notify();
        thread::spawn(move || {
            for book in books {
                let id = book.id();
                let _ = sender.send_blocking(UiEvent::Started { id: id.clone() });
                let file_stem =
                    safe_filename(&book.filename().unwrap_or_else(|| book.title()), &id);
                let file_stem = Path::new(&file_stem)
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or(&id)
                    .to_string();
                let raw_path = output.join(format!("{file_stem}.books.epub"));
                let progress_sender = sender.clone();
                let progress_id = id.clone();
                let downloaded = downloader::download_legacy(
                    &session,
                    &book,
                    &raw_path,
                    workers,
                    move |progress| {
                        let _ = progress_sender.send_blocking(UiEvent::Progress {
                            id: progress_id.clone(),
                            progress,
                        });
                    },
                );
                match downloaded {
                    Ok(raw_path) if auto_decrypt => {
                        let plain_path = output.join(format!("{file_stem}.epub"));
                        let passphrase = session.email.clone();
                        match lcp::decrypt_epub(&raw_path, &plain_path, &[passphrase]) {
                            Ok(path) => {
                                let _ = sender.send_blocking(UiEvent::Finished {
                                    id,
                                    path,
                                    decrypted: true,
                                });
                            }
                            Err(error) => {
                                let _ = sender.send_blocking(UiEvent::Failed {
                                    id,
                                    message: format!("downloaded; export failed: {error:#}"),
                                });
                            }
                        }
                    }
                    Ok(path) => {
                        let _ = sender.send_blocking(UiEvent::Finished {
                            id,
                            path,
                            decrypted: false,
                        });
                    }
                    Err(error) => {
                        let _ = sender.send_blocking(UiEvent::Failed {
                            id,
                            message: format!("{error:#}"),
                        });
                    }
                }
            }
            let _ = sender.send_blocking(UiEvent::QueueDone);
            let _ = sender.send_blocking(UiEvent::Message("Download queue finished.".to_string()));
        });
    }

    fn button(
        label: impl Into<SharedString>,
        primary: bool,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let label = label.into();
        let background = if primary {
            rgb(0x167d70)
        } else {
            rgb(0xe8eeeb)
        };
        let foreground = if primary {
            rgb(0xffffff)
        } else {
            rgb(0x273a35)
        };
        div()
            .id(label.clone())
            .px_4()
            .py_2()
            .rounded_md()
            .bg(background)
            .text_color(foreground)
            .text_sm()
            .cursor_pointer()
            .child(label)
            .on_click(on_click)
    }

    fn input_field(
        label: &'static str,
        value: String,
        placeholder: &'static str,
        focused: bool,
        cursor: usize,
        masked: bool,
        on_mouse_down: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let mut shown = if masked {
            "•".repeat(value.chars().count())
        } else if value.is_empty() {
            placeholder.to_string()
        } else {
            value
        };
        if focused && shown != placeholder {
            let byte_index = char_to_byte_index(&shown, cursor);
            shown.insert(byte_index, '|');
        }
        let shown_color = if shown == placeholder {
            rgb(0x899590)
        } else {
            rgb(0x243631)
        };
        div()
            .flex()
            .flex_col()
            .flex_grow()
            .gap_1()
            .child(div().text_xs().text_color(rgb(0x67756f)).child(label))
            .child(
                div()
                    .id((
                        "field",
                        match label {
                            "EMAIL" => 0usize,
                            "PASSWORD" => 1,
                            "SAVE TO" => 2,
                            _ => 3,
                        },
                    ))
                    .h(px(38.))
                    .px_3()
                    .flex()
                    .items_center()
                    .rounded_md()
                    .border_1()
                    .border_color(if focused {
                        rgb(0x168576)
                    } else {
                        rgb(0xd4ddd9)
                    })
                    .bg(rgb(0xffffff))
                    .text_sm()
                    .text_color(shown_color)
                    .child(shown)
                    .on_click(on_mouse_down),
            )
    }
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.start_listener(window, cx);
        let selected_count = self.rows.iter().filter(|row| row.selected).count();
        let all_selected = !self.rows.is_empty() && self.rows.iter().all(|row| row.selected);
        let rows = self.rows.iter().enumerate().map(|(row_index, row)| {
            let id = row.book.id();
            let selected = row.selected;
            let title = row.book.title();
            let author = row.book.author();
            let status = row.status.clone();
            let progress = row.progress();
            let progress_text = if row.total > 0 {
                format!(
                    "{} / {}",
                    format_bytes(row.received),
                    format_bytes(row.total)
                )
            } else {
                String::new()
            };
            let speed = if row.speed > 0.0 {
                format_speed(row.speed)
            } else {
                String::new()
            };
            div()
                .flex()
                .items_center()
                .gap_3()
                .px_3()
                .py_3()
                .border_b_1()
                .border_color(rgb(0xe8eeeb))
                .child(
                    div()
                        .id(("select", row_index))
                        .w(px(28.))
                        .text_color(if selected {
                            rgb(0x167d70)
                        } else {
                            rgb(0x87928d)
                        })
                        .cursor_pointer()
                        .child(if selected { "☑" } else { "☐" })
                        .on_click(cx.listener(move |this, _, _, cx| this.toggle_book(&id, cx))),
                )
                .child(
                    div()
                        .w(px(58.))
                        .text_xs()
                        .text_color(rgb(0x738079))
                        .child(row.book.id()),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .flex_grow()
                        .child(div().text_sm().text_color(rgb(0x243631)).child(title))
                        .child(div().text_xs().text_color(rgb(0x718079)).child(author)),
                )
                .child(
                    div()
                        .w(px(290.))
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .text_xs()
                                .text_color(rgb(0x718079))
                                .child(status)
                                .child(format!("{progress_text}  {speed}")),
                        )
                        .child(
                            div()
                                .w_full()
                                .h(px(5.))
                                .rounded_md()
                                .bg(rgb(0xe5ebe8))
                                .child(
                                    div()
                                        .h_full()
                                        .w(relative(progress))
                                        .rounded_md()
                                        .bg(rgb(0x168576)),
                                ),
                        ),
                )
        });

        div()
            .size_full()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event, _, cx| this.handle_key(event, cx)))
            .bg(rgb(0xf4f7f5))
            .text_color(rgb(0x243631))
            .flex()
            .flex_col()
            .gap_0()
            .child(
                div()
                    .w_full()
                    .px_7()
                    .py_5()
                    .bg(rgb(0xffffff))
                    .border_b_1()
                    .border_color(rgb(0xe1e8e4))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(21.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child("Endao Books"),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(0x718079))
                                    .child("Purchased library downloader"),
                            ),
                    )
                    .child(div().text_sm().text_color(rgb(0x607068)).child(
                        if self.session.is_some() {
                            "Signed in"
                        } else {
                            "Not signed in"
                        },
                    )),
            )
            .child(
                div()
                    .px_7()
                    .py_4()
                    .flex()
                    .items_end()
                    .gap_3()
                    .child(Self::input_field(
                        "EMAIL",
                        self.email.clone(),
                        "account email",
                        self.focused_field == FocusedField::Email,
                        self.cursor,
                        false,
                        cx.listener(|this, _, window, cx| {
                            this.click_field(FocusedField::Email, window, cx)
                        }),
                    ))
                    .child(Self::input_field(
                        "PASSWORD",
                        self.password.clone(),
                        "password",
                        self.focused_field == FocusedField::Password,
                        self.cursor,
                        true,
                        cx.listener(|this, _, window, cx| {
                            this.click_field(FocusedField::Password, window, cx)
                        }),
                    ))
                    .child(Self::button(
                        "Sign in",
                        true,
                        cx.listener(|this, _, _, cx| this.start_login(cx)),
                    )),
            )
            .child(
                div()
                    .px_7()
                    .pb_4()
                    .flex()
                    .items_end()
                    .gap_3()
                    .child(
                        div()
                            .flex_grow()
                            .flex()
                            .items_end()
                            .gap_2()
                            .child(Self::input_field(
                                "SAVE TO",
                                self.output_text.clone(),
                                "Choose a folder",
                                self.focused_field == FocusedField::Output,
                                self.cursor,
                                false,
                                cx.listener(|this, _, window, cx| {
                                    this.click_field(FocusedField::Output, window, cx)
                                }),
                            ))
                            .child(Self::button(
                                "Browse…",
                                false,
                                cx.listener(|this, _, window, cx| this.choose_folder(window, cx)),
                            )),
                    )
                    .child(Self::input_field(
                        "WORKERS",
                        self.workers_text.clone(),
                        "4",
                        self.focused_field == FocusedField::Workers,
                        self.cursor,
                        false,
                        cx.listener(|this, _, window, cx| {
                            this.click_field(FocusedField::Workers, window, cx)
                        }),
                    ))
                    .child(
                        div()
                            .id("auto-decrypt")
                            .h(px(38.))
                            .px_2()
                            .flex()
                            .items_center()
                            .text_sm()
                            .text_color(rgb(0x334840))
                            .cursor_pointer()
                            .child(if self.settings.auto_decrypt {
                                "☑ Export decrypted EPUB"
                            } else {
                                "☐ Export decrypted EPUB"
                            })
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_decrypt(cx))),
                    ),
            )
            .child(
                div()
                    .px_7()
                    .py_3()
                    .bg(rgb(0xeaf0ed))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(0x56675f))
                            .child("PURCHASED BOOKS"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(Self::button(
                                "Refresh",
                                false,
                                cx.listener(|this, _, _, cx| this.refresh_books(cx)),
                            ))
                            .child(Self::button(
                                if all_selected {
                                    "Clear selection"
                                } else {
                                    "Select all"
                                },
                                false,
                                cx.listener(|this, _, _, cx| this.toggle_all(cx)),
                            ))
                            .child(Self::button(
                                format!("Download selected ({selected_count})"),
                                true,
                                cx.listener(|this, _, _, cx| this.start_downloads(cx)),
                            )),
                    ),
            )
            .child(
                div()
                    .id("book-list")
                    .mx_7()
                    .mt_3()
                    .flex_grow()
                    .min_h(px(100.))
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(0xe1e8e4))
                    .bg(rgb(0xffffff))
                    .overflow_y_scroll()
                    .children(rows),
            )
            .child(
                div()
                    .px_7()
                    .py_3()
                    .text_sm()
                    .text_color(rgb(0x56675f))
                    .child(self.status.clone()),
            )
    }
}

pub fn run() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1180.), px(820.)), cx);
        let _ = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("Endao Books".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| cx.new(AppView::new),
        );
        cx.activate(true);
    });
}

fn safe_filename(value: &str, fallback: &str) -> String {
    let name = Path::new(value)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(fallback);
    let safe: String = name
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, '.' | '_' | '-') {
                character
            } else {
                '_'
            }
        })
        .collect();
    if safe.trim_matches(['.', '_', '-']).is_empty() {
        fallback.to_string()
    } else {
        safe
    }
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
