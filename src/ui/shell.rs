use super::*;

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.start_listener(window, cx);
        let account_label = self
            .session
            .as_ref()
            .map(|session| session.email.clone())
            .unwrap_or_else(|| self.text(Text::NotSignedIn).to_string());
        let page_title = match self.page {
            ActivePage::Library => self.text(Text::LibraryNav),
            ActivePage::Downloads => self.text(Text::DownloadsNav),
            ActivePage::Settings => self.text(Text::SettingsNav),
        };
        let page_subtitle = match self.page {
            ActivePage::Library if self.session.is_none() => {
                self.text(Text::SignInPrompt).to_string()
            }
            ActivePage::Library => match self.locale() {
                Locale::English => format!("{} books in your collection", self.rows.len()),
                Locale::SimplifiedChinese => format!("已购书库 · 共 {} 本", self.rows.len()),
            },
            ActivePage::Downloads => self.text(Text::DownloadsSubtitle).to_string(),
            ActivePage::Settings => self.text(Text::SettingsSubtitle).to_string(),
        };

        div()
            .id("app-root")
            .debug_selector(|| "app-root".to_string())
            .size_full()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event, _, cx| this.handle_key(event, cx)))
            .flex()
            .bg(rgb(0xf3f6f4))
            .text_color(rgb(0x253630))
            .child(
                div()
                    .id("sidebar")
                    .w(px(208.))
                    .flex_shrink_0()
                    .h_full()
                    .px_4()
                    .py_5()
                    .flex()
                    .flex_col()
                    .bg(rgb(0x20332e))
                    .text_color(rgb(0xf4f7f5))
                    .child(
                        div()
                            .px_2()
                            .pb_6()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(19.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child("Endao"),
                            )
                            .child(div().text_xs().text_color(rgb(0xaab9b2)).child("BOOKS")),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(Self::nav_button(
                                "nav-library",
                                self.text(Text::LibraryNav),
                                self.page == ActivePage::Library,
                                cx.listener(|this, _, _, cx| this.navigate(NavItem::Library, cx)),
                            ))
                            .child(Self::nav_button(
                                "nav-downloads",
                                self.text(Text::DownloadsNav),
                                self.page == ActivePage::Downloads,
                                cx.listener(|this, _, _, cx| this.navigate(NavItem::Downloads, cx)),
                            ))
                            .child(Self::nav_button(
                                "nav-settings",
                                self.text(Text::SettingsNav),
                                self.page == ActivePage::Settings,
                                cx.listener(|this, _, _, cx| this.navigate(NavItem::Settings, cx)),
                            )),
                    )
                    .child(div().flex_grow())
                    .child(
                        div()
                            .px_2()
                            .py_3()
                            .border_t_1()
                            .border_color(rgb(0x43564f))
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0xaab9b2))
                                    .child(self.text(Text::Account)),
                            )
                            .child(div().text_sm().text_ellipsis().child(account_label))
                            .children(if self.session.is_some() {
                                Some(Self::button(
                                    self.text(Text::Logout),
                                    false,
                                    cx.listener(|this, _, _, cx| this.logout(cx)),
                                ))
                            } else {
                                None
                            }),
                    ),
            )
            .child(
                div()
                    .h_full()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .flex_grow()
                    .child(
                        div()
                            .h(px(76.))
                            .px_7()
                            .flex()
                            .items_center()
                            .justify_between()
                            .bg(rgb(0xffffff))
                            .border_b_1()
                            .border_color(rgb(0xe0e7e3))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_size(px(20.))
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .child(page_title),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(0x718079))
                                            .whitespace_nowrap()
                                            .child(page_subtitle),
                                    ),
                            ),
                    )
                    .child(div().flex_grow().min_h(px(0.)).px_7().py_5().children(
                        match self.page {
                            ActivePage::Library => Some(self.render_library(cx).into_any_element()),
                            ActivePage::Downloads => {
                                Some(self.render_downloads(cx).into_any_element())
                            }
                            ActivePage::Settings => {
                                Some(self.render_settings(cx).into_any_element())
                            }
                        },
                    ))
                    .child(
                        div()
                            .id("status-line")
                            .debug_selector(|| "status-line".to_string())
                            .h(px(38.))
                            .px_7()
                            .flex()
                            .items_center()
                            .border_t_1()
                            .border_color(rgb(0xe1e8e4))
                            .bg(rgb(0xf8faf9))
                            .text_xs()
                            .text_color(rgb(0x687770))
                            .child(self.status.clone()),
                    ),
            )
    }
}

pub fn run(demo_page: Option<String>) {
    Application::new().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1180.), px(820.)), cx);
        let opened = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(960.), px(640.))),
                app_id: Some("com.endao.books".to_string()),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("Endao Books".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title("Endao Books");
                cx.new(|cx| {
                    if let Some(page) = demo_page.as_deref() {
                        AppView::demo(page, cx)
                    } else {
                        AppView::new(cx)
                    }
                })
            },
        );
        if let Err(error) = opened {
            eprintln!("Could not open Endao window: {error:#}");
            cx.quit();
            return;
        }
        cx.activate(true);
    });
}
