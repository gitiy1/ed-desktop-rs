use super::*;

impl AppView {
    pub(super) fn click_field(
        &mut self,
        field: FocusedField,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focused_field = field;
        self.cursor = self.current_value().chars().count();
        self.select_all = false;
        window.focus(&self.focus_handle);
        cx.notify();
    }

    pub(super) fn handle_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let key = event.keystroke.key.to_ascii_lowercase();
        if !self.visible_fields().contains(&self.focused_field) {
            return;
        }
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
                let fields = self.visible_fields();
                if fields.is_empty() {
                    return;
                }
                let index = fields
                    .iter()
                    .position(|field| *field == self.focused_field)
                    .unwrap_or(0);
                let next = if event.keystroke.modifiers.shift {
                    index.checked_sub(1).unwrap_or(fields.len() - 1)
                } else {
                    (index + 1) % fields.len()
                };
                self.focused_field = fields[next];
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
        if event.keystroke.modifiers.control
            || event.keystroke.modifiers.platform
            || event.keystroke.modifiers.alt
        {
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

    pub(super) fn visible_fields(&self) -> Vec<FocusedField> {
        match self.page {
            ActivePage::Library if self.session.is_none() => {
                vec![FocusedField::Email, FocusedField::Password]
            }
            ActivePage::Library => vec![FocusedField::Search],
            ActivePage::Downloads => vec![],
            ActivePage::Settings => {
                let mut fields = vec![FocusedField::Output];
                if self.settings.experimental_parallel_downloads {
                    fields.push(FocusedField::Workers);
                }
                if self.settings.proxy.mode == ProxyMode::Manual {
                    fields.push(FocusedField::ProxyAddress);
                }
                fields
            }
        }
    }

    pub(super) fn current_value(&self) -> &str {
        match self.focused_field {
            FocusedField::Email => &self.email,
            FocusedField::Password => &self.password,
            FocusedField::Output => &self.output_text,
            FocusedField::Workers => &self.workers_text,
            FocusedField::ProxyAddress => &self.proxy_address,
            FocusedField::Search => &self.search_text,
        }
    }

    pub(super) fn current_value_mut(&mut self) -> &mut String {
        match self.focused_field {
            FocusedField::Email => &mut self.email,
            FocusedField::Password => &mut self.password,
            FocusedField::Output => &mut self.output_text,
            FocusedField::Workers => &mut self.workers_text,
            FocusedField::ProxyAddress => &mut self.proxy_address,
            FocusedField::Search => &mut self.search_text,
        }
    }

    pub(super) fn insert_text(&mut self, text: &str) {
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

    pub(super) fn delete_selection_or_char(&mut self, forward: bool) {
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

    pub(super) fn sync_settings(&mut self) {
        if !self.output_text.trim().is_empty() {
            self.settings.output_directory = PathBuf::from(self.output_text.trim());
        }
        if let Ok(workers) = self.workers_text.parse::<usize>() {
            self.settings.workers = workers.clamp(2, 16);
        }
        self.settings.proxy.address = self.proxy_address.trim().to_string();
        self.save_settings();
    }

    pub(super) fn save_settings(&self) {
        if self.persist_settings {
            settings::save(&self.settings);
        }
    }

    pub(super) fn set_theme(&mut self, theme: ThemePreference, cx: &mut Context<Self>) {
        self.settings.theme = theme;
        self.save_settings();
        cx.notify();
    }

    pub(super) fn set_language(&mut self, language: LanguagePreference, cx: &mut Context<Self>) {
        let old_locale = self.locale();
        let old_books_message = self.books_loaded_message();
        self.settings.language = language;
        if self.status == old_books_message {
            self.status = self.books_loaded_message();
        } else {
            for key in [
                Text::SignInPrompt,
                Text::EmptyCredentials,
                Text::SigningIn,
                Text::Refreshing,
                Text::SelectBooks,
                Text::QueueRunning,
                Text::QueueFinished,
                Text::PauseRequested,
                Text::ResumeRequested,
                Text::ProxySettingSaved,
            ] {
                if self.status == i18n::text(old_locale, key) {
                    self.status = self.text(key).to_string();
                    break;
                }
            }
        }
        self.save_settings();
        cx.notify();
    }

    pub(super) fn set_proxy_mode(&mut self, mode: ProxyMode, cx: &mut Context<Self>) {
        self.settings.proxy.mode = mode;
        self.sync_settings();
        self.status = self.text(Text::ProxySettingSaved).to_string();
        cx.notify();
    }

    pub(super) fn set_proxy_protocol(&mut self, protocol: ProxyProtocol, cx: &mut Context<Self>) {
        self.settings.proxy.protocol = protocol;
        self.sync_settings();
        self.status = self.text(Text::ProxySettingSaved).to_string();
        cx.notify();
    }

    pub(super) fn start_login(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            self.load_demo_library();
            self.page = ActivePage::Library;
            cx.notify();
            return;
        }
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
        let sender = self.event_sender();
        cx.notify();
        thread::spawn(move || {
            let version = api::latest_version(&proxy);
            let result = api::login(&email, &password, version, &proxy).and_then(|session| {
                api::purchased_books(&session, &proxy).map(|books| (session, books))
            });
            sender.send(UiEvent::LoginDone(result));
        });
    }

    pub(super) fn refresh_books(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            self.load_demo_library();
            cx.notify();
            return;
        }
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
        let sender = self.event_sender();
        cx.notify();
        thread::spawn(move || {
            sender.send(UiEvent::BooksDone(api::purchased_books(&session, &proxy)));
        });
    }

    pub(super) fn toggle_all(&mut self, cx: &mut Context<Self>) {
        let should_select = self.rows.iter().any(|row| !row.selected);
        for row in &mut self.rows {
            row.selected = should_select;
        }
        cx.notify();
    }

    pub(super) fn toggle_book(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(row) = self.row_mut(id) {
            row.selected = !row.selected;
        }
        cx.notify();
    }

    pub(super) fn toggle_decrypt(&mut self, cx: &mut Context<Self>) {
        self.settings.auto_decrypt = !self.settings.auto_decrypt;
        self.save_settings();
        cx.notify();
    }

    pub(super) fn set_parallel_downloads(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.settings.experimental_parallel_downloads = enabled;
        if !enabled && self.focused_field == FocusedField::Workers {
            self.focused_field = FocusedField::Output;
            self.cursor = self.output_text.chars().count();
            self.select_all = false;
        }
        self.save_settings();
        cx.notify();
    }

    pub(super) fn toggle_keep_encrypted(&mut self, cx: &mut Context<Self>) {
        self.settings.keep_encrypted_original = !self.settings.keep_encrypted_original;
        self.save_settings();
        cx.notify();
    }

    pub(super) fn navigate(&mut self, item: NavItem, cx: &mut Context<Self>) {
        self.page = match item {
            NavItem::Library => ActivePage::Library,
            NavItem::Downloads => ActivePage::Downloads,
            NavItem::Settings => ActivePage::Settings,
        };
        if let Some(field) = self.visible_fields().first() {
            self.focused_field = *field;
            self.cursor = self.current_value().chars().count();
        }
        self.select_all = false;
        cx.notify();
    }

    pub(super) fn logout(&mut self, cx: &mut Context<Self>) {
        if let Some(control) = self.download_control.take() {
            control.cancel();
        }
        self.epoch += 1;
        self.busy = false;
        self.session = None;
        self.rows.clear();
        self.queued_download_ids.clear();
        self.email.clear();
        self.password.clear();
        self.search_text.clear();
        self.focused_field = FocusedField::Email;
        self.cursor = 0;
        self.page = ActivePage::Library;
        self.status = self.text(Text::SignInPrompt).to_string();
        cx.notify();
    }

    pub(super) fn pause_downloads(&mut self, cx: &mut Context<Self>) {
        if let Some(control) = &self.download_control {
            control.set_paused(None, true);
            self.advance_demo_queue();
            self.sync_queue_states();
            self.status = self.text(Text::PauseRequested).to_string();
            cx.notify();
        }
    }

    pub(super) fn resume_downloads(&mut self, cx: &mut Context<Self>) {
        if let Some(control) = &self.download_control {
            control.set_paused(None, false);
            self.advance_demo_queue();
            self.sync_queue_states();
            self.status = self.text(Text::ResumeRequested).to_string();
            cx.notify();
        }
    }

    pub(super) fn toggle_task_pause(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(queue) = &self.download_control {
            let paused = queue.task_state(id) == Some(TaskState::Paused);
            queue.set_paused(Some(id), !paused);
            self.advance_demo_queue();
            self.sync_task_state(id);
            cx.notify();
        }
    }

    pub(super) fn clear_completed_downloads(&mut self, cx: &mut Context<Self>) {
        for row in &self.rows {
            if row.state.is_completed() && !row.deleting {
                self.queued_download_ids.remove(&row.book.id());
            }
        }
        cx.notify();
    }

    pub(super) fn choose_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

    pub(super) fn start_downloads(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            self.status = self.text(Text::QueueRunning).to_string();
            cx.notify();
            return;
        }
        if self.demo {
            self.start_demo_downloads(cx);
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
            .filter(|row| row.selected && !row.state.is_active() && !row.deleting)
            .map(|row| {
                row.state = DownloadState::Queued;
                row.received = 0;
                row.total = 0;
                row.file_action_error = None;
                row.book.clone()
            })
            .collect();
        if books.is_empty() {
            self.status = self.text(Text::SelectBooks).to_string();
            cx.notify();
            return;
        }
        self.sync_settings();
        let workers = self.settings.download_workers();
        let control = Arc::new(DownloadQueue::new(books.iter().map(Book::id)));
        self.download_control = Some(Arc::clone(&control));
        for book in &books {
            self.queued_download_ids.insert(book.id());
        }
        let all_books = self
            .rows
            .iter()
            .map(|row| row.book.clone())
            .collect::<Vec<_>>();
        self.busy = true;
        self.page = ActivePage::Downloads;
        self.status = match locale {
            Locale::English => format!("Queued {} books · {workers} workers", books.len()),
            Locale::SimplifiedChinese => {
                format!("{} 本书已加入队列 · {workers} 个线程", books.len())
            }
        };
        cx.notify();
        jobs::start_queue(
            session,
            books,
            export_stems(&all_books),
            self.settings.clone(),
            control,
            self.event_sender(),
            locale,
        );
    }
}
