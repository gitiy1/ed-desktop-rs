use crate::api::{self, Book, Session};
use crate::downloader::{self, DownloadProgress};
use crate::i18n::{self, LanguagePreference, Locale, Text};
use crate::lcp;
use crate::settings::{self, AppSettings, ProxyMode, ProxyProtocol};
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
    ProxyAddress,
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
    fn new(book: Book, locale: Locale) -> Self {
        Self {
            book,
            selected: false,
            status: i18n::text(locale, Text::Ready).to_string(),
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
    proxy_address: String,
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
    persist_settings: bool,
}

impl AppView {
    fn locale(&self) -> Locale {
        self.settings.language.resolve()
    }

    fn text(&self, key: Text) -> &'static str {
        i18n::text(self.locale(), key)
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
            settings,
            session: None,
            rows: Vec::new(),
            status,
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
            persist_settings: true,
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
        let locale = self.locale();
        match event {
            UiEvent::LoginDone(Ok((session, books))) => {
                self.session = Some(session);
                self.password.clear();
                if self.focused_field == FocusedField::Password {
                    self.cursor = 0;
                }
                self.replace_books(books);
                self.busy = false;
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
                if let Some(row) = self.row_mut(&id) {
                    row.status = i18n::text(locale, Text::Downloading).to_string();
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
                            "{}: {}",
                            i18n::text(locale, Text::DecryptedFile),
                            path.file_name().unwrap_or_default().to_string_lossy()
                        )
                    } else {
                        format!(
                            "{}: {}",
                            i18n::text(locale, Text::EncryptedFile),
                            path.file_name().unwrap_or_default().to_string_lossy()
                        )
                    };
                    row.received = row.total;
                    row.speed = 0.0;
                }
            }
            UiEvent::Failed { id, message } => {
                if let Some(row) = self.row_mut(&id) {
                    row.status = format!("{}: {message}", i18n::text(locale, Text::Failed));
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
                        status: row.status.clone(),
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
                    FocusedField::Workers => FocusedField::ProxyAddress,
                    FocusedField::ProxyAddress => FocusedField::Email,
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
            FocusedField::ProxyAddress => &self.proxy_address,
        }
    }

    fn current_value_mut(&mut self) -> &mut String {
        match self.focused_field {
            FocusedField::Email => &mut self.email,
            FocusedField::Password => &mut self.password,
            FocusedField::Output => &mut self.output_text,
            FocusedField::Workers => &mut self.workers_text,
            FocusedField::ProxyAddress => &mut self.proxy_address,
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
        self.settings.proxy.address = self.proxy_address.trim().to_string();
        self.save_settings();
    }

    fn save_settings(&self) {
        if self.persist_settings {
            settings::save(&self.settings);
        }
    }

    fn set_language(&mut self, language: LanguagePreference, cx: &mut Context<Self>) {
        self.settings.language = language;
        self.save_settings();
        cx.notify();
    }

    fn set_proxy_mode(&mut self, mode: ProxyMode, cx: &mut Context<Self>) {
        self.settings.proxy.mode = mode;
        self.sync_settings();
        self.status = self.text(Text::ProxySettingSaved).to_string();
        cx.notify();
    }

    fn set_proxy_protocol(&mut self, protocol: ProxyProtocol, cx: &mut Context<Self>) {
        self.settings.proxy.protocol = protocol;
        self.sync_settings();
        self.status = self.text(Text::ProxySettingSaved).to_string();
        cx.notify();
    }

    fn start_login(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.sync_settings();
        self.busy = true;
        if self.email.trim().is_empty() || self.password.is_empty() {
            self.busy = false;
            self.status = self.text(Text::EmptyCredentials).to_string();
            cx.notify();
            return;
        }
        self.status = self.text(Text::SigningIn).to_string();
        let email = self.email.clone();
        let password = self.password.clone();
        let proxy = self.settings.proxy.clone();
        let sender = self.events_tx.clone();
        cx.notify();
        thread::spawn(move || {
            let version = api::latest_version(&proxy);
            let result = api::login(&email, &password, version, &proxy).and_then(|session| {
                api::purchased_books(&session, &proxy).map(|books| (session, books))
            });
            let _ = sender.send_blocking(UiEvent::LoginDone(result));
        });
    }

    fn refresh_books(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.session.clone() else {
            self.status = self.text(Text::RefreshPrompt).to_string();
            cx.notify();
            return;
        };
        if self.busy {
            return;
        }
        self.busy = true;
        self.status = self.text(Text::Refreshing).to_string();
        let proxy = self.settings.proxy.clone();
        let sender = self.events_tx.clone();
        cx.notify();
        thread::spawn(move || {
            let _ =
                sender.send_blocking(UiEvent::BooksDone(api::purchased_books(&session, &proxy)));
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
        self.save_settings();
        cx.notify();
    }

    fn choose_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let folder = rfd::FileDialog::new()
            .set_title(self.text(Text::SaveTo))
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
            self.status = self.text(Text::QueueRunning).to_string();
            cx.notify();
            return;
        }
        let Some(session) = self.session.clone() else {
            self.status = self.text(Text::SignInPrompt).to_string();
            cx.notify();
            return;
        };
        let locale = self.locale();
        let books: Vec<Book> = self
            .rows
            .iter_mut()
            .filter(|row| row.selected && row.status != i18n::text(locale, Text::Downloading))
            .map(|row| {
                row.status = i18n::text(locale, Text::Queued).to_string();
                row.received = 0;
                row.total = 0;
                row.book.clone()
            })
            .collect();
        if books.is_empty() {
            self.status = self.text(Text::SelectBooks).to_string();
            cx.notify();
            return;
        }
        self.sync_settings();
        let output = self.settings.output_directory.clone();
        let workers = self.settings.workers;
        let auto_decrypt = self.settings.auto_decrypt;
        let proxy = self.settings.proxy.clone();
        let sender = self.events_tx.clone();
        self.busy = true;
        self.status = match locale {
            Locale::English => format!("Queued {} books · {workers} workers", books.len()),
            Locale::SimplifiedChinese => {
                format!("{} 本书已加入队列 · {workers} 个线程", books.len())
            }
        };
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
                    &proxy,
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
                                    message: format!(
                                        "{}: {error:#}",
                                        i18n::text(locale, Text::Downloaded)
                                    ),
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
            let _ = sender.send_blocking(UiEvent::Message(
                i18n::text(locale, Text::QueueFinished).to_string(),
            ));
        });
    }

    fn button(
        label: impl Into<SharedString>,
        primary: bool,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let label = label.into();
        let debug_label = label.to_string();
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
            .debug_selector(move || debug_label.clone())
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

    fn segmented(
        id: &'static str,
        label: impl Into<SharedString>,
        selected: bool,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        div()
            .id(id)
            .debug_selector(|| id.to_string())
            .h(px(34.))
            .px_3()
            .flex()
            .items_center()
            .rounded_md()
            .border_1()
            .border_color(if selected {
                rgb(0x167d70)
            } else {
                rgb(0xd4ddd9)
            })
            .bg(if selected {
                rgb(0x167d70)
            } else {
                rgb(0xffffff)
            })
            .text_sm()
            .text_color(if selected {
                rgb(0xffffff)
            } else {
                rgb(0x354841)
            })
            .cursor_pointer()
            .child(label.into())
            .on_click(on_click)
    }

    fn input_field(
        spec: InputFieldSpec,
        on_mouse_down: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let mut shown = if spec.masked {
            "•".repeat(spec.value.chars().count())
        } else if spec.value.is_empty() {
            spec.placeholder.to_string()
        } else {
            spec.value
        };
        if spec.focused && shown != spec.placeholder {
            let byte_index = char_to_byte_index(&shown, spec.cursor);
            shown.insert(byte_index, '|');
        }
        let shown_color = if shown == spec.placeholder {
            rgb(0x899590)
        } else {
            rgb(0x243631)
        };
        div()
            .flex()
            .flex_col()
            .flex_grow()
            .min_w(if spec.field == FocusedField::ProxyAddress {
                px(220.)
            } else {
                px(100.)
            })
            .gap_1()
            .child(div().text_xs().text_color(rgb(0x67756f)).child(spec.label))
            .child(
                div()
                    .id((
                        "field",
                        match spec.field {
                            FocusedField::Email => 0usize,
                            FocusedField::Password => 1,
                            FocusedField::Output => 2,
                            FocusedField::Workers => 3,
                            FocusedField::ProxyAddress => 4,
                        },
                    ))
                    .debug_selector(|| {
                        format!(
                            "field-{}",
                            match spec.field {
                                FocusedField::Email => 0,
                                FocusedField::Password => 1,
                                FocusedField::Output => 2,
                                FocusedField::Workers => 3,
                                FocusedField::ProxyAddress => 4,
                            }
                        )
                    })
                    .h(px(38.))
                    .px_3()
                    .flex()
                    .items_center()
                    .rounded_md()
                    .border_1()
                    .border_color(if spec.focused {
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
        let locale = self.locale();
        let text = |key| i18n::text(locale, key);
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
                        .debug_selector(move || format!("select-{row_index}"))
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
            .id("app-root")
            .debug_selector(|| "app-root".to_string())
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
                                    .child(text(Text::AppSubtitle)),
                            ),
                    )
                    .child(div().text_sm().text_color(rgb(0x607068)).child(
                        if self.session.is_some() {
                            text(Text::SignedIn)
                        } else {
                            text(Text::NotSignedIn)
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
                        InputFieldSpec {
                            field: FocusedField::Email,
                            label: text(Text::Email),
                            value: self.email.clone(),
                            placeholder: text(Text::EmailPlaceholder),
                            focused: self.focused_field == FocusedField::Email,
                            cursor: self.cursor,
                            masked: false,
                        },
                        cx.listener(|this, _, window, cx| {
                            this.click_field(FocusedField::Email, window, cx)
                        }),
                    ))
                    .child(Self::input_field(
                        InputFieldSpec {
                            field: FocusedField::Password,
                            label: text(Text::Password),
                            value: self.password.clone(),
                            placeholder: text(Text::PasswordPlaceholder),
                            focused: self.focused_field == FocusedField::Password,
                            cursor: self.cursor,
                            masked: true,
                        },
                        cx.listener(|this, _, window, cx| {
                            this.click_field(FocusedField::Password, window, cx)
                        }),
                    ))
                    .child(Self::button(
                        text(Text::SignIn),
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
                                InputFieldSpec {
                                    field: FocusedField::Output,
                                    label: text(Text::SaveTo),
                                    value: self.output_text.clone(),
                                    placeholder: text(Text::FolderPlaceholder),
                                    focused: self.focused_field == FocusedField::Output,
                                    cursor: self.cursor,
                                    masked: false,
                                },
                                cx.listener(|this, _, window, cx| {
                                    this.click_field(FocusedField::Output, window, cx)
                                }),
                            ))
                            .child(Self::button(
                                text(Text::Browse),
                                false,
                                cx.listener(|this, _, window, cx| this.choose_folder(window, cx)),
                            )),
                    )
                    .child(Self::input_field(
                        InputFieldSpec {
                            field: FocusedField::Workers,
                            label: text(Text::Workers),
                            value: self.workers_text.clone(),
                            placeholder: "4",
                            focused: self.focused_field == FocusedField::Workers,
                            cursor: self.cursor,
                            masked: false,
                        },
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
                                format!("☑ {}", text(Text::AutoDecrypt))
                            } else {
                                format!("☐ {}", text(Text::AutoDecrypt))
                            })
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_decrypt(cx))),
                    ),
            )
            .child(
                div()
                    .px_7()
                    .pb_3()
                    .flex()
                    .items_end()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x67756f))
                                    .child(text(Text::Language)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_1()
                                    .child(Self::segmented(
                                        "language-system",
                                        text(Text::FollowSystem),
                                        self.settings.language == LanguagePreference::System,
                                        cx.listener(|this, _, _, cx| {
                                            this.set_language(LanguagePreference::System, cx)
                                        }),
                                    ))
                                    .child(Self::segmented(
                                        "language-english",
                                        text(Text::English),
                                        self.settings.language == LanguagePreference::English,
                                        cx.listener(|this, _, _, cx| {
                                            this.set_language(LanguagePreference::English, cx)
                                        }),
                                    ))
                                    .child(Self::segmented(
                                        "language-zh-cn",
                                        text(Text::SimplifiedChinese),
                                        self.settings.language
                                            == LanguagePreference::SimplifiedChinese,
                                        cx.listener(|this, _, _, cx| {
                                            this.set_language(
                                                LanguagePreference::SimplifiedChinese,
                                                cx,
                                            )
                                        }),
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x67756f))
                                    .child(text(Text::Proxy)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_1()
                                    .child(Self::segmented(
                                        "proxy-system",
                                        text(Text::SystemProxy),
                                        self.settings.proxy.mode == ProxyMode::System,
                                        cx.listener(|this, _, _, cx| {
                                            this.set_proxy_mode(ProxyMode::System, cx)
                                        }),
                                    ))
                                    .child(Self::segmented(
                                        "proxy-manual",
                                        text(Text::ManualProxy),
                                        self.settings.proxy.mode == ProxyMode::Manual,
                                        cx.listener(|this, _, _, cx| {
                                            this.set_proxy_mode(ProxyMode::Manual, cx)
                                        }),
                                    )),
                            ),
                    )
                    .children(if self.settings.proxy.mode == ProxyMode::Manual {
                        Some(
                            div()
                                .flex()
                                .items_end()
                                .gap_1()
                                .child(Self::segmented(
                                    "proxy-http",
                                    text(Text::HttpProxy),
                                    self.settings.proxy.protocol == ProxyProtocol::Http,
                                    cx.listener(|this, _, _, cx| {
                                        this.set_proxy_protocol(ProxyProtocol::Http, cx)
                                    }),
                                ))
                                .child(Self::segmented(
                                    "proxy-socks5",
                                    text(Text::Socks5Proxy),
                                    self.settings.proxy.protocol == ProxyProtocol::Socks5,
                                    cx.listener(|this, _, _, cx| {
                                        this.set_proxy_protocol(ProxyProtocol::Socks5, cx)
                                    }),
                                ))
                                .child(Self::input_field(
                                    InputFieldSpec {
                                        field: FocusedField::ProxyAddress,
                                        label: text(Text::ProxyAddress),
                                        value: self.proxy_address.clone(),
                                        placeholder: text(Text::ProxyAddressPlaceholder),
                                        focused: self.focused_field == FocusedField::ProxyAddress,
                                        cursor: self.cursor,
                                        masked: false,
                                    },
                                    cx.listener(|this, _, window, cx| {
                                        this.click_field(FocusedField::ProxyAddress, window, cx)
                                    }),
                                )),
                        )
                    } else {
                        Some(
                            div()
                                .flex_grow()
                                .pb_2()
                                .text_xs()
                                .text_color(rgb(0x718079))
                                .child(text(Text::SystemProxyFallback)),
                        )
                    }),
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
                            .child(text(Text::PurchasedBooks)),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(Self::button(
                                text(Text::Refresh),
                                false,
                                cx.listener(|this, _, _, cx| this.refresh_books(cx)),
                            ))
                            .child(Self::button(
                                if all_selected {
                                    text(Text::ClearSelection)
                                } else {
                                    text(Text::SelectAll)
                                },
                                false,
                                cx.listener(|this, _, _, cx| this.toggle_all(cx)),
                            ))
                            .child(Self::button(
                                format!("{} ({selected_count})", text(Text::DownloadSelected)),
                                true,
                                cx.listener(|this, _, _, cx| this.start_downloads(cx)),
                            )),
                    ),
            )
            .child(
                div()
                    .id("book-list")
                    .debug_selector(|| "book-list".to_string())
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
                    .id("status-line")
                    .debug_selector(|| "status-line".to_string())
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

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;
    use serde_json::json;

    fn fixture_book(id: u64, title: &str) -> Book {
        Book {
            raw: json!({ "bookId": id, "name": title, "author": "Test Author" }),
        }
    }

    #[gpui::test]
    fn settings_and_bookshelf_support_mouse_and_keyboard_interaction(cx: &mut TestAppContext) {
        let (view, cx) = cx.add_window_view(|_, cx| AppView::new(cx));
        cx.simulate_resize(gpui::size(px(1180.), px(820.)));
        view.update(cx, |view, _| {
            view.persist_settings = false;
            view.email.clear();
            view.password.clear();
            view.rows = vec![
                BookRow::new(fixture_book(1, "Book One"), Locale::English),
                BookRow::new(fixture_book(2, "Book Two"), Locale::English),
            ];
        });
        cx.refresh().unwrap();

        let email_bounds = cx.debug_bounds("field-0").expect("email field rendered");
        cx.simulate_click(email_bounds.center(), gpui::Modifiers::none());
        cx.simulate_input("reader@example.com");
        view.update(cx, |view, _| assert_eq!(view.email, "reader@example.com"));

        let language_bounds = cx
            .debug_bounds("language-zh-cn")
            .expect("Simplified Chinese option rendered");
        cx.simulate_click(language_bounds.center(), gpui::Modifiers::none());
        view.update(cx, |view, _| {
            assert_eq!(
                view.settings.language,
                LanguagePreference::SimplifiedChinese
            );
            assert_eq!(view.text(Text::SignIn), "登录");
        });
        cx.refresh().unwrap();

        let proxy_mode_bounds = cx
            .debug_bounds("proxy-manual")
            .expect("manual proxy option rendered");
        cx.simulate_click(proxy_mode_bounds.center(), gpui::Modifiers::none());
        view.update(cx, |view, _| {
            assert_eq!(view.settings.proxy.mode, ProxyMode::Manual)
        });
        cx.refresh().unwrap();

        let address_bounds = cx
            .debug_bounds("field-4")
            .expect("manual proxy address field rendered");
        cx.simulate_click(address_bounds.center(), gpui::Modifiers::none());
        cx.simulate_input("127.0.0.1:1080");
        view.update(cx, |view, _| {
            assert_eq!(view.proxy_address, "127.0.0.1:1080");
            assert_eq!(view.settings.proxy.address, "127.0.0.1:1080");
        });

        let select_all_bounds = cx.debug_bounds("全选").expect("select all action rendered");
        cx.simulate_click(select_all_bounds.center(), gpui::Modifiers::none());
        view.update(cx, |view, _| {
            assert!(view.rows.iter().all(|row| row.selected))
        });
        cx.refresh().unwrap();
        let first_checkbox = cx
            .debug_bounds("select-0")
            .expect("first book checkbox rendered");
        cx.simulate_click(first_checkbox.center(), gpui::Modifiers::none());
        view.update(cx, |view, _| {
            assert!(!view.rows[0].selected);
            assert!(view.rows[1].selected);
        });

        let address_bounds = cx
            .debug_bounds("field-4")
            .expect("proxy address remains visible");
        let app_bounds = cx
            .debug_bounds("app-root")
            .expect("root view bounds rendered");
        assert!(address_bounds.origin.x >= app_bounds.origin.x);
        assert!(
            address_bounds.origin.x + address_bounds.size.width
                <= app_bounds.origin.x + app_bounds.size.width
        );
        assert!(
            address_bounds.size.width >= px(160.),
            "proxy field width was {:?} in root bounds {:?}",
            address_bounds.size.width,
            app_bounds.size
        );
        let list_bounds = cx.debug_bounds("book-list").expect("bookshelf rendered");
        let status_bounds = cx.debug_bounds("status-line").expect("status rendered");
        assert!(list_bounds.origin.y >= address_bounds.origin.y + address_bounds.size.height);
        assert!(
            status_bounds.origin.y + status_bounds.size.height
                <= app_bounds.origin.y + app_bounds.size.height
        );
    }
}
