use super::*;
use gpui::{
    ElementInputHandler, InputHandler, Modifiers, TestAppContext, VisualTestContext, point,
};

fn focus_input(input: &Entity<TextInput>, cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.focus(&input.read(cx).focus_handle.clone()));
    cx.refresh().unwrap();
}

fn input_point(
    input: &Entity<TextInput>,
    index: usize,
    cx: &mut VisualTestContext,
) -> Point<Pixels> {
    cx.refresh().unwrap();
    input.read_with(cx, |input, _| {
        let bounds = input
            .layout
            .as_ref()
            .unwrap()
            .bounds_for_range(index..index);
        point(bounds.left(), bounds.center().y)
    })
}

#[gpui::test]
fn native_handler_replaces_preedit_commits_chinese_and_positions_candidates(
    cx: &mut TestAppContext,
) {
    let (input, cx) = cx.add_window_view(|_, cx| TextInput::new("A😀".into(), 0, false, false, cx));
    cx.simulate_resize(size(px(320.), px(100.)));
    focus_input(&input, cx);
    let mut handler = ElementInputHandler::new(Bounds::default(), input.clone());
    cx.update(|window, cx| {
        handler.replace_and_mark_text_in_range(None, "zhong", Some(5..5), window, cx);
        assert_eq!(handler.marked_text_range(window, cx), Some(3..8));
        assert_eq!(
            handler
                .selected_text_range(false, window, cx)
                .unwrap()
                .range,
            8..8
        );
        handler.replace_and_mark_text_in_range(None, "zhongwen", Some(8..8), window, cx);
    });
    cx.refresh().unwrap();
    // Enter belongs to the input method while composing. It must not submit
    // the form or add another ASCII character.
    cx.simulate_keystrokes("enter");
    input.read_with(cx, |input, _| {
        assert_eq!(input.buffer.text, "A😀zhongwen");
        assert_eq!(input.buffer.committed, "A😀");
    });
    cx.update(|window, cx| {
        let bounds = handler.bounds_for_range(11..11, window, cx).unwrap();
        assert!(bounds.size.height > px(0.));
        assert!(bounds.left() >= px(0.) && bounds.right() <= px(320.));
        let offset = handler
            .character_index_for_point(bounds.origin, window, cx)
            .unwrap();
        assert_eq!(offset, 11);
        handler.replace_text_in_range(None, "中文", window, cx);
        assert!(handler.marked_text_range(window, cx).is_none());
        assert_eq!(
            handler
                .selected_text_range(false, window, cx)
                .unwrap()
                .range,
            5..5
        );
        let mut adjusted = None;
        assert_eq!(
            handler
                .text_for_range(1..5, &mut adjusted, window, cx)
                .unwrap(),
            "😀中文"
        );
        assert_eq!(adjusted, Some(1..5));
    });
    input.read_with(cx, |input, _| assert_eq!(input.buffer.committed, "A😀中文"));
    cx.refresh().unwrap();
    cx.simulate_keystrokes("backspace");
    cx.simulate_input("字");
    input.read_with(cx, |input, _| assert_eq!(input.buffer.text, "A😀中字"));
}

#[gpui::test]
fn mouse_drag_shift_click_and_word_selection_replace_the_selected_text(cx: &mut TestAppContext) {
    let (input, cx) =
        cx.add_window_view(|_, cx| TextInput::new("alpha beta 中文".into(), 0, false, false, cx));
    cx.simulate_resize(size(px(400.), px(100.)));
    focus_input(&input, cx);
    let start = input_point(&input, 6, cx);
    let end = input_point(&input, 10, cx);
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(end, Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    input.read_with(cx, |input, _| assert_eq!(input.selection(), 6..10));
    cx.simulate_keystrokes("secondary-c");
    cx.update(|_, cx| assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), "beta"));
    cx.simulate_input("书籍");
    input.read_with(cx, |input, _| {
        assert_eq!(input.buffer.text, "alpha 书籍 中文")
    });
    let first = input_point(&input, 0, cx);
    let next = input_point(&input, 5, cx);
    cx.simulate_click(first, Modifiers::none());
    cx.simulate_click(
        next,
        Modifiers {
            shift: true,
            ..Modifiers::none()
        },
    );
    input.read_with(cx, |input, _| assert_eq!(input.selection(), 0..5));
    cx.simulate_keystrokes("secondary-x");
    input.read_with(cx, |input, _| assert_eq!(input.buffer.text, " 书籍 中文"));
    cx.simulate_keystrokes("secondary-v");
    input.read_with(cx, |input, _| {
        assert_eq!(input.buffer.text, "alpha 书籍 中文")
    });
    let start = input_point(&input, 5, cx);
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    let outside = point(px(-20.), start.y);
    cx.simulate_mouse_move(outside, Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(outside, MouseButton::Left, Modifiers::none());
    input.read_with(cx, |input, _| {
        assert_eq!(
            input.selection(),
            0..5,
            "dragging out of the field keeps extending the selection"
        );
        assert!(!input.dragging);
    });
    let word = input_point(&input, 2, cx);
    cx.simulate_event(MouseDownEvent {
        position: word,
        button: MouseButton::Left,
        click_count: 2,
        ..Default::default()
    });
    input.read_with(cx, |input, _| assert_eq!(input.selection(), 0..5));
}

#[gpui::test]
fn shift_arrows_word_navigation_and_long_line_scrolling_work(cx: &mut TestAppContext) {
    let (input, cx) =
        cx.add_window_view(|_, cx| TextInput::new("alpha beta".into(), 0, false, false, cx));
    cx.simulate_resize(size(px(180.), px(80.)));
    focus_input(&input, cx);
    cx.simulate_keystrokes("home ctrl-shift-right");
    input.read_with(cx, |input, _| assert_eq!(input.selection(), 0..5));
    cx.simulate_input("你好");
    cx.simulate_keystrokes("end ctrl-shift-left");
    input.read_with(cx, |input, _| assert_eq!(input.selection(), 7..11));
    cx.simulate_keystrokes("backspace");
    cx.simulate_input("e\u{301}👨‍👩‍👧‍👦");
    cx.simulate_keystrokes("shift-left backspace");
    input.read_with(cx, |input, _| {
        assert_eq!(input.buffer.text, "你好 e\u{301}")
    });
    cx.simulate_keystrokes("shift-left");
    input.read_with(cx, |input, _| assert_eq!(input.selection(), 7..10));
    cx.simulate_input("abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz");
    cx.refresh().unwrap();
    input.read_with(cx, |input, _| {
        assert!(input.scroll_x > px(0.));
        let layout = input.layout.as_ref().unwrap();
        let caret =
            layout.bounds_for_range(input.buffer.selection.head..input.buffer.selection.head);
        assert!(caret.left() >= layout.bounds.left());
        assert!(caret.right() <= layout.bounds.right());
    });
}

#[gpui::test]
fn password_masking_disables_copy_and_cut_and_keeps_selection_geometry(cx: &mut TestAppContext) {
    let (input, cx) =
        cx.add_window_view(|_, cx| TextInput::new("密😀e\u{301}".into(), 1, true, false, cx));
    cx.simulate_resize(size(px(180.), px(80.)));
    focus_input(&input, cx);
    cx.update(|_, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string("clipboard sentinel".into()))
    });
    cx.simulate_keystrokes("secondary-a secondary-c secondary-x");
    input.read_with(cx, |input, _| {
        assert_eq!(input.buffer.text, "密😀e\u{301}");
        assert_eq!(input.layout.as_ref().unwrap().line.text.as_ref(), "•••");
    });
    cx.update(|_, cx| {
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().unwrap(),
            "clipboard sentinel"
        )
    });
    cx.simulate_input("新密码");
    input.read_with(cx, |input, _| assert_eq!(input.buffer.text, "新密码"));
}

#[test]
fn preedit_updates_replace_previous_preedit_and_commit_only_the_candidate() {
    let mut buffer = Buffer::new("prefix ".into(), false);
    buffer.replace(None, "ni", true, Some(2..2));
    assert_eq!(buffer.text, "prefix ni");
    assert_eq!(buffer.committed, "prefix ");
    buffer.replace(None, "nihao", true, Some(5..5));
    assert_eq!(buffer.text, "prefix nihao");
    assert_eq!(buffer.selection.head, "prefix nihao".len());
    buffer.replace(None, "你好", false, None);
    assert_eq!(buffer.text, "prefix 你好");
    assert_eq!(buffer.committed, "prefix 你好");
    assert!(buffer.marked.is_none());
}

#[test]
fn utf16_ranges_and_relative_preedit_selection_handle_surrogate_pairs() {
    let mut buffer = Buffer::new("A😀中文B".into(), false);
    assert_eq!(buffer.range_from_utf16(1..3), 1..5);
    assert_eq!(buffer.range_from_utf16(2..3), 1..5);
    buffer.replace(Some(3..5), "你好🌏", true, Some(2..4));
    assert_eq!(buffer.text, "A😀你好🌏B");
    assert_eq!(buffer.marked, Some(5..15));
    assert_eq!(buffer.selection.range(), 11..15);
    assert_eq!(buffer.range_to_utf16(buffer.selection.range()), 5..7);
    buffer.replace(None, "地球", false, None);
    assert_eq!(buffer.text, "A😀地球B");
}

#[test]
fn deletion_respects_grapheme_clusters_and_selection_direction() {
    let mut buffer = Buffer::new("a👨‍👩‍👧‍👦e\u{301}中文".into(), false);
    buffer.delete(false, false);
    buffer.delete(false, false);
    assert_eq!(buffer.text, "a👨‍👩‍👧‍👦e\u{301}");
    buffer.delete(false, false);
    assert_eq!(buffer.text, "a👨‍👩‍👧‍👦");
    buffer.selection.move_to(1, true);
    assert!(buffer.selection.head < buffer.selection.anchor);
    buffer.replace(None, "你好", false, None);
    assert_eq!(buffer.text, "a你好");
}

#[test]
fn numeric_fields_reject_invalid_commits_and_cancellation_clears_marked_text() {
    let mut buffer = Buffer::new("4".into(), true);
    buffer.selection = Selection { anchor: 0, head: 1 };
    buffer.replace(None, "zhong", true, None);
    assert_eq!(buffer.committed, "4");
    buffer.replace(None, "中", false, None);
    assert_eq!(buffer.text, "4");
    buffer.replace(Some(0..1), "6", false, None);
    assert_eq!(buffer.text, "6");
    let mut text = Buffer::new("".into(), false);
    text.replace(None, "pinyin", true, None);
    text.replace(None, "", true, None);
    assert!(text.text.is_empty());
    assert!(text.marked.is_none());
}
