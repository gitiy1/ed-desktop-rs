use super::*;

impl AppView {
    pub(super) fn file_button(
        &self,
        id: &'static str,
        index: usize,
        label: &'static str,
        destructive: bool,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let palette = self.palette();
        div()
            .id((id, index))
            .debug_selector(move || format!("{id}-{index}"))
            .w_full()
            .h(px(30.))
            .flex()
            .items_center()
            .justify_center()
            .rounded_md()
            .bg(palette.button)
            .text_color(if destructive {
                palette.danger
            } else {
                palette.text
            })
            .text_xs()
            .cursor_pointer()
            .hover(move |style| style.bg(palette.button_hover))
            .child(label)
            .on_click(on_click)
    }
    pub(super) fn setting_toggle(
        &self,
        id: &'static str,
        label: &'static str,
        description: Option<&'static str>,
        enabled: bool,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let palette = self.palette();
        div()
            .id(id)
            .debug_selector(|| id.to_string())
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .py_1()
            .cursor_pointer()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w(px(0.))
                    .gap_1()
                    .child(div().text_sm().child(label))
                    .children(
                        description
                            .map(|text| div().text_xs().text_color(palette.secondary).child(text)),
                    ),
            )
            .child(
                div()
                    .w(px(52.))
                    .h(px(30.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(if enabled {
                        palette.accent
                    } else {
                        palette.button
                    })
                    .border_1()
                    .border_color(if enabled {
                        palette.accent
                    } else {
                        palette.border
                    })
                    .text_sm()
                    .text_color(if enabled {
                        palette.on_accent
                    } else {
                        palette.secondary
                    })
                    .child(self.text(if enabled { Text::On } else { Text::Off })),
            )
            .on_click(on_click)
    }

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
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = self.palette();
        let input = self.inputs[spec.field.index()].clone();
        input.update(cx, |input, cx| {
            input.configure(spec.placeholder, palette, cx)
        });
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
            .children((!spec.label.is_empty()).then(|| {
                div()
                    .h(px(18.))
                    .flex_shrink_0()
                    .text_xs()
                    .text_color(palette.secondary)
                    .child(spec.label)
            }))
            .child(input)
    }
}
