use super::*;

impl AppView {
    pub(super) fn render_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette();
        div()
            .id("settings-scroll")
            .debug_selector(|| "settings-scroll".to_string())
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
                        .child(self.input_field(
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
                        .child(self.button(
                            self.text(Text::Browse),
                            false,
                            cx.listener(|this, _, window, cx| this.choose_folder(window, cx)),
                        )),
                ),
            )
            .child(
                self.settings_section(
                    self.text(Text::Preferences),
                    div().flex().items_end().justify_between().child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(palette.secondary)
                                    .child(self.text(Text::Language)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .child(self.segmented(
                                        "language-system",
                                        self.text(Text::FollowSystem),
                                        self.settings.language == LanguagePreference::System,
                                        cx.listener(|this, _, _, cx| {
                                            this.set_language(LanguagePreference::System, cx)
                                        }),
                                    ))
                                    .child(self.segmented(
                                        "language-english",
                                        self.text(Text::English),
                                        self.settings.language == LanguagePreference::English,
                                        cx.listener(|this, _, _, cx| {
                                            this.set_language(LanguagePreference::English, cx)
                                        }),
                                    ))
                                    .child(self.segmented(
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
                    ),
                ),
            )
            .child(
                self.settings_section(
                    self.text(Text::Appearance),
                    div()
                        .flex()
                        .gap_2()
                        .child(self.segmented(
                            "theme-system",
                            self.text(Text::FollowSystem),
                            self.settings.theme == ThemePreference::System,
                            cx.listener(|this, _, _, cx| {
                                this.set_theme(ThemePreference::System, cx)
                            }),
                        ))
                        .child(self.segmented(
                            "theme-dark",
                            self.text(Text::DarkTheme),
                            self.settings.theme == ThemePreference::Dark,
                            cx.listener(|this, _, _, cx| this.set_theme(ThemePreference::Dark, cx)),
                        ))
                        .child(self.segmented(
                            "theme-light",
                            self.text(Text::LightTheme),
                            self.settings.theme == ThemePreference::Light,
                            cx.listener(|this, _, _, cx| {
                                this.set_theme(ThemePreference::Light, cx)
                            }),
                        )),
                ),
            )
            .child(
                self.settings_section(
                    self.text(Text::DownloadBehavior),
                    div()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .items_end()
                                .gap_2()
                                .child(self.segmented(
                                    "download-single",
                                    self.text(Text::SingleThread),
                                    !self.settings.experimental_parallel_downloads,
                                    cx.listener(|this, _, _, cx| {
                                        this.set_parallel_downloads(false, cx)
                                    }),
                                ))
                                .child(self.segmented(
                                    "download-parallel",
                                    self.text(Text::ExperimentalParallel),
                                    self.settings.experimental_parallel_downloads,
                                    cx.listener(|this, _, _, cx| {
                                        this.set_parallel_downloads(true, cx)
                                    }),
                                ))
                                .children(self.settings.experimental_parallel_downloads.then(
                                    || {
                                        self.input_field(
                                            InputFieldSpec {
                                                field: FocusedField::Workers,
                                                label: self.text(Text::Workers),
                                                value: self.workers_text.clone(),
                                                placeholder: "4",
                                                focused: self.focused_field
                                                    == FocusedField::Workers,
                                                cursor: self.cursor,
                                                masked: false,
                                            },
                                            cx.listener(|this, _, window, cx| {
                                                this.click_field(FocusedField::Workers, window, cx)
                                            }),
                                        )
                                    },
                                )),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(palette.secondary)
                                .child(self.text(Text::DownloadModeHint)),
                        )
                        .child(self.setting_toggle(
                            "auto-decrypt",
                            self.text(Text::AutoDecrypt),
                            None,
                            self.settings.auto_decrypt,
                            cx.listener(|this, _, _, cx| this.toggle_decrypt(cx)),
                        ))
                        .child(self.setting_toggle(
                            "keep-encrypted",
                            self.text(Text::KeepEncrypted),
                            Some(self.text(Text::KeepEncryptedHint)),
                            self.settings.keep_encrypted_original,
                            cx.listener(|this, _, _, cx| this.toggle_keep_encrypted(cx)),
                        ))
                        .child(
                            div()
                                .text_xs()
                                .text_color(palette.muted)
                                .child(self.text(Text::NextQueueHint)),
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
                                .child(self.segmented(
                                    "proxy-system",
                                    self.text(Text::SystemProxy),
                                    self.settings.proxy.mode == ProxyMode::System,
                                    cx.listener(|this, _, _, cx| {
                                        this.set_proxy_mode(ProxyMode::System, cx)
                                    }),
                                ))
                                .child(self.segmented(
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
                                        .text_color(palette.muted)
                                        .child(self.text(Text::SystemProxyFallback)),
                                ),
                        )
                        .children(if self.settings.proxy.mode == ProxyMode::Manual {
                            Some(
                                div()
                                    .flex()
                                    .items_end()
                                    .gap_2()
                                    .child(self.segmented(
                                        "proxy-http",
                                        self.text(Text::HttpProxy),
                                        self.settings.proxy.protocol == ProxyProtocol::Http,
                                        cx.listener(|this, _, _, cx| {
                                            this.set_proxy_protocol(ProxyProtocol::Http, cx)
                                        }),
                                    ))
                                    .child(self.segmented(
                                        "proxy-socks5",
                                        self.text(Text::Socks5Proxy),
                                        self.settings.proxy.protocol == ProxyProtocol::Socks5,
                                        cx.listener(|this, _, _, cx| {
                                            this.set_proxy_protocol(ProxyProtocol::Socks5, cx)
                                        }),
                                    ))
                                    .child(self.input_field(
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
