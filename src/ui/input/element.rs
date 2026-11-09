use super::*;
use gpui::{
    ContentMask, Element, ElementId, ElementInputHandler, GlobalElementId, InspectorElementId,
    LayoutId, PaintQuad, ShapedLine, Style, TextRun, UnderlineStyle, fill, point, relative,
};
use unicode_segmentation::UnicodeSegmentation;

pub(super) struct TextLayout {
    pub line: ShapedLine,
    pub bounds: Bounds<Pixels>,
    pub origin: Point<Pixels>,
    offsets: Vec<(usize, usize)>,
}

impl TextLayout {
    fn display_index(&self, source: usize) -> usize {
        self.offsets
            .iter()
            .rev()
            .find(|(index, _)| *index <= source)
            .map(|(_, display)| *display)
            .unwrap_or(0)
    }

    pub fn index_for_point(&self, point: Point<Pixels>) -> usize {
        let display = self.line.closest_index_for_x(point.x - self.origin.x);
        self.offsets
            .iter()
            .rev()
            .find(|(_, index)| *index <= display)
            .map(|(source, _)| *source)
            .unwrap_or(0)
    }

    pub fn bounds_for_range(&self, range: Range<usize>) -> Bounds<Pixels> {
        let x = |index| {
            (self.origin.x + self.line.x_for_index(self.display_index(index)))
                .clamp(self.bounds.left(), self.bounds.right() - px(1.))
        };
        let start = x(range.start);
        let end = x(range.end).max(start + px(1.));
        Bounds::from_corners(
            point(start, self.bounds.top()),
            point(end, self.bounds.bottom()),
        )
    }
}

pub(super) struct TextElement {
    pub input: Entity<TextInput>,
}

pub(super) struct PrepaintState {
    layout: TextLayout,
    selection: Option<PaintQuad>,
    caret: PaintQuad,
}

impl IntoElement for TextElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = px(20.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> PrepaintState {
        let input = self.input.read(cx);
        let text = &input.buffer.text;
        let placeholder = text.is_empty();
        let mut offsets = vec![(0, 0)];
        let mut display = String::new();
        for (index, grapheme) in text.grapheme_indices(true) {
            display.push_str(if input.masked { "•" } else { grapheme });
            offsets.push((index + grapheme.len(), display.len()));
        }
        if placeholder {
            display = input.placeholder.to_string();
        }
        let display_index = |source| {
            offsets
                .iter()
                .rev()
                .find(|(index, _)| *index <= source)
                .map(|(_, index)| *index)
                .unwrap_or(0)
        };
        let text_style = window.text_style();
        let run = TextRun {
            len: display.len(),
            font: text_style.font(),
            color: if placeholder {
                input.palette.muted.into()
            } else {
                input.palette.text.into()
            },
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = if let Some(marked) = &input.buffer.marked {
            let start = display_index(marked.start);
            let end = display_index(marked.end);
            vec![
                TextRun {
                    len: start,
                    ..run.clone()
                },
                TextRun {
                    len: end - start,
                    underline: Some(UnderlineStyle {
                        color: Some(input.palette.focus.into()),
                        thickness: px(1.),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: display.len() - end,
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect::<Vec<_>>()
        } else {
            vec![run]
        };
        let line = window.text_system().shape_line(
            display.into(),
            text_style.font_size.to_pixels(window.rem_size()),
            &runs,
            None,
        );
        let caret_x = line.x_for_index(display_index(input.buffer.selection.head));
        let mut scroll = input.scroll_x;
        if caret_x < scroll {
            scroll = caret_x;
        }
        if caret_x + px(2.) > scroll + bounds.size.width {
            scroll = caret_x + px(2.) - bounds.size.width;
        }
        scroll = scroll.clamp(
            px(0.),
            (line.width + px(2.) - bounds.size.width).max(px(0.)),
        );
        let origin = point(bounds.left() - scroll, bounds.top());
        let range = input.buffer.selection.range();
        let selection = (!range.is_empty()).then(|| {
            fill(
                Bounds::from_corners(
                    point(
                        origin.x + line.x_for_index(display_index(range.start)),
                        bounds.top(),
                    ),
                    point(
                        origin.x + line.x_for_index(display_index(range.end)),
                        bounds.bottom(),
                    ),
                ),
                gpui::Rgba {
                    a: 0.25,
                    ..input.palette.accent
                },
            )
        });
        let caret = fill(
            Bounds::new(
                point(origin.x + caret_x, bounds.top()),
                size(px(1.5), bounds.size.height),
            ),
            input.palette.focus,
        );
        PrepaintState {
            layout: TextLayout {
                line,
                bounds,
                origin,
                offsets,
            },
            selection,
            caret,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        prepaint: &mut PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        let input = self.input.downgrade();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
            if phase == gpui::DispatchPhase::Bubble {
                let _ = input.update(cx, |input, cx| input.mouse_move(event, window, cx));
            }
        });
        let focused = focus.is_focused(window);
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            if focused && let Some(selection) = &prepaint.selection {
                window.paint_quad(selection.clone());
            }
            prepaint
                .layout
                .line
                .paint(prepaint.layout.origin, px(20.), window, cx)
                .ok();
            if focused {
                window.paint_quad(prepaint.caret.clone());
            }
        });
        self.input.update(cx, |input, _| {
            input.scroll_x = bounds.left() - prepaint.layout.origin.x;
            input.layout = Some(TextLayout {
                line: prepaint.layout.line.clone(),
                bounds,
                origin: prepaint.layout.origin,
                offsets: prepaint.layout.offsets.clone(),
            });
        });
    }
}
