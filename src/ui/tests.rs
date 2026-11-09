use super::*;
use gpui::{ScrollDelta, ScrollWheelEvent, TestAppContext, VisualTestContext, point};
use std::path::Path;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

struct TestFileActions {
    revealed: Mutex<Vec<PathBuf>>,
    deleted: Mutex<Vec<PathBuf>>,
    recycled: PathBuf,
    outcome: DeleteOutcome,
    fail_delete: AtomicBool,
}

impl FileActions for TestFileActions {
    fn reveal(&self, path: &Path, _cx: &App) -> Result<()> {
        anyhow::ensure!(path.is_file(), "file missing");
        self.revealed.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }

    fn delete(&self, path: &Path) -> Result<DeleteOutcome> {
        self.deleted.lock().unwrap().push(path.to_path_buf());
        anyhow::ensure!(!self.fail_delete.load(Ordering::Acquire), "file is locked");
        match self.outcome {
            DeleteOutcome::Trashed => std::fs::rename(path, &self.recycled)?,
            DeleteOutcome::PermanentlyDeleted => std::fs::remove_file(path)?,
            DeleteOutcome::AlreadyMissing => {}
        }
        Ok(self.outcome)
    }
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    for _ in 0..12 {
        cx.refresh().unwrap();
        let viewport = cx.debug_bounds("settings-scroll");
        if let Some(bounds) = cx.debug_bounds(selector) {
            if let Some(viewport) = viewport
                && bounds.center().x >= viewport.left()
                && (bounds.center().y < viewport.top() || bounds.center().y > viewport.bottom())
            {
                scroll_settings(
                    cx,
                    if bounds.center().y < viewport.top() {
                        200.
                    } else {
                        -200.
                    },
                );
                continue;
            }
            cx.simulate_click(bounds.center(), gpui::Modifiers::none());
            return;
        }
        if viewport.is_none() {
            break;
        }
        scroll_settings(cx, -200.);
    }
    panic!("missing or unreachable control {selector}");
}

fn scroll_settings(cx: &mut VisualTestContext, delta: f32) {
    cx.refresh().unwrap();
    let viewport = cx.debug_bounds("settings-scroll").unwrap();
    cx.simulate_event(ScrollWheelEvent {
        position: viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.), px(delta))),
        ..Default::default()
    });
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
            vec![FocusedField::Output, FocusedField::ProxyAddress]
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
    cx.simulate_keystrokes("secondary-a backspace");
    view.update(cx, |view, _| assert!(view.search_text.is_empty()));
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
    assert!(control.has_paused());
    click(cx, "Resume all");
    assert!(!control.has_paused());
    view.update(cx, |view, _| {
        assert_eq!(view.rows[0].state, DownloadState::Downloading)
    });
    click(cx, "nav-settings");
    click(cx, "language-zh-cn");
    click(cx, "nav-downloads");
    click(cx, "全部暂停");
    assert!(control.has_paused());
    view.update(cx, |view, _| {
        assert_eq!(view.rows[0].state, DownloadState::Paused);
        assert_eq!(view.rows[0].state.label(view.locale()), "已暂停");
    });
    click(cx, "全部继续");
    assert!(!control.has_paused());
    click(cx, "全部暂停");
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
fn task_pause_skips_to_next_book_and_clear_completed_preserves_exports(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| AppView::demo("downloads", cx));
    cx.simulate_resize(size(px(960.), px(640.)));
    let directory = tempfile::tempdir().unwrap();
    let exported = directory.path().join("finished.epub");
    std::fs::write(&exported, b"exported book").unwrap();
    view.update(cx, |view, _| {
        view.rows[2].state = DownloadState::Finished {
            path: exported.clone(),
            decrypted: true,
            cleanup_warning: None,
        };
    });
    click(cx, "Clear completed");
    assert_eq!(std::fs::read(&exported).unwrap(), b"exported book");
    view.update(cx, |view, _| {
        assert_eq!(view.queued_download_ids.len(), 2);
        assert_eq!(view.rows.len(), 6);
        assert!(view.busy);
    });
    click(cx, "Resume all");
    click(cx, "task-control-0");
    view.update(cx, |view, _| {
        assert_eq!(view.rows[0].state, DownloadState::Paused);
        assert_eq!(view.rows[0].received, 1_920_000);
        assert_eq!(view.rows[1].state, DownloadState::Downloading);
    });
    click(cx, "task-control-0");
    view.update(cx, |view, _| {
        assert_eq!(view.rows[0].state, DownloadState::Queued);
        assert_eq!(view.rows[1].state, DownloadState::Downloading);
    });
    // Pause a pending item independently, then switch the active item.
    click(cx, "task-control-0");
    click(cx, "task-control-1");
    click(cx, "task-control-0");
    view.update(cx, |view, _| {
        assert_eq!(view.rows[0].state, DownloadState::Downloading);
        assert_eq!(view.rows[1].state, DownloadState::Paused);
    });
    cx.refresh().unwrap();
    let root = cx.debug_bounds("app-root").unwrap();
    for selector in [
        "task-control-0",
        "task-control-1",
        "Pause all",
        "Resume all",
    ] {
        let bounds = cx.debug_bounds(selector).unwrap();
        assert!(bounds.left() >= root.left());
        assert!(bounds.right() <= root.right());
        assert!(bounds.bottom() <= root.bottom());
    }
    // Clearing the last completed entries leaves an empty queue view.
    view.update(cx, |view, cx| {
        view.download_control.as_ref().unwrap().finish("demo-1");
        view.download_control.as_ref().unwrap().finish("demo-2");
        for row in view.rows.iter_mut().take(2) {
            row.state = DownloadState::Finished {
                path: exported.clone(),
                decrypted: true,
                cleanup_warning: None,
            };
        }
        view.handle_event(UiEvent::QueueDone);
        cx.notify();
    });
    click(cx, "Clear completed");
    view.update(cx, |view, _| assert!(view.queued_download_ids.is_empty()));
    cx.refresh().unwrap();
    assert!(cx.debug_bounds("empty-downloads").is_some());
    assert!(exported.exists());
}

#[gpui::test]
fn single_thread_and_source_retention_settings_are_explicit_and_localized(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, cx| AppView::demo("settings", cx));
    cx.simulate_resize(size(px(1180.), px(820.)));
    view.update(cx, |view, _| {
        assert_eq!(view.settings.download_workers(), 1);
        assert!(!view.settings.keep_encrypted_original);
        assert!(!view.visible_fields().contains(&FocusedField::Workers));
    });
    click(cx, "download-parallel");
    click(cx, "field-3");
    // GPUI maps secondary to Command on macOS and Control on Windows/Linux.
    cx.simulate_keystrokes("secondary-a");
    view.update(cx, |view, _| {
        assert!(
            view.select_all,
            "the native select-all shortcut must select the field"
        );
    });
    cx.simulate_input("6");
    view.update(cx, |view, _| {
        assert_eq!(
            view.workers_text, "6",
            "typing must replace the selected value"
        );
        assert_eq!(view.settings.download_workers(), 6);
    });
    click(cx, "keep-encrypted");
    view.update(cx, |view, _| assert!(view.settings.keep_encrypted_original));
    click(cx, "download-single");
    view.update(cx, |view, _| {
        assert_eq!(view.settings.download_workers(), 1);
        assert_eq!(view.settings.workers, 6);
        assert_ne!(view.focused_field, FocusedField::Workers);
    });
    scroll_settings(cx, 1000.);
    click(cx, "language-zh-cn");
    click(cx, "keep-encrypted");
    click(cx, "auto-decrypt");
    view.update(cx, |view, _| {
        assert!(!view.settings.keep_encrypted_original);
        assert!(!view.settings.auto_decrypt);
        assert_eq!(view.text(Text::SingleThread), "单线程");
        assert_eq!(view.text(Text::ClearCompleted), "清除已完成");
    });
}

#[gpui::test]
fn completed_download_file_controls_reveal_delete_and_preserve_library(cx: &mut TestAppContext) {
    for outcome in [
        DeleteOutcome::Trashed,
        DeleteOutcome::PermanentlyDeleted,
        DeleteOutcome::AlreadyMissing,
    ] {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("书名 - Author's Book.epub");
        let source = directory.path().join("书名 - Author's Book.books.epub");
        std::fs::write(&output, b"exported book").unwrap();
        std::fs::write(&source, b"retained encrypted original").unwrap();
        let backend = Arc::new(TestFileActions {
            revealed: Mutex::new(Vec::new()),
            deleted: Mutex::new(Vec::new()),
            recycled: directory.path().join("recycled.epub"),
            outcome,
            fail_delete: AtomicBool::new(false),
        });
        let (view, cx) = cx.add_window_view(|_, cx| AppView::demo("downloads", cx));
        cx.simulate_resize(size(px(960.), px(640.)));
        view.update(cx, |view, cx| {
            view.file_actions = backend.clone();
            view.rows[2].state = DownloadState::Finished {
                path: output.clone(),
                decrypted: true,
                cleanup_warning: None,
            };
            cx.notify();
        });
        cx.refresh().unwrap();
        let row = cx.debug_bounds("book-row-2").unwrap();
        for selector in ["reveal-file-2", "delete-file-2"] {
            let bounds = cx.debug_bounds(selector).unwrap();
            assert!(bounds.left() >= row.left());
            assert!(bounds.right() <= row.right());
            assert!(bounds.top() >= row.top());
            assert!(bounds.bottom() <= row.bottom());
        }
        click(cx, "reveal-file-2");
        assert_eq!(*backend.revealed.lock().unwrap(), vec![output.clone()]);
        // Removing a book externally produces a useful error without losing
        // the record. Deletion can still clear the missing output state.
        if outcome == DeleteOutcome::AlreadyMissing {
            std::fs::remove_file(&output).unwrap();
            click(cx, "reveal-file-2");
            view.update(cx, |view, _| {
                assert!(
                    view.rows[2]
                        .file_action_error
                        .as_ref()
                        .unwrap()
                        .contains("Could not show file")
                )
            });
        }
        click(cx, "delete-file-2");
        cx.run_until_parked();
        view.update(cx, |view, _| {
            assert_eq!(view.rows[2].state, DownloadState::Deleted(outcome));
            assert!(!view.rows[2].deleting);
            assert!(view.rows[2].file_action_error.is_none());
            assert_eq!(view.rows.len(), 6);
            assert!(
                view.busy,
                "deleting a completed file must not stop other downloads"
            );
        });
        assert_eq!(*backend.deleted.lock().unwrap(), vec![output.clone()]);
        assert!(!output.exists());
        assert_eq!(
            std::fs::read(&source).unwrap(),
            b"retained encrypted original"
        );
        if outcome == DeleteOutcome::Trashed {
            assert_eq!(std::fs::read(&backend.recycled).unwrap(), b"exported book");
        }
        click(cx, "Clear completed");
        view.update(cx, |view, _| {
            assert_eq!(view.queued_download_ids.len(), 2);
            assert_eq!(view.rows.len(), 6);
        });
    }
}

#[gpui::test]
fn file_deletion_errors_can_be_retried_and_inflight_deletion_cannot_be_queued_twice(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("finished.epub");
    std::fs::write(&output, b"exported book").unwrap();
    let backend = Arc::new(TestFileActions {
        revealed: Mutex::new(Vec::new()),
        deleted: Mutex::new(Vec::new()),
        recycled: directory.path().join("recycled.epub"),
        outcome: DeleteOutcome::Trashed,
        fail_delete: AtomicBool::new(true),
    });
    let (view, cx) = cx.add_window_view(|_, cx| AppView::demo("downloads", cx));
    cx.simulate_resize(size(px(1180.), px(820.)));
    view.update(cx, |view, cx| {
        view.file_actions = backend.clone();
        view.rows[2].state = DownloadState::Finished {
            path: output.clone(),
            decrypted: true,
            cleanup_warning: None,
        };
        view.set_language(LanguagePreference::SimplifiedChinese, cx);
    });
    click(cx, "delete-file-2");
    cx.run_until_parked();
    view.update(cx, |view, _| {
        assert!(matches!(view.rows[2].state, DownloadState::Finished { .. }));
        assert!(!view.rows[2].deleting);
        assert!(
            view.rows[2]
                .file_action_error
                .as_ref()
                .unwrap()
                .contains("文件删除失败")
        );
    });
    assert_eq!(std::fs::read(&output).unwrap(), b"exported book");
    backend.fail_delete.store(false, Ordering::Release);
    view.update(cx, |view, cx| {
        view.delete_download("demo-3", cx);
        view.delete_download("demo-3", cx);
        assert!(view.rows[2].deleting);
        view.clear_completed_downloads(cx);
        assert!(view.queued_download_ids.contains("demo-3"));
        view.rows[2].selected = true;
        view.download_control = None;
        view.busy = false;
        view.start_downloads(cx);
        assert!(view.rows[2].deleting);
        assert!(matches!(view.rows[2].state, DownloadState::Finished { .. }));
    });
    cx.run_until_parked();
    view.update(cx, |view, _| {
        assert_eq!(
            view.rows[2].state,
            DownloadState::Deleted(DeleteOutcome::Trashed)
        );
        assert_eq!(view.rows[2].state.label(view.locale()), "已移至回收站");
    });
    assert_eq!(
        backend.deleted.lock().unwrap().len(),
        2,
        "one failed attempt and one successful retry"
    );
    assert!(!output.exists());
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
