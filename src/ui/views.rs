use super::*;

impl AppView {
    pub(super) fn render_book_list(
        &self,
        cx: &mut Context<Self>,
        downloads_only: bool,
    ) -> impl IntoElement {
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
                let status = row.state.label(self.locale());
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
                        .flex()
                        .items_center()
                        .gap_3()
                        .px_4()
                        .py_3()
                        .bg(if selected && !downloads_only {
                            rgb(0xf0f7f3)
                        } else {
                            rgb(0xffffff)
                        })
                        .border_b_1()
                        .border_color(rgb(0xe5eae7))
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
                                    rgb(0x167d70)
                                } else {
                                    rgb(0x87928d)
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
                                        .text_color(rgb(0x718079))
                                        .text_ellipsis()
                                        .child(author),
                                ),
                        )
                        .children(if downloads_only {
                            Some(
                                div()
                                    .w(px(290.))
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
                                            .text_color(rgb(0x607068))
                                            .child(status),
                                    )
                                    .child(
                                        div()
                                            .w_full()
                                            .h(px(6.))
                                            .rounded_md()
                                            .bg(rgb(0xe5ebe8))
                                            .child(
                                                div()
                                                    .h_full()
                                                    .w(relative(progress))
                                                    .rounded_md()
                                                    .bg(if row.state == DownloadState::Paused {
                                                        rgb(0xc29a4b)
                                                    } else {
                                                        rgb(0x168576)
                                                    }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .justify_between()
                                            .text_xs()
                                            .text_color(rgb(0x718079))
                                            .child(progress_text)
                                            .child(speed),
                                    ),
                            )
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
                .text_color(rgb(0x77857f))
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
                                    .text_color(rgb(0x6b7973))
                                    .child(self.text(Text::LoginSubtitle)),
                            ),
                    )
                    .child(Self::input_field(
                        InputFieldSpec {
                            field: FocusedField::Email,
                            label: self.text(Text::Email),
                            value: self.email.clone(),
                            placeholder: self.text(Text::EmailPlaceholder),
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
                            label: self.text(Text::Password),
                            value: self.password.clone(),
                            placeholder: self.text(Text::PasswordPlaceholder),
                            focused: self.focused_field == FocusedField::Password,
                            cursor: self.cursor,
                            masked: true,
                        },
                        cx.listener(|this, _, window, cx| {
                            this.click_field(FocusedField::Password, window, cx)
                        }),
                    ))
                    .child(Self::button(
                        self.text(Text::SignIn),
                        true,
                        cx.listener(|this, _, _, cx| this.start_login(cx)),
                    )),
            )
    }

    pub(super) fn render_library(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
            .border_color(rgb(0xe0e7e3))
            .bg(rgb(0xffffff))
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_5()
                    .py_4()
                    .child(Self::input_field(
                        InputFieldSpec {
                            field: FocusedField::Search,
                            label: "",
                            value: self.search_text.clone(),
                            placeholder: self.text(Text::SearchBooks),
                            focused: self.focused_field == FocusedField::Search,
                            cursor: self.cursor,
                            masked: false,
                        },
                        cx.listener(|this, _, window, cx| {
                            this.click_field(FocusedField::Search, window, cx)
                        }),
                    ))
                    .child(Self::button(
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
                    .border_color(rgb(0xe1e8e4))
                    .bg(rgb(0xf8faf9))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(Self::button(
                                if all_selected {
                                    self.text(Text::ClearSelection)
                                } else {
                                    self.text(Text::SelectAll)
                                },
                                false,
                                cx.listener(|this, _, _, cx| this.toggle_all(cx)),
                            ))
                            .child(
                                div().text_xs().text_color(rgb(0x718079)).child(format!(
                                    "{selected} {}",
                                    self.text(Text::SelectedCount)
                                )),
                            ),
                    )
                    .child(Self::button(
                        format!("{} ({selected})", self.text(Text::DownloadSelected)),
                        true,
                        cx.listener(|this, _, _, cx| this.start_downloads(cx)),
                    )),
            )
            .child(self.render_book_list(cx, false))
            .into_any_element()
    }

    pub(super) fn render_downloads(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let control = self.download_control.clone();
        let has_downloads = !self.queued_download_ids.is_empty();
        let paused = control.as_ref().is_some_and(|control| control.is_paused());
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
            .border_color(rgb(0xe0e7e3))
            .bg(rgb(0xffffff))
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
                    .justify_between()
                    .items_center()
                    .border_b_1()
                    .border_color(rgb(0xe1e8e4))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().w(px(8.)).h(px(8.)).rounded_full().bg(if paused {
                                rgb(0xc29a4b)
                            } else {
                                rgb(0x168576)
                            }))
                            .child(div().text_sm().text_color(rgb(0x52645b)).child(queue_label)),
                    )
                    .child(if self.busy && paused {
                        Self::button(
                            self.text(Text::Resume),
                            true,
                            cx.listener(|this, _, _, cx| this.resume_downloads(cx)),
                        )
                        .into_any_element()
                    } else if self.busy && control.is_some() {
                        Self::button(
                            self.text(Text::Pause),
                            false,
                            cx.listener(|this, _, _, cx| this.pause_downloads(cx)),
                        )
                        .into_any_element()
                    } else {
                        div().into_any_element()
                    }),
            )
            .child(if has_downloads {
                self.render_book_list(cx, true).into_any_element()
            } else {
                div()
                    .flex_grow()
                    .flex()
                    .justify_center()
                    .items_center()
                    .text_color(rgb(0x77857f))
                    .child(self.text(Text::NoDownloads))
                    .into_any_element()
            })
    }

    pub(super) fn render_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("settings-scroll")
            .size_full()
            .overflow_y_scroll()
            .px_6()
            .child(
                self.settings_section(
                    self.text(Text::DownloadFolder),
                    div()
                        .flex()
                        .items_end()
                        .gap_3()
                        .child(Self::input_field(
                            InputFieldSpec {
                                field: FocusedField::Output,
                                label: self.text(Text::SaveTo),
                                value: self.output_text.clone(),
                                placeholder: self.text(Text::FolderPlaceholder),
                                focused: self.focused_field == FocusedField::Output,
                                cursor: self.cursor,
                                masked: false,
                            },
                            cx.listener(|this, _, window, cx| {
                                this.click_field(FocusedField::Output, window, cx)
                            }),
                        ))
                        .child(Self::button(
                            self.text(Text::Browse),
                            false,
                            cx.listener(|this, _, window, cx| this.choose_folder(window, cx)),
                        ))
                        .child(Self::input_field(
                            InputFieldSpec {
                                field: FocusedField::Workers,
                                label: self.text(Text::Workers),
                                value: self.workers_text.clone(),
                                placeholder: "4",
                                focused: self.focused_field == FocusedField::Workers,
                                cursor: self.cursor,
                                masked: false,
                            },
                            cx.listener(|this, _, window, cx| {
                                this.click_field(FocusedField::Workers, window, cx)
                            }),
                        )),
                ),
            )
            .child(
                self.settings_section(
                    self.text(Text::Preferences),
                    div()
                        .flex()
                        .items_end()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(0x687770))
                                        .child(self.text(Text::Language)),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .gap_2()
                                        .child(Self::segmented(
                                            "language-system",
                                            self.text(Text::FollowSystem),
                                            self.settings.language == LanguagePreference::System,
                                            cx.listener(|this, _, _, cx| {
                                                this.set_language(LanguagePreference::System, cx)
                                            }),
                                        ))
                                        .child(Self::segmented(
                                            "language-english",
                                            self.text(Text::English),
                                            self.settings.language == LanguagePreference::English,
                                            cx.listener(|this, _, _, cx| {
                                                this.set_language(LanguagePreference::English, cx)
                                            }),
                                        ))
                                        .child(Self::segmented(
                                            "language-zh-cn",
                                            self.text(Text::SimplifiedChinese),
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
                                .id("auto-decrypt")
                                .h(px(38.))
                                .px_2()
                                .flex()
                                .items_center()
                                .text_sm()
                                .cursor_pointer()
                                .child(if self.settings.auto_decrypt {
                                    format!("☑ {}", self.text(Text::AutoDecrypt))
                                } else {
                                    format!("☐ {}", self.text(Text::AutoDecrypt))
                                })
                                .on_click(cx.listener(|this, _, _, cx| this.toggle_decrypt(cx))),
                        ),
                ),
            )
            .child(
                self.settings_section(
                    self.text(Text::Network),
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(Self::segmented(
                                    "proxy-system",
                                    self.text(Text::SystemProxy),
                                    self.settings.proxy.mode == ProxyMode::System,
                                    cx.listener(|this, _, _, cx| {
                                        this.set_proxy_mode(ProxyMode::System, cx)
                                    }),
                                ))
                                .child(Self::segmented(
                                    "proxy-manual",
                                    self.text(Text::ManualProxy),
                                    self.settings.proxy.mode == ProxyMode::Manual,
                                    cx.listener(|this, _, _, cx| {
                                        this.set_proxy_mode(ProxyMode::Manual, cx)
                                    }),
                                ))
                                .child(
                                    div()
                                        .flex_grow()
                                        .text_xs()
                                        .text_color(rgb(0x75827c))
                                        .child(self.text(Text::SystemProxyFallback)),
                                ),
                        )
                        .children(if self.settings.proxy.mode == ProxyMode::Manual {
                            Some(
                                div()
                                    .flex()
                                    .items_end()
                                    .gap_2()
                                    .child(Self::segmented(
                                        "proxy-http",
                                        self.text(Text::HttpProxy),
                                        self.settings.proxy.protocol == ProxyProtocol::Http,
                                        cx.listener(|this, _, _, cx| {
                                            this.set_proxy_protocol(ProxyProtocol::Http, cx)
                                        }),
                                    ))
                                    .child(Self::segmented(
                                        "proxy-socks5",
                                        self.text(Text::Socks5Proxy),
                                        self.settings.proxy.protocol == ProxyProtocol::Socks5,
                                        cx.listener(|this, _, _, cx| {
                                            this.set_proxy_protocol(ProxyProtocol::Socks5, cx)
                                        }),
                                    ))
                                    .child(Self::input_field(
                                        InputFieldSpec {
                                            field: FocusedField::ProxyAddress,
                                            label: self.text(Text::ProxyAddress),
                                            value: self.proxy_address.clone(),
                                            placeholder: self.text(Text::ProxyAddressPlaceholder),
                                            focused: self.focused_field
                                                == FocusedField::ProxyAddress,
                                            cursor: self.cursor,
                                            masked: false,
                                        },
                                        cx.listener(|this, _, window, cx| {
                                            this.click_field(FocusedField::ProxyAddress, window, cx)
                                        }),
                                    )),
                            )
                        } else {
                            None
                        }),
                ),
            )
    }
}
