use super::*;

impl AppView {
    pub(super) fn input_is_composing(&self, cx: &App) -> bool {
        self.inputs[self.focused_field.index()]
            .read(cx)
            .is_composing()
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

    pub(super) fn field_value(&self, field: FocusedField) -> &str {
        match field {
            FocusedField::Email => &self.email,
            FocusedField::Password => &self.password,
            FocusedField::Output => &self.output_text,
            FocusedField::Workers => &self.workers_text,
            FocusedField::ProxyAddress => &self.proxy_address,
            FocusedField::Search => &self.search_text,
        }
    }

    pub(super) fn sync_input(&self, field: FocusedField, cx: &mut Context<Self>) {
        let text = self.field_value(field).to_string();
        self.inputs[field.index()].update(cx, |input, cx| input.set_text(text, cx));
    }

    pub(super) fn input_event(
        &mut self,
        field: FocusedField,
        event: &InputEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Changed(text) => {
                match field {
                    FocusedField::Email => self.email = text.clone(),
                    FocusedField::Password => self.password = text.clone(),
                    FocusedField::Output => self.output_text = text.clone(),
                    FocusedField::Workers => self.workers_text = text.clone(),
                    FocusedField::ProxyAddress => self.proxy_address = text.clone(),
                    FocusedField::Search => self.search_text = text.clone(),
                }
                if matches!(
                    field,
                    FocusedField::Output | FocusedField::Workers | FocusedField::ProxyAddress
                ) {
                    self.sync_settings();
                }
            }
            InputEvent::Focused => self.focused_field = field,
            InputEvent::Tab(reverse) => {
                let fields = self.visible_fields();
                if !fields.is_empty() {
                    let index = fields
                        .iter()
                        .position(|candidate| *candidate == field)
                        .unwrap_or(0);
                    let next = if *reverse {
                        index.checked_sub(1).unwrap_or(fields.len() - 1)
                    } else {
                        (index + 1) % fields.len()
                    };
                    self.focused_field = fields[next];
                    self.pending_input_focus = Some(fields[next]);
                }
            }
            InputEvent::Submit if field == FocusedField::Password => self.start_login(cx),
            InputEvent::Submit => {}
        }
        cx.notify();
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
            self.load_demo_library(cx);
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
            self.load_demo_library(cx);
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
            self.pending_input_focus = Some(FocusedField::Output);
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
            self.pending_input_focus = Some(*field);
        }
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
        for field in [
            FocusedField::Email,
            FocusedField::Password,
            FocusedField::Search,
        ] {
            self.sync_input(field, cx);
        }
        self.pending_input_focus = Some(FocusedField::Email);
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
            self.sync_input(FocusedField::Output, cx);
            window.focus(
                &self.inputs[FocusedField::Output.index()]
                    .read(cx)
                    .focus_handle
                    .clone(),
            );
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
