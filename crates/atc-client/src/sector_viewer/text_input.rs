//! Single-line editor for the student join form.

use std::ops::Range;

use gpui::{
    canvas, div, fill, point, prelude::*, px, rgb, size, App, Bounds, ClipboardItem, Context,
    CursorStyle, ElementInputHandler, EntityInputHandler, FocusHandle, Focusable, KeyDownEvent,
    Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Point, ShapedLine,
    SharedString, TextRun, UTF16Selection, Window,
};

use super::{BORDER, MUTED, TEXT};

pub(super) struct TextInput {
    focus: FocusHandle,
    placeholder: &'static str,
    value: String,
    anchor: usize,
    cursor: usize,
    marked: Option<Range<usize>>,
    layout: Option<(Bounds<Pixels>, ShapedLine)>,
}

impl TextInput {
    pub(super) fn new(placeholder: &'static str, cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle().tab_stop(true),
            placeholder,
            value: String::new(),
            anchor: 0,
            cursor: 0,
            marked: None,
            layout: None,
        }
    }

    pub(super) fn value(&self) -> &str {
        &self.value
    }

    fn selection(&self) -> Range<usize> {
        self.anchor.min(self.cursor)..self.anchor.max(self.cursor)
    }

    fn move_cursor(&mut self, offset: usize, extend: bool, cx: &mut Context<Self>) {
        self.cursor = offset;
        if !extend {
            self.anchor = offset;
        }
        cx.notify();
    }

    fn replace(&mut self, range: Option<Range<usize>>, text: &str) -> Range<usize> {
        let range = range
            .map(|range| self.utf8_range(range))
            .or_else(|| self.marked.take())
            .unwrap_or_else(|| self.selection());
        let inserted = range.start..range.start + text.len();
        self.value.replace_range(range, text);
        inserted
    }

    fn utf8_range(&self, range: Range<usize>) -> Range<usize> {
        let start = utf8_offset(&self.value, range.start);
        start..utf8_offset(&self.value, range.end).max(start)
    }

    fn utf16_range(&self, range: Range<usize>) -> Range<usize> {
        self.value[..range.start].encode_utf16().count()
            ..self.value[..range.end].encode_utf16().count()
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        let shift = key.modifiers.shift;
        if key.modifiers == Modifiers::secondary_key() {
            let selection = self.selection();
            match key.key.as_str() {
                "a" => {
                    self.anchor = 0;
                    self.move_cursor(self.value.len(), true, cx);
                }
                "c" | "x" => {
                    if !selection.is_empty() {
                        cx.write_to_clipboard(ClipboardItem::new_string(
                            self.value[selection].to_owned(),
                        ));
                        if key.key == "x" {
                            self.replace_text_in_range(None, "", window, cx);
                        }
                    }
                }
                "v" => {
                    if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                        let line = text.trim().replace(['\r', '\n', '\t'], " ");
                        self.replace_text_in_range(None, &line, window, cx);
                    }
                }
                _ => return,
            }
        } else {
            if key.modifiers.control || key.modifiers.platform || key.modifiers.alt {
                return;
            }
            let selection = self.selection();
            let previous = self.value[..self.cursor]
                .char_indices()
                .next_back()
                .map_or(0, |(offset, _)| offset);
            let next = self.cursor
                + self.value[self.cursor..]
                    .chars()
                    .next()
                    .map_or(0, char::len_utf8);
            match key.key.as_str() {
                "left" => self.move_cursor(
                    if !shift && !selection.is_empty() {
                        selection.start
                    } else {
                        previous
                    },
                    shift,
                    cx,
                ),
                "right" => self.move_cursor(
                    if !shift && !selection.is_empty() {
                        selection.end
                    } else {
                        next
                    },
                    shift,
                    cx,
                ),
                "home" => self.move_cursor(0, shift, cx),
                "end" => self.move_cursor(self.value.len(), shift, cx),
                "backspace" | "delete" => {
                    if selection.is_empty() {
                        self.anchor = if key.key == "backspace" {
                            previous
                        } else {
                            next
                        };
                    }
                    self.replace_text_in_range(None, "", window, cx);
                }
                "tab" if shift => window.focus_prev(),
                "tab" => window.focus_next(),
                _ => return,
            }
        }
        cx.stop_propagation();
    }

    fn index_for_point(&self, point: Point<Pixels>) -> usize {
        if let Some((bounds, line)) = &self.layout {
            // A platform callback can arrive between an edit and the next paint.
            let index = line.closest_index_for_x(point.x - bounds.left());
            self.value.floor_char_boundary(index.min(self.value.len()))
        } else {
            self.cursor
        }
    }
}

// The platform handler supplies UTF-16 offsets. Rust strings use UTF-8 bytes.
fn utf8_offset(value: &str, offset: usize) -> usize {
    let mut units = 0;
    for (index, ch) in value.char_indices() {
        if units >= offset {
            return index;
        }
        units += ch.len_utf16();
    }
    value.len()
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.utf8_range(range);
        *actual_range = Some(self.utf16_range(range.clone()));
        self.value.get(range).map(str::to_owned)
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.utf16_range(self.selection()),
            reversed: self.cursor < self.anchor,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.clone().map(|range| self.utf16_range(range))
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked = None;
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if text.contains(['\r', '\n', '\t']) {
            return;
        }
        let inserted = self.replace(range, text);
        self.marked = None;
        self.move_cursor(inserted.end, false, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selection: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if text.contains(['\r', '\n', '\t']) {
            return;
        }
        let inserted = self.replace(range, text);
        let selection = selection.map_or_else(
            || inserted.end..inserted.end,
            |selection| {
                inserted.start + utf8_offset(text, selection.start)
                    ..inserted.start + utf8_offset(text, selection.end)
            },
        );
        self.marked = (!inserted.is_empty()).then_some(inserted);
        self.anchor = selection.start;
        self.move_cursor(selection.end, true, cx);
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let (bounds, line) = self.layout.as_ref()?;
        let range = self.utf8_range(range);
        let x1 = bounds.left() + line.x_for_index(range.start);
        let x2 = bounds.left() + line.x_for_index(range.end);
        Some(Bounds::from_corners(
            point(x1.min(x2), bounds.top()),
            point(x1.max(x2), bounds.bottom()),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(
            self.value[..self.index_for_point(point)]
                .encode_utf16()
                .count(),
        )
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for TextInput {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let empty = self.value.is_empty();
        let display: SharedString = if empty {
            self.placeholder.into()
        } else {
            self.value.clone().into()
        };
        let selection = self.selection();
        let cursor = self.cursor;
        let focus = self.focus.clone();
        let input = cx.entity();
        let line = canvas(
            |_, _, _| (),
            move |bounds, (), window, cx| {
                let focused = focus.is_focused(window);
                window.handle_input(&focus, ElementInputHandler::new(bounds, input.clone()), cx);
                let style = window.text_style();
                let run = TextRun {
                    len: display.len(),
                    font: style.font(),
                    color: rgb(if empty { MUTED } else { TEXT }).into(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                let font_size = style.font_size.to_pixels(window.rem_size());
                let line = window
                    .text_system()
                    .shape_line(display, font_size, &[run], None);
                let scroll = (line.x_for_index(cursor) - bounds.size.width + px(2.)).max(px(0.));
                let origin = bounds.origin - point(scroll, px(0.));
                if focused && !selection.is_empty() {
                    let x1 = origin.x + line.x_for_index(selection.start);
                    let x2 = origin.x + line.x_for_index(selection.end);
                    window.paint_quad(fill(
                        Bounds::from_corners(
                            point(x1.min(x2), bounds.top()),
                            point(x1.max(x2), bounds.bottom()),
                        ),
                        rgb(0x33594d),
                    ));
                }
                let _ = line.paint(origin, bounds.size.height, window, cx);
                if focused && selection.is_empty() {
                    window.paint_quad(fill(
                        Bounds::new(
                            origin + point(line.x_for_index(cursor), px(1.)),
                            size(px(1.), bounds.size.height - px(2.)),
                        ),
                        rgb(TEXT),
                    ));
                }
                input.update(cx, |input, _| {
                    input.layout = Some((Bounds::new(origin, bounds.size), line));
                });
            },
        )
        .w_full()
        .h(px(20.));

        div()
            .id(self.placeholder)
            .track_focus(&self.focus)
            .cursor(CursorStyle::IBeam)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|input, event: &MouseDownEvent, window, cx| {
                    input.focus.focus(window);
                    input.move_cursor(
                        input.index_for_point(event.position),
                        event.modifiers.shift,
                        cx,
                    );
                }),
            )
            .on_mouse_move(cx.listener(|input, event: &MouseMoveEvent, window, cx| {
                if event.dragging() && input.focus.is_focused(window) {
                    input.move_cursor(input.index_for_point(event.position), true, cx);
                }
            }))
            .w_full()
            .px_2()
            .py_1()
            .border_1()
            .border_color(rgb(BORDER))
            .focus(|input| input.border_color(rgb(0x629284)))
            .bg(rgb(0x101313))
            .child(div().overflow_hidden().child(line))
    }
}

#[cfg(test)]
mod tests {
    use super::utf8_offset;

    #[test]
    fn utf16_offsets_account_for_surrogate_pairs() {
        assert_eq!(utf8_offset("a😀b", 3), 5);
    }
}
