use super::*;
use gpui::{TestAppContext, VisualTestContext};

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    cx.refresh().unwrap();
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("missing control {selector}"));
    cx.simulate_click(bounds.center(), gpui::Modifiers::none());
}

#[gpui::test]
fn pages_login_selection_and_settings_support_mouse_and_keyboard(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| AppView::demo("login", cx));
    cx.simulate_resize(size(px(1180.), px(820.)));
    click(cx, "field-0");
    cx.simulate_input("reader@example.test");
    cx.simulate_keystrokes("tab");
    cx.simulate_input("demo-password");
    view.update(cx, |view, _| {
        assert_eq!(view.email, "reader@example.test");
        assert_eq!(view.password, "demo-password");
        assert_eq!(
            view.visible_fields(),
            vec![FocusedField::Email, FocusedField::Password]
        );
    });
    click(cx, "Sign in");
    view.update(cx, |view, _| {
        assert!(view.session.is_some());
        assert_eq!(view.page, ActivePage::Library);
        assert_eq!(view.rows.len(), 6);
    });
    click(cx, "Select all");
    view.update(cx, |view, _| {
        assert!(view.rows.iter().all(|row| row.selected))
    });
    click(cx, "select-0");
    view.update(cx, |view, _| {
        assert!(!view.rows[0].selected);
        assert!(view.rows[1].selected);
    });
    click(cx, "field-5");
    cx.simulate_input("Coast");
    view.update(cx, |view, _| assert_eq!(view.search_text, "Coast"));
    click(cx, "nav-settings");
    view.update(cx, |view, _| assert_eq!(view.page, ActivePage::Settings));
    click(cx, "language-zh-cn");
    click(cx, "proxy-manual");
    click(cx, "field-4");
    cx.simulate_input("127.0.0.1:1080");
    click(cx, "proxy-socks5");
    view.update(cx, |view, _| {
        assert_eq!(
            view.settings.language,
            LanguagePreference::SimplifiedChinese
        );
        assert_eq!(view.settings.proxy.mode, ProxyMode::Manual);
        assert_eq!(view.settings.proxy.protocol, ProxyProtocol::Socks5);
        assert_eq!(view.settings.proxy.address, "127.0.0.1:1080");
        assert_eq!(
            view.visible_fields(),
            vec![
                FocusedField::Output,
                FocusedField::Workers,
                FocusedField::ProxyAddress
            ]
        );
    });
    // Focus cycles within the current page, including reverse tab.
    cx.simulate_keystrokes("tab");
    view.update(cx, |view, _| {
        assert_eq!(view.focused_field, FocusedField::Output)
    });
    cx.simulate_keystrokes("shift-tab");
    view.update(cx, |view, _| {
        assert_eq!(view.focused_field, FocusedField::ProxyAddress)
    });
    click(cx, "nav-library");
    click(cx, "field-5");
    cx.simulate_keystrokes("ctrl-a backspace");
    click(cx, "退出登录");
    view.update(cx, |view, _| {
        assert!(view.session.is_none());
        assert!(view.rows.is_empty());
        assert!(view.email.is_empty());
        assert!(view.password.is_empty());
        assert_eq!(view.page, ActivePage::Library);
    });
    assert!(cx.debug_bounds("field-0").is_some());
}

#[gpui::test]
fn queue_pause_resume_language_switch_and_logout_cancel(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| AppView::demo("downloads", cx));
    cx.simulate_resize(size(px(1180.), px(820.)));
    let control = view.update(cx, |view, _| view.download_control.clone().unwrap());
    assert!(control.is_paused());
    click(cx, "Resume");
    assert!(!control.is_paused());
    view.update(cx, |view, _| {
        assert_eq!(view.rows[0].state, DownloadState::Downloading)
    });
    click(cx, "nav-settings");
    click(cx, "language-zh-cn");
    click(cx, "nav-downloads");
    click(cx, "暂停");
    assert!(control.is_paused());
    view.update(cx, |view, _| {
        assert_eq!(view.rows[0].state, DownloadState::Paused);
        assert_eq!(view.rows[0].state.label(view.locale()), "已暂停");
    });
    click(cx, "继续");
    assert!(!control.is_paused());
    click(cx, "暂停");
    let sender = view.update(cx, |view, _| view.event_sender());
    click(cx, "退出登录");
    assert!(control.is_cancelled());
    // Simulate a late completion from the old account's worker.
    sender.send(UiEvent::Message("stale account event".to_string()));
    cx.run_until_parked();
    view.update(cx, |view, _| {
        assert!(view.session.is_none());
        assert!(!view.busy);
        assert!(view.download_control.is_none());
        assert!(view.queued_download_ids.is_empty());
        assert_ne!(view.status, "stale account event");
    });
}

#[gpui::test]
fn compact_window_keeps_book_list_and_controls_inside_content(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| AppView::demo("library", cx));
    cx.simulate_resize(size(px(960.), px(640.)));
    cx.refresh().unwrap();
    let root = cx.debug_bounds("app-root").unwrap();
    for selector in [
        "book-list",
        "field-5",
        "Download selected (0)",
        "status-line",
    ] {
        let bounds = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("missing {selector}"));
        assert!(
            bounds.origin.x >= root.origin.x,
            "{selector} outside left edge"
        );
        assert!(
            bounds.origin.x + bounds.size.width <= root.origin.x + root.size.width,
            "{selector} outside right edge"
        );
        assert!(
            bounds.origin.y + bounds.size.height <= root.origin.y + root.size.height,
            "{selector} outside bottom edge"
        );
    }
    click(cx, "nav-downloads");
    view.update(cx, |view, _| assert_eq!(view.page, ActivePage::Downloads));
}

#[gpui::test]
fn theme_controls_follow_system_changes_and_keep_manual_overrides(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| AppView::demo("settings", cx));
    cx.simulate_resize(size(px(1180.), px(820.)));
    click(cx, "theme-dark");
    view.update(cx, |view, cx| {
        assert!(view.appearance_subscription.is_some());
        assert_eq!(view.settings.theme, ThemePreference::Dark);
        view.system_appearance_changed(WindowAppearance::Light, cx);
        assert_eq!(view.palette(), Palette::dark());
    });
    click(cx, "theme-system");
    view.update(cx, |view, cx| {
        assert_eq!(view.settings.theme, ThemePreference::System);
        assert_eq!(view.palette(), Palette::light());
        view.system_appearance_changed(WindowAppearance::VibrantDark, cx);
        assert_eq!(view.palette(), Palette::dark());
        view.system_appearance_changed(WindowAppearance::VibrantLight, cx);
        assert_eq!(view.palette(), Palette::light());
    });
    click(cx, "theme-light");
    view.update(cx, |view, cx| {
        view.system_appearance_changed(WindowAppearance::Dark, cx);
        assert_eq!(view.settings.theme, ThemePreference::Light);
        assert_eq!(view.palette(), Palette::light());
    });
    click(cx, "language-zh-cn");
    view.update(cx, |view, _| {
        assert_eq!(view.text(Text::Appearance), "外观");
        assert_eq!(view.text(Text::DarkTheme), "深色");
        assert_eq!(view.text(Text::LightTheme), "浅色");
    });
    click(cx, "theme-dark");
    click(cx, "nav-library");
    click(cx, "nav-downloads");
    view.update(cx, |view, _| assert_eq!(view.palette(), Palette::dark()));
    click(cx, "退出登录");
    view.update(cx, |view, _| {
        assert!(view.session.is_none());
        assert_eq!(view.settings.theme, ThemePreference::Dark);
        assert_eq!(view.palette(), Palette::dark());
    });
}
