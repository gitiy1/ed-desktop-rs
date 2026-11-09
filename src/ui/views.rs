use super::*;

impl AppView {
    pub(super) fn render_book_list(
        &self,
        cx: &mut Context<Self>,
        downloads_only: bool,
    ) -> impl IntoElement {
        let palette = self.palette();
        let search = self.search_text.trim().to_lowercase();
        let rows: Vec<_> = self
            .rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| {
                if downloads_only && !self.queued_download_ids.contains(&row.book.id()) {
                    return None;
                }
                let haystack = format!("{} {}", row.book.title(), row.book.author()).to_lowercase();
                if !downloads_only && !search.is_empty() && !haystack.contains(&search) {
                    return None;
                }
                let id = row.book.id();
                let cover = cover_color(&id);
                let selected = row.selected;
                let title = row.book.title();
                let author = row.book.author();
                let status = if row.deleting {
                    self.text(Text::DeletingFile).to_string()
                } else {
                    row.file_action_error
                        .clone()
                        .unwrap_or_else(|| row.state.label(self.locale()))
                };
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
                let speed = if row.speed > 0.0 && row.state == DownloadState::Downloading {
                    format_speed(row.speed)
                } else {
                    String::new()
                };
                Some(
                    div()
                        .id(("book-row", index))
                        .debug_selector(move || format!("book-row-{index}"))
                        .flex()
                        .items_center()
                        .gap_3()
                        .px_4()
                        .py_3()
                        .bg(if selected && !downloads_only {
                            palette.selected
                        } else {
                            palette.surface
                        })
                        .border_b_1()
                        .border_color(palette.border)
                        .child(if downloads_only {
                            div().w(px(28.)).child("").into_any_element()
                        } else {
                            div()
                                .id(("select", index))
                                .debug_selector(move || format!("select-{index}"))
                                .w(px(32.))
                                .h(px(40.))
                                .flex_shrink_0()
                                .flex()
                                .justify_center()
                                .items_center()
                                .text_size(px(20.))
                                .text_color(if selected {
                                    palette.accent
                                } else {
                                    palette.muted
                                })
                                .cursor_pointer()
                                .child(if selected { "☑" } else { "☐" })
                                .on_click(
                                    cx.listener(move |this, _, _, cx| this.toggle_book(&id, cx)),
                                )
                                .into_any_element()
                        })
                        .child(
                            div()
                                .w(px(42.))
                                .h(px(56.))
                                .flex_shrink_0()
                                .flex()
                                .justify_center()
                                .items_center()
                                .rounded_md()
                                .bg(cover)
                                .text_color(rgb(0xfffbef))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_size(px(18.))
                                .child(title.chars().next().unwrap_or('书').to_string()),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(3.))
                                .flex_grow()
                                .min_w(px(0.))
                                .child(
                                    div()
                                        .text_size(px(15.))
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_ellipsis()
                                        .child(title),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(palette.secondary)
                                        .text_ellipsis()
                                        .child(author),
                                ),
                        )
                        .children(if downloads_only {
                            Some(
                                div()
                                    .w(px(228.))
                                    .flex_shrink_0()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(
                                        div()
                                            .w_full()
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .text_xs()
                                            .text_color(if row.file_action_error.is_some() {
                                                palette.danger
                                            } else if matches!(
                                                row.state,
                                                DownloadState::Finished {
                                                    cleanup_warning: Some(_),
                                                    ..
                                                }
                                            ) {
                                                palette.paused
                                            } else {
                                                palette.secondary
                                            })
                                            .child(status),
                                    )
                                    .when(
                                        !matches!(row.state, DownloadState::Deleted(_)),
                                        |status| {
                                            status.child(
                                                div()
                                                    .w_full()
                                                    .h(px(6.))
                                                    .rounded_md()
                                                    .bg(palette.progress_track)
                                                    .child(
                                                        div()
                                                            .h_full()
                                                            .w(relative(progress))
                                                            .rounded_md()
                                                            .bg(
                                                                if row.state
                                                                    == DownloadState::Paused
                                                                {
                                                                    palette.paused
                                                                } else {
                                                                    palette.focus
                                                                },
                                                            ),
                                                    ),
                                            )
                                        },
                                    )
                                    .when(
                                        !matches!(row.state, DownloadState::Deleted(_)),
                                        |status| {
                                            status.child(
                                                div()
                                                    .flex()
                                                    .justify_between()
                                                    .text_xs()
                                                    .text_color(palette.secondary)
                                                    .child(progress_text)
                                                    .child(speed),
                                            )
                                        },
                                    ),
                            )
                        } else {
                            None
                        })
                        .children(if downloads_only {
                            Some(self.render_task_controls(row, index, cx))
                        } else {
                            None
                        }),
                )
            })
            .collect();
        if rows.is_empty() {
            return div()
                .id("book-list")
                .debug_selector(|| "book-list".to_string())
                .flex_grow()
                .min_h(px(140.))
                .flex()
                .justify_center()
                .items_center()
                .text_color(palette.muted)
                .child(self.text(Text::NoBooks));
        }
        div()
            .id("book-list")
            .debug_selector(|| "book-list".to_string())
            .flex_grow()
            .min_h(px(140.))
            .overflow_y_scroll()
            .children(rows)
    }

    pub(super) fn render_login(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette();
        div()
            .size_full()
            .flex()
            .justify_center()
            .items_center()
            .child(
                div()
                    .w(px(460.))
                    .flex()
                    .flex_col()
                    .gap_5()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(24.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(self.text(Text::LoginTitle)),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(palette.secondary)
                                    .child(self.text(Text::LoginSubtitle)),
                            ),
                    )
                    .child(self.input_field(
                        InputFieldSpec {
                            field: FocusedField::Email,
                            label: self.text(Text::Email),
                            placeholder: self.text(Text::EmailPlaceholder),
                        },
                        cx,
                    ))
                    .child(self.input_field(
                        InputFieldSpec {
                            field: FocusedField::Password,
                            label: self.text(Text::Password),
                            placeholder: self.text(Text::PasswordPlaceholder),
                        },
                        cx,
                    ))
                    .child(self.button(
                        self.text(Text::SignIn),
                        true,
                        cx.listener(|this, _, _, cx| this.start_login(cx)),
                    )),
            )
    }

    pub(super) fn render_library(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette();
        if self.session.is_none() {
            return self.render_login(cx).into_any_element();
        }
        let selected = self.rows.iter().filter(|row| row.selected).count();
        let all_selected = !self.rows.is_empty() && self.rows.iter().all(|row| row.selected);
        div()
            .size_full()
            .flex()
            .flex_col()
            .rounded_lg()
            .border_1()
            .border_color(palette.border)
            .bg(palette.surface)
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_5()
                    .py_4()
                    .child(self.input_field(
                        InputFieldSpec {
                            field: FocusedField::Search,
                            label: "",
                            placeholder: self.text(Text::SearchBooks),
                        },
                        cx,
                    ))
                    .child(self.button(
                        self.text(Text::Refresh),
                        false,
                        cx.listener(|this, _, _, cx| this.refresh_books(cx)),
                    )),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_5()
                    .py_3()
                    .border_b_1()
                    .border_color(palette.border)
                    .bg(palette.inset)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(self.button(
                                if all_selected {
                                    self.text(Text::ClearSelection)
                                } else {
                                    self.text(Text::SelectAll)
                                },
                                false,
                                cx.listener(|this, _, _, cx| this.toggle_all(cx)),
                            ))
                            .child(
                                div().text_xs().text_color(palette.secondary).child(format!(
                                    "{selected} {}",
                                    self.text(Text::SelectedCount)
                                )),
                            ),
                    )
                    .child(self.button(
                        format!("{} ({selected})", self.text(Text::DownloadSelected)),
                        true,
                        cx.listener(|this, _, _, cx| this.start_downloads(cx)),
                    )),
            )
            .child(self.render_book_list(cx, false))
            .into_any_element()
    }

    pub(super) fn render_downloads(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette();
        let control = self.download_control.clone();
        let has_downloads = !self.queued_download_ids.is_empty();
        let paused = control
            .as_ref()
            .is_some_and(|control| control.has_paused() && !control.has_unpaused());
        let has_completed = self.rows.iter().any(|row| {
            self.queued_download_ids.contains(&row.book.id())
                && row.state.is_completed()
                && !row.deleting
        });
        let queue_label = match self.locale() {
            Locale::English => format!("{} books in this queue", self.queued_download_ids.len()),
            Locale::SimplifiedChinese => {
                format!("队列中有 {} 本书", self.queued_download_ids.len())
            }
        };
        div()
            .id("downloads-view")
            .debug_selector(|| "downloads-view".to_string())
            .rounded_lg()
            .border_1()
            .border_color(palette.border)
            .bg(palette.surface)
            .overflow_hidden()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .id("downloads-toolbar")
                    .debug_selector(|| "downloads-toolbar".to_string())
                    .px_6()
                    .py_4()
                    .flex()
                    .flex_wrap()
                    .gap_3()
                    .justify_between()
                    .items_center()
                    .border_b_1()
                    .border_color(palette.border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().w(px(8.)).h(px(8.)).rounded_full().bg(if paused {
                                palette.paused
                            } else {
                                palette.focus
                            }))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(palette.secondary)
                                    .child(queue_label),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_2()
                            .children(has_completed.then(|| {
                                self.button(
                                    self.text(Text::ClearCompleted),
                                    false,
                                    cx.listener(|this, _, _, cx| {
                                        this.clear_completed_downloads(cx)
                                    }),
                                )
                            }))
                            .children(
                                control
                                    .as_ref()
                                    .is_some_and(|control| control.has_unpaused())
                                    .then(|| {
                                        self.button(
                                            self.text(Text::PauseAll),
                                            false,
                                            cx.listener(|this, _, _, cx| this.pause_downloads(cx)),
                                        )
                                    }),
                            )
                            .children(
                                control
                                    .as_ref()
                                    .is_some_and(|control| control.has_paused())
                                    .then(|| {
                                        self.button(
                                            self.text(Text::ResumeAll),
                                            true,
                                            cx.listener(|this, _, _, cx| this.resume_downloads(cx)),
                                        )
                                    }),
                            ),
                    ),
            )
            .child(if has_downloads {
                self.render_book_list(cx, true).into_any_element()
            } else {
                div()
                    .id("empty-downloads")
                    .debug_selector(|| "empty-downloads".to_string())
                    .flex_grow()
                    .flex()
                    .justify_center()
                    .items_center()
                    .text_color(palette.muted)
                    .child(self.text(Text::NoDownloads))
                    .into_any_element()
            })
            .children(has_downloads.then(|| {
                div()
                    .px_5()
                    .py_2()
                    .border_t_1()
                    .border_color(palette.border)
                    .text_xs()
                    .text_color(palette.muted)
                    .child(self.text(Text::DeletePolicy))
            }))
    }

    fn render_task_controls(
        &self,
        row: &BookRow,
        index: usize,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let id = row.book.id();
        let controls = div().w(px(112.)).flex_shrink_0().flex().flex_col().gap_1();
        if row.deleting {
            return controls.child(
                div()
                    .text_xs()
                    .text_color(self.palette().muted)
                    .child(self.text(Text::DeletingFile)),
            );
        }
        match row.state {
            DownloadState::Queued | DownloadState::Downloading | DownloadState::Paused => controls
                .child(
                    div()
                        .id(("task-control", index))
                        .debug_selector(move || format!("task-control-{index}"))
                        .child(self.button(
                            if row.state == DownloadState::Paused {
                                self.text(Text::Resume)
                            } else {
                                self.text(Text::Pause)
                            },
                            false,
                            cx.listener(move |this, _, _, cx| this.toggle_task_pause(&id, cx)),
                        )),
                ),
            DownloadState::Finished { .. } => {
                let delete_id = id.clone();
                controls
                    .child(self.file_button(
                        "reveal-file",
                        index,
                        self.text(Text::ShowInFolder),
                        false,
                        cx.listener(move |this, _, _, cx| this.reveal_download(&id, cx)),
                    ))
                    .child(self.file_button(
                        "delete-file",
                        index,
                        self.text(Text::DeleteFile),
                        true,
                        cx.listener(move |this, _, _, cx| this.delete_download(&delete_id, cx)),
                    ))
            }
            _ => controls,
        }
    }
}
