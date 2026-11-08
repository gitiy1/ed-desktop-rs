use super::*;

impl AppView {
    pub(super) fn settings_section(
        &self,
        title: &'static str,
        content: impl IntoElement,
    ) -> impl IntoElement {
        let palette = self.palette();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .py_5()
            .border_b_1()
            .border_color(palette.border)
            .child(
                div()
                    .h(px(20.))
                    .flex_shrink_0()
                    .text_xs()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(palette.secondary)
                    .child(title),
            )
            .child(content)
    }

    pub(super) fn nav_button(
        &self,
        id: &'static str,
        label: &'static str,
        selected: bool,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let palette = self.palette();
        div()
            .id(id)
            .debug_selector(|| id.to_string())
            .w_full()
            .px_3()
            .py_3()
            .rounded_md()
            .bg(if selected {
                palette.nav_selected
            } else {
                palette.sidebar
            })
            .text_sm()
            .text_color(if selected {
                palette.sidebar_text
            } else {
                palette.sidebar_muted
            })
            .cursor_pointer()
            .hover(move |style| style.bg(palette.nav_hover).text_color(palette.sidebar_text))
            .child(label)
            .on_click(on_click)
    }

    pub(super) fn button(
        &self,
        label: impl Into<SharedString>,
        primary: bool,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let palette = self.palette();
        let label = label.into();
        let debug_label = label.to_string();
        let background = if primary {
            palette.accent
        } else {
            palette.button
        };
        let foreground = if primary {
            palette.on_accent
        } else {
            palette.text
        };
        div()
            .id(label.clone())
            .debug_selector(move || debug_label.clone())
            .flex_shrink_0()
            .px_4()
            .py_2()
            .rounded_md()
            .bg(background)
            .text_color(foreground)
            .text_sm()
            .text_center()
            .cursor_pointer()
            .hover(move |style| {
                style.bg(if primary {
                    palette.accent_hover
                } else {
                    palette.button_hover
                })
            })
            .child(label)
            .on_click(on_click)
    }

    pub(super) fn segmented(
        &self,
        id: &'static str,
        label: impl Into<SharedString>,
        selected: bool,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let palette = self.palette();
        div()
            .id(id)
            .debug_selector(|| id.to_string())
            .h(px(34.))
            .px_3()
            .flex()
            .items_center()
            .rounded_md()
            .border_1()
            .border_color(if selected {
                palette.accent
            } else {
                palette.border
            })
            .bg(if selected {
                palette.accent
            } else {
                palette.surface
            })
            .text_sm()
            .text_color(if selected {
                palette.on_accent
            } else {
                palette.text
            })
            .cursor_pointer()
            .child(label.into())
            .on_click(on_click)
    }

    pub(super) fn input_field(
        &self,
        spec: InputFieldSpec,
        on_mouse_down: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let palette = self.palette();
        let has_value = !spec.value.is_empty();
        let mut shown = if !has_value {
            spec.placeholder.to_string()
        } else if spec.masked {
            "•".repeat(spec.value.chars().count())
        } else {
            spec.value
        };
        if spec.focused && has_value {
            let byte_index = char_to_byte_index(&shown, spec.cursor);
            shown.insert(byte_index, '|');
        }
        let shown_color = if !has_value {
            palette.muted
        } else {
            palette.text
        };
        div()
            .flex()
            .flex_col()
            .flex_grow()
            .when(spec.field == FocusedField::Workers, |field| {
                field.flex_none().w(px(100.)).flex_shrink_0()
            })
            .min_w(if spec.field == FocusedField::ProxyAddress {
                px(220.)
            } else {
                px(100.)
            })
            .gap_1()
            .children(if spec.label.is_empty() {
                None
            } else {
                Some(
                    div()
                        .h(px(18.))
                        .flex_shrink_0()
                        .text_xs()
                        .text_color(palette.secondary)
                        .child(spec.label),
                )
            })
            .child(
                div()
                    .id((
                        "field",
                        match spec.field {
                            FocusedField::Email => 0usize,
                            FocusedField::Password => 1,
                            FocusedField::Output => 2,
                            FocusedField::Workers => 3,
                            FocusedField::ProxyAddress => 4,
                            FocusedField::Search => 5,
                        },
                    ))
                    .debug_selector(|| {
                        format!(
                            "field-{}",
                            match spec.field {
                                FocusedField::Email => 0,
                                FocusedField::Password => 1,
                                FocusedField::Output => 2,
                                FocusedField::Workers => 3,
                                FocusedField::ProxyAddress => 4,
                                FocusedField::Search => 5,
                            }
                        )
                    })
                    .h(px(38.))
                    .px_3()
                    .flex()
                    .items_center()
                    .rounded_md()
                    .border_1()
                    .border_color(if spec.focused {
                        palette.focus
                    } else {
                        palette.border
                    })
                    .bg(palette.input)
                    .text_sm()
                    .overflow_hidden()
                    .text_color(shown_color)
                    .child(shown)
                    .on_click(on_mouse_down),
            )
    }
}
