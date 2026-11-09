mod buffer;
mod element;
#[cfg(test)]
mod tests;

use crate::theme::Palette;
use buffer::{Buffer, Selection, to_utf16};
use element::{TextElement, TextLayout};
use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, Entity, EntityInputHandler, EventEmitter,
    FocusHandle, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Point, Render,
    SharedString, Subscription, UTF16Selection, Window, div, prelude::*, px, size,
};
use std::ops::Range;

pub(super) enum InputEvent {
    Changed(String),
    Focused,
    Tab(bool),
    Submit,
}

pub(super) struct TextInput {
    buffer: Buffer,
    pub focus_handle: FocusHandle,
    placeholder: SharedString,
    palette: Palette,
    masked: bool,
    id: usize,
    dragging: bool,
    layout: Option<TextLayout>,
    scroll_x: Pixels,
    blur_subscription: Option<Subscription>,
}

impl EventEmitter<InputEvent> for TextInput {}

impl TextInput {
    pub fn is_composing(&self) -> bool {
        self.buffer.marked.is_some()
    }
    #[cfg(test)]
    pub fn selection(&self) -> Range<usize> {
        self.buffer.selection.range()
    }
    pub fn new(
        text: String,
        id: usize,
        masked: bool,
        digits_only: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            buffer: Buffer::new(text, digits_only),
            focus_handle: cx.focus_handle(),
            placeholder: "".into(),
            palette: Palette::light(),
            masked,
            id,
            dragging: false,
            layout: None,
            scroll_x: px(0.),
            blur_subscription: None,
        }
    }

    pub fn configure(
        &mut self,
        placeholder: &'static str,
        palette: Palette,
        cx: &mut Context<Self>,
    ) {
        if self.placeholder.as_ref() != placeholder || self.palette != palette {
            self.placeholder = placeholder.into();
            self.palette = palette;
            cx.notify();
        }
    }

    pub fn set_text(&mut self, text: String, cx: &mut Context<Self>) {
        self.buffer.set_text(text);
        self.layout = None;
        self.scroll_x = px(0.);
        cx.notify();
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        if self.buffer.marked.is_none() {
            cx.emit(InputEvent::Changed(self.buffer.committed.clone()));
        }
        self.layout = None;
        cx.notify();
    }

    fn mouse_index(&self, position: Point<Pixels>) -> usize {
        self.layout
            .as_ref()
            .map(|layout| layout.index_for_point(position))
            .unwrap_or(self.buffer.text.len())
    }

    fn mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus_handle);
        cx.emit(InputEvent::Focused);
        let was_marked = self.buffer.marked.is_some();
        self.buffer.commit();
        if was_marked {
            cx.emit(InputEvent::Changed(self.buffer.committed.clone()));
        }
        let index = self.mouse_index(event.position);
        match event.click_count {
            2 => {
                let word = self.buffer.word_at(index);
                self.buffer.selection = Selection {
                    anchor: word.start,
                    head: word.end,
                };
            }
            3.. => {
                self.buffer.selection = Selection {
                    anchor: 0,
                    head: self.buffer.text.len(),
                }
            }
            _ => self.buffer.selection.move_to(index, event.modifiers.shift),
        }
        self.dragging = true;
        window.prevent_default();
        cx.stop_propagation();
        cx.notify();
    }

    fn mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.dragging && event.dragging() {
            self.buffer
                .selection
                .move_to(self.mouse_index(event.position), true);
            cx.notify();
        }
    }

    fn handle_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        // The platform input method owns keys while it is composing, including
        // candidate navigation and Enter. Printable text arrives via the IME API.
        if self.buffer.marked.is_some() {
            return;
        }
        let key = event.keystroke.key.as_str();
        let modifiers = event.keystroke.modifiers;
        let mut handled = true;
        if modifiers.secondary() && !modifiers.alt && matches!(key, "a" | "c" | "x" | "v") {
            match key {
                "a" => {
                    self.buffer.selection = Selection {
                        anchor: 0,
                        head: self.buffer.text.len(),
                    }
                }
                "c" | "x" => {
                    let range = self.buffer.selection.range();
                    if !range.is_empty() && !self.masked {
                        cx.write_to_clipboard(ClipboardItem::new_string(
                            self.buffer.text[range].to_string(),
                        ));
                        if key == "x" {
                            self.buffer.replace(None, "", false, None);
                            self.changed(cx);
                        }
                    }
                }
                "v" => {
                    if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                        self.buffer.replace(None, &text, false, None);
                        self.changed(cx);
                    }
                }
                _ => unreachable!(),
            }
        } else {
            let word = modifiers.control || (cfg!(target_os = "macos") && modifiers.alt);
            let line = cfg!(target_os = "macos") && modifiers.platform;
            let head = self.buffer.selection.head;
            match key {
                "left" | "arrowleft" | "right" | "arrowright" | "home" | "end" | "up" | "down" => {
                    let right = matches!(key, "right" | "arrowright" | "end" | "down");
                    let range = self.buffer.selection.range();
                    let offset = if matches!(key, "home" | "end" | "up" | "down") || line {
                        if right { self.buffer.text.len() } else { 0 }
                    } else if !modifiers.shift && !range.is_empty() && !word {
                        if right { range.end } else { range.start }
                    } else if word {
                        if right {
                            self.buffer.next_word(head)
                        } else {
                            self.buffer.previous_word(head)
                        }
                    } else if right {
                        self.buffer.next_grapheme(head)
                    } else {
                        self.buffer.previous_grapheme(head)
                    };
                    self.buffer.selection.move_to(offset, modifiers.shift);
                }
                "backspace" | "delete" => {
                    self.buffer.delete(key == "delete", word);
                    self.changed(cx);
                }
                "tab" => cx.emit(InputEvent::Tab(modifiers.shift)),
                "enter" => cx.emit(InputEvent::Submit),
                _ => handled = false,
            }
        }
        if handled {
            window.prevent_default();
            cx.stop_propagation();
            cx.notify();
        }
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.buffer.range_from_utf16(range);
        *adjusted = Some(self.buffer.range_to_utf16(range.clone()));
        Some(self.buffer.text[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.buffer.range_to_utf16(self.buffer.selection.range()),
            reversed: self.buffer.selection.head < self.buffer.selection.anchor,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.buffer
            .marked
            .clone()
            .map(|range| self.buffer.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.buffer.commit();
        self.changed(cx);
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !text.is_empty() && text.chars().all(char::is_control) {
            return;
        }
        self.buffer.replace(range, text, false, None);
        self.changed(cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.buffer.replace(range, text, true, selected);
        self.changed(cx);
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self.buffer.range_from_utf16(range);
        Some(
            self.layout
                .as_ref()
                .map(|layout| layout.bounds_for_range(range))
                .unwrap_or_else(|| Bounds::new(bounds.origin, size(px(2.), bounds.size.height))),
        )
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(to_utf16(
            &self.buffer.text,
            self.layout.as_ref()?.index_for_point(point),
        ))
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.blur_subscription.is_none() {
            self.blur_subscription =
                Some(
                    cx.on_blur(&self.focus_handle.clone(), window, |this, _, cx| {
                        this.dragging = false;
                        if this.buffer.marked.is_some() {
                            this.buffer.commit();
                            this.changed(cx);
                        }
                        cx.notify();
                    }),
                );
        }
        let focused = self.focus_handle.is_focused(window);
        let palette = self.palette;
        let id = self.id;
        div()
            .id(("field", id))
            .debug_selector(move || format!("field-{id}"))
            .track_focus(&self.focus_handle)
            .h(px(38.))
            .w_full()
            .px_3()
            .flex()
            .items_center()
            .rounded_md()
            .border_1()
            .border_color(if focused {
                palette.focus
            } else {
                palette.border
            })
            .bg(palette.input)
            .text_color(palette.text)
            .text_sm()
            .line_height(px(20.))
            .cursor(CursorStyle::IBeam)
            .overflow_hidden()
            .on_key_down(cx.listener(Self::handle_key))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::mouse_down))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.dragging = false),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.dragging = false),
            )
            .child(TextElement { input: cx.entity() })
    }
}
