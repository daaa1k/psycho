use std::ops::Range;
use std::sync::Arc;

use gpui::{
    App, Bounds, Context, CursorStyle, Element, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, FocusHandle, Focusable, GlobalElementId, LayoutId, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, SharedString, Style,
    TextRun, UTF16Selection, UnderlineStyle, Window, WrappedLine, actions, div, fill, hsla, point,
    prelude::*, px, relative, rgba, size,
};

actions!(
    ime_probe,
    [
        Backspace, Delete, Left, Right, Up, Down, SelectAll, Undo, Redo, Enter, Escape
    ]
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UndoGroup {
    Typing,
    Backspace,
    Delete,
    Newline,
    Atomic,
    Composition,
}

fn utf8_offset_from_utf16(text: &str, offset: usize) -> usize {
    let mut utf8_offset = 0;
    let mut utf16_count = 0;
    for ch in text.chars() {
        if utf16_count >= offset {
            break;
        }
        utf16_count += ch.len_utf16();
        utf8_offset += ch.len_utf8();
    }
    utf8_offset
}

fn utf16_offset_from_utf8(text: &str, offset: usize) -> usize {
    let mut utf16_offset = 0;
    let mut utf8_count = 0;
    for ch in text.chars() {
        if utf8_count >= offset {
            break;
        }
        utf8_count += ch.len_utf8();
        utf16_offset += ch.len_utf16();
    }
    utf16_offset
}

fn utf8_range_from_utf16(text: &str, range: &Range<usize>) -> Range<usize> {
    utf8_offset_from_utf16(text, range.start)..utf8_offset_from_utf16(text, range.end)
}

fn selection_range_after_marked_text(
    replacement_range: &Range<usize>,
    new_text: &str,
    new_selected_range_utf16: Option<&Range<usize>>,
) -> Range<usize> {
    // NSTextInputClient measures selectedRange from the beginning of the inserted string.
    new_selected_range_utf16
        .map(|range| utf8_range_from_utf16(new_text, range))
        .map(|range| replacement_range.start + range.start..replacement_range.start + range.end)
        .unwrap_or_else(|| {
            replacement_range.start + new_text.len()..replacement_range.start + new_text.len()
        })
}

#[derive(Clone)]
pub struct CanvasInputLayout {
    pub font_size: f32,
    pub line_height: f32,
    pub width: f32,
    pub scale: f32,
    pub placement: super::text::TextPlacement,
    pub code: bool,
    pub language: Option<String>,
    pub bullets: bool,
}

#[derive(Clone, Default)]
struct DisplayText {
    text: String,
    boundaries: Vec<(usize, usize)>,
}

impl DisplayText {
    fn new(raw: &str, expand_tabs: bool) -> Self {
        let mut result = Self {
            text: String::new(),
            boundaries: vec![(0, 0)],
        };
        let mut column = 0;
        for (offset, ch) in raw.char_indices() {
            match ch {
                '\t' if expand_tabs => {
                    let count = 4 - column % 4;
                    result.text.extend(std::iter::repeat_n(' ', count));
                    column += count;
                }
                '\n' => {
                    result.text.push(ch);
                    column = 0;
                }
                _ => {
                    result.text.push(ch);
                    column += 1;
                }
            }
            result
                .boundaries
                .push((offset + ch.len_utf8(), result.text.len()));
        }
        result
    }

    fn display_offset(&self, raw: usize) -> usize {
        self.boundaries
            .get(
                self.boundaries
                    .partition_point(|(r, _)| *r <= raw)
                    .saturating_sub(1),
            )
            .map_or(0, |(_, d)| *d)
    }

    fn raw_offset(&self, display: usize) -> usize {
        self.boundaries
            .get(
                self.boundaries
                    .partition_point(|(_, d)| *d <= display)
                    .saturating_sub(1),
            )
            .map_or(0, |(r, _)| *r)
    }
}

pub struct TextInputState {
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    multiline: bool,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    composition_before: Option<String>,
    undo_stack: Vec<String>,
    redo_stack: Vec<String>,
    last_undo_group: Option<UndoGroup>,
    pending_undo_group: Option<UndoGroup>,
    last_layout: Option<Arc<Vec<(WrappedLine, usize)>>>,
    last_line_height: Pixels,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
    canvas_layout: Option<CanvasInputLayout>,
    last_display: DisplayText,
    last_scale: f32,
    last_indent: Pixels,
    last_paragraph_gap: Pixels,
}

impl TextInputState {
    pub fn new(cx: &mut Context<Self>, initial_text: &str) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: initial_text.into(),
            placeholder: "タイトルを入力".into(),
            multiline: false,
            selected_range: initial_text.len()..initial_text.len(),
            selection_reversed: false,
            marked_range: None,
            composition_before: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            last_undo_group: None,
            pending_undo_group: None,
            last_layout: None,
            last_line_height: px(20.0),
            last_bounds: None,
            is_selecting: false,
            canvas_layout: None,
            last_display: DisplayText::new(initial_text, false),
            last_scale: 1.,
            last_indent: px(0.),
            last_paragraph_gap: px(0.),
        }
    }

    pub fn set_canvas_layout(&mut self, layout: Option<CanvasInputLayout>) {
        self.canvas_layout = layout;
    }

    fn record_acceptance_state(&self, window: &Window) {
        let Some(path) = std::env::var_os("PSYCHO_INPUT_STATE") else {
            return;
        };
        let selected = self.range_to_utf16(&self.selected_range);
        let marked = self
            .marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range));
        let cursor = self
            .position_for_offset(self.cursor_offset())
            .unwrap_or(point(px(0.), px(0.)));
        let origin = self
            .last_bounds
            .map_or(point(px(0.), px(0.)), |bounds| bounds.origin);
        let text_hex = self
            .content
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let _ = std::fs::write(
            path,
            format!(
                "{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                trace_timestamp(),
                text_hex,
                selected.start,
                selected.end,
                marked.as_ref().map_or(-1, |range| range.start as i64),
                marked.as_ref().map_or(-1, |range| range.end as i64),
                f32::from((origin + cursor).x),
                f32::from((origin + cursor).y),
                f32::from(self.line_height()) * self.last_scale,
                self.undo_stack.len(),
                self.redo_stack.len(),
                self.composition_before.is_some(),
                self.has_focus(window)
            ),
        );
    }

    pub fn value(&self) -> String {
        self.content.to_string()
    }

    pub fn is_marked(&self) -> bool {
        self.marked_range.is_some()
    }

    pub fn finish_composition(&mut self, cx: &mut Context<Self>) {
        let had_marked_text = self.marked_range.take().is_some();
        let before = self.composition_before.take();
        let had_composition = before.is_some();
        if let Some(before) = before {
            self.push_undo(before, UndoGroup::Composition);
        }
        if had_marked_text || had_composition {
            cx.notify();
        }
    }

    pub fn has_focus(&self, window: &Window) -> bool {
        self.focus_handle.is_focused(window)
    }

    pub fn is_multiline(&self) -> bool {
        self.multiline
    }

    pub fn set_multiline(&mut self, multiline: bool, cx: &mut Context<Self>) {
        self.multiline = multiline;
        cx.notify();
    }

    pub fn focus_handle(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    pub fn set_value(&mut self, value: &str, cx: &mut Context<Self>) {
        self.content = value.into();
        self.selected_range = value.len()..value.len();
        self.selection_reversed = false;
        self.marked_range = None;
        self.composition_before = None;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.last_undo_group = None;
        self.pending_undo_group = None;
        cx.notify();
    }

    /// Returns the committed text states since the editor was last initialized.
    /// The first stored value is the baseline, so it is omitted from the list.
    pub fn history_states_for_commit(&self) -> Vec<String> {
        let mut states = self.undo_stack.iter().skip(1).cloned().collect::<Vec<_>>();
        let current = self.content.to_string();
        if states.last() != Some(&current) {
            states.push(current);
        }
        states
    }

    pub fn reset_undo_history(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.last_undo_group = None;
        self.pending_undo_group = None;
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.marked_range.is_none() {
            self.break_undo_group();
            let offset = if self.selected_range.is_empty() {
                self.previous_boundary(self.cursor_offset())
            } else {
                self.selected_range.start
            };
            self.move_to(offset, cx);
        } else {
            cx.notify();
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.marked_range.is_none() {
            self.break_undo_group();
            let offset = if self.selected_range.is_empty() {
                self.next_boundary(self.cursor_offset())
            } else {
                self.selected_range.end
            };
            self.move_to(offset, cx);
        } else {
            cx.notify();
        }
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        if !self.multiline || self.marked_range.is_some() {
            return;
        }
        self.break_undo_group();
        self.move_vertically(-1, cx);
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        if !self.multiline || self.marked_range.is_some() {
            return;
        }
        self.break_undo_group();
        self.move_vertically(1, cx);
    }

    fn enter(&mut self, _: &Enter, window: &mut Window, cx: &mut Context<Self>) {
        if self.multiline && self.marked_range.is_none() {
            self.pending_undo_group = Some(UndoGroup::Newline);
            self.replace_text_in_range(None, "\n", window, cx);
        }
    }

    fn escape(&mut self, _: &Escape, window: &mut Window, cx: &mut Context<Self>) {
        if self.marked_range.is_none() {
            window.dispatch_action(Box::new(super::CommitTextEdit), cx);
        }
        cx.notify();
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.break_undo_group();
        self.selected_range = 0..self.content.len();
        self.selection_reversed = false;
        cx.notify();
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.marked_range.is_some() {
            return;
        }
        if self.selected_range.is_empty() {
            let previous = self.previous_boundary(self.cursor_offset());
            if previous == self.cursor_offset() {
                window.play_system_bell();
                return;
            }
            self.selected_range = previous..self.cursor_offset();
        }
        self.pending_undo_group = Some(UndoGroup::Backspace);
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.marked_range.is_some() {
            return;
        }
        if self.selected_range.is_empty() {
            let next = self.next_boundary(self.cursor_offset());
            if next == self.cursor_offset() {
                window.play_system_bell();
                return;
            }
            self.selected_range = self.cursor_offset()..next;
        }
        self.pending_undo_group = Some(UndoGroup::Delete);
        self.replace_text_in_range(None, "", window, cx);
    }

    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        if self.marked_range.is_some() {
            return;
        }
        self.break_undo_group();
        if let Some(before) = self.undo_stack.pop() {
            self.redo_stack.push(self.content.to_string());
            self.content = before.into();
            let end = self.content.len();
            self.selected_range = end..end;
            self.marked_range = None;
            self.composition_before = None;
            cx.notify();
        }
    }

    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        if self.marked_range.is_some() {
            return;
        }
        self.break_undo_group();
        if let Some(after) = self.redo_stack.pop() {
            self.undo_stack.push(self.content.to_string());
            self.content = after.into();
            let end = self.content.len();
            self.selected_range = end..end;
            self.marked_range = None;
            self.composition_before = None;
            cx.notify();
        }
    }

    fn on_mouse_down(&mut self, event: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.break_undo_group();
        self.is_selecting = true;
        let offset = self.index_for_mouse_position(event.position);
        if event.modifiers.shift {
            self.select_to(offset, cx);
        } else {
            self.move_to(offset, cx);
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.break_undo_group();
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        cx.notify();
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.break_undo_group();
        if self.selection_reversed {
            self.selected_range.start = offset;
        } else {
            self.selected_range.end = offset;
        }
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.notify();
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        let Some(bounds) = self.last_bounds else {
            return 0;
        };
        self.offset_for_position(position - bounds.origin)
    }

    fn line_height(&self) -> Pixels {
        self.last_line_height
    }

    fn position_for_offset(&self, offset: usize) -> Option<Point<Pixels>> {
        let lines = self.last_layout.as_ref()?;
        let offset = self.last_display.display_offset(offset);
        let mut top = px(0.);
        for (index, (line, start)) in lines.iter().enumerate() {
            if offset <= start + line.len() || index + 1 == lines.len() {
                let position = line.position_for_index(
                    offset.saturating_sub(*start).min(line.len()),
                    self.line_height(),
                )?;
                return Some(
                    point(position.x + self.last_indent, position.y + top) * self.last_scale,
                );
            }
            top += line.size(self.line_height()).height + self.last_paragraph_gap;
        }
        Some(point(self.last_indent, top) * self.last_scale)
    }

    fn offset_for_position(&self, position: Point<Pixels>) -> usize {
        let Some(lines) = self.last_layout.as_ref() else {
            return 0;
        };
        let position = position / self.last_scale;
        let mut top = px(0.);
        for (line, start) in lines.iter() {
            let height = line.size(self.line_height()).height;
            if position.y <= top + height {
                let index = line
                    .closest_index_for_position(
                        point(
                            (position.x - self.last_indent).max(px(0.)),
                            (position.y - top).max(px(0.)),
                        ),
                        self.line_height(),
                    )
                    .unwrap_or(line.len())
                    .min(line.len());
                return self
                    .last_display
                    .raw_offset(start + index)
                    .min(self.content.len());
            }
            top += height + self.last_paragraph_gap;
        }
        self.content.len()
    }

    fn move_vertically(&mut self, direction: i8, cx: &mut Context<Self>) {
        if self.marked_range.is_some() {
            return;
        }
        let Some(position) = self.position_for_offset(self.cursor_offset()) else {
            cx.notify();
            return;
        };
        let target = point(
            position.x,
            position.y + self.line_height() * self.last_scale * direction as f32,
        );
        let offset = self.offset_for_position(target);
        self.move_to(offset, cx);
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        utf8_offset_from_utf16(&self.content, offset)
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        utf16_offset_from_utf8(&self.content, offset)
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range.start)..self.offset_from_utf16(range.end)
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .char_indices()
            .map(|(index, _)| index)
            .take_while(|index| *index < offset)
            .last()
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .char_indices()
            .map(|(index, ch)| index + ch.len_utf8())
            .find(|index| *index > offset)
            .unwrap_or(self.content.len())
    }

    fn push_undo(&mut self, before: String, group: UndoGroup) {
        if before == self.content.as_ref() {
            return;
        }
        let can_merge = self.last_undo_group == Some(group)
            && matches!(
                group,
                UndoGroup::Typing | UndoGroup::Backspace | UndoGroup::Delete
            );
        if !can_merge {
            self.undo_stack.push(before);
            self.redo_stack.clear();
        }
        self.last_undo_group = Some(group);
    }

    fn break_undo_group(&mut self) {
        self.last_undo_group = None;
    }
}

impl EntityInputHandler for TextInputState {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        // Native clients may commit by unmarking without insertText. Keep that
        // composition atomic and repaint its selection/underline immediately.
        self.finish_composition(cx);
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let group = self.pending_undo_group.take().unwrap_or_else(|| {
            if new_text.chars().count() == 1 && !new_text.contains('\n') {
                UndoGroup::Typing
            } else {
                UndoGroup::Atomic
            }
        });
        let range = range_utf16
            .as_ref()
            .map(|range| self.range_from_utf16(range))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        let before = self
            .composition_before
            .take()
            .unwrap_or_else(|| self.content.to_string());
        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.marked_range = None;
        self.push_undo(before, group);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.marked_range.is_none() {
            self.break_undo_group();
            self.composition_before = Some(self.content.to_string());
        }
        let range = range_utf16
            .as_ref()
            .map(|range| self.range_from_utf16(range))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        self.marked_range = if new_text.is_empty() {
            None
        } else {
            Some(range.start..range.start + new_text.len())
        };
        self.selected_range =
            selection_range_after_marked_text(&range, new_text, new_selected_range_utf16.as_ref());
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        self.last_layout.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        let start = self.position_for_offset(range.start)?;
        let end = self.position_for_offset(range.end)?;
        let top = start.y.min(end.y);
        let bottom =
            (start.y.max(end.y) + self.line_height() * self.last_scale).min(bounds.size.height);
        let rectangle = Bounds::from_corners(
            point(bounds.left() + start.x, bounds.top() + top),
            point(
                bounds.left() + end.x.max(start.x + px(1.0)),
                bounds.top() + bottom,
            ),
        );
        if let Some(path) = std::env::var_os("PSYCHO_IME_RECTS") {
            use std::io::Write;
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
            {
                let _ = writeln!(
                    file,
                    "{},{},{},{},{},{},{}",
                    trace_timestamp(),
                    range_utf16.start,
                    range_utf16.end,
                    f32::from(rectangle.origin.x),
                    f32::from(rectangle.origin.y),
                    f32::from(rectangle.size.width),
                    f32::from(rectangle.size.height)
                );
            }
        }
        Some(rectangle)
    }

    fn character_index_for_point(
        &mut self,
        click: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let bounds = self.last_bounds?;
        let position = point(click.x - bounds.left(), click.y - bounds.top());
        Some(self.offset_to_utf16(self.offset_for_position(position)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_tabs_keep_raw_unicode_and_utf16_edit_offsets() {
        let raw = "日\t語\n\t😀x\t";
        let display = DisplayText::new(raw, true);
        assert_eq!(display.text, "日   語\n    😀x  ");
        for (offset, _) in raw.char_indices().chain(std::iter::once((raw.len(), '\0'))) {
            assert_eq!(display.raw_offset(display.display_offset(offset)), offset);
            let utf16 = utf16_offset_from_utf8(raw, offset);
            assert_eq!(utf8_offset_from_utf16(raw, utf16), offset);
        }
        let before_tab = "日".len();
        assert_eq!(
            display.raw_offset(display.display_offset(before_tab) + 2),
            before_tab
        );
        let unchanged = DisplayText::new(raw, false);
        assert_eq!(unchanged.text, raw);
    }

    #[test]
    fn marked_text_selection_is_relative_to_replacement_text() {
        let original = "前題後";
        let replacement_range = "前".len().."前題".len();
        let new_text = "にほんごへんかん";
        let content_after_replacement = format!(
            "{}{}{}",
            &original[..replacement_range.start],
            new_text,
            &original[replacement_range.end..]
        );
        let selected_range_utf16 = new_text.encode_utf16().count()..new_text.encode_utf16().count();

        let selected_range = selection_range_after_marked_text(
            &replacement_range,
            new_text,
            Some(&selected_range_utf16),
        );

        assert_eq!(
            selected_range,
            replacement_range.start + new_text.len()..replacement_range.start + new_text.len()
        );
        assert!(selected_range.end <= content_after_replacement.len());
    }

    #[test]
    fn marked_text_selection_stays_within_a_fully_replaced_title() {
        let original = "題";
        let replacement_range = 0..original.len();
        let new_text = "にほんごへんかん";
        let content_after_replacement = new_text.to_owned();
        let selected_range_utf16 = new_text.encode_utf16().count()..new_text.encode_utf16().count();

        let selected_range = selection_range_after_marked_text(
            &replacement_range,
            new_text,
            Some(&selected_range_utf16),
        );

        assert_eq!(selected_range, new_text.len()..new_text.len());
        assert!(selected_range.end <= content_after_replacement.len());
    }
}

impl Render for TextInputState {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w_full()
            .flex_shrink_0()
            .key_context("TextInput")
            .track_focus(&self.focus_handle)
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::enter))
            .on_action(cx.listener(Self::escape))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .child(TextElement { input: cx.entity() })
    }
}

impl Focusable for TextInputState {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

struct TextElement {
    input: Entity<TextInputState>,
}

struct PrepaintState {
    lines: Arc<Vec<(WrappedLine, usize)>>,
    cursor: Option<PaintQuad>,
    selection: Vec<PaintQuad>,
    underline: Vec<PaintQuad>,
    bounds: Bounds<Pixels>,
    display: DisplayText,
    scale: f32,
    font_size: Pixels,
    line_height: Pixels,
    indent: Pixels,
    paragraph_gap: Pixels,
    canvas: bool,
    color: gpui::Hsla,
    canvas_origin: Point<Pixels>,
    highlights: Vec<(Range<usize>, gpui::HighlightStyle)>,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
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
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        let input = self.input.read(cx);
        if let Some(layout) = &input.canvas_layout {
            let display = DisplayText::new(&input.content, layout.code);
            let indent = if layout.bullets { 28. } else { 0. };
            let gap = if layout.bullets { 8. } else { 0. };
            let run = window.text_style().to_run(display.text.len());
            let lines = window
                .text_system()
                .shape_text(
                    display.text.into(),
                    px(layout.font_size),
                    &[run],
                    (!layout.code).then(|| px(layout.width - indent)),
                    None,
                )
                .unwrap_or_default();
            let height = lines
                .iter()
                .map(|line| f32::from(line.size(px(layout.line_height)).height))
                .sum::<f32>()
                .max(layout.line_height)
                + gap * lines.len().saturating_sub(1) as f32;
            style.size.height = px(height * layout.scale).into();
            return (window.request_layout(style, [], cx), ());
        }
        let text = if input.content.is_empty() {
            input.placeholder.clone()
        } else {
            input.content.clone()
        };
        let text_style = window.text_style();
        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();
        let run = text_style.to_run(text.len());
        let layout = window.request_measured_layout(style, move |known, available, window, _| {
            let width = known.width.or(match available.width {
                gpui::AvailableSpace::Definite(width) => Some(width),
                _ => None,
            });
            let lines = window
                .text_system()
                .shape_text(
                    text.clone(),
                    font_size,
                    std::slice::from_ref(&run),
                    width,
                    None,
                )
                .unwrap_or_default();
            let height = lines
                .iter()
                .map(|line| line.size(line_height).height)
                .sum::<Pixels>()
                .max(line_height);
            size(
                width.unwrap_or_else(|| {
                    lines
                        .iter()
                        .map(|line| line.size(line_height).width)
                        .max()
                        .unwrap_or(px(0.))
                }),
                height,
            )
        });
        (layout, ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let layout = input.canvas_layout.clone();
        let display = DisplayText::new(
            if input.content.is_empty() && layout.is_none() {
                &input.placeholder
            } else {
                &input.content
            },
            layout.as_ref().is_some_and(|layout| layout.code),
        );
        let text: SharedString = display.text.clone().into();
        let selected_range = display.display_offset(input.selected_range.start)
            ..display.display_offset(input.selected_range.end);
        let cursor_offset = display.display_offset(input.cursor_offset());
        let style = window.text_style();
        let text_color = if input.content.is_empty() {
            hsla(0.0, 0.0, 0.0, 0.3)
        } else {
            style.color
        };
        let base_run = TextRun {
            len: text.len(),
            font: style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let marked = input
            .marked_range
            .as_ref()
            .map(|range| display.display_offset(range.start)..display.display_offset(range.end));
        let runs = if let Some(marked) = marked.as_ref() {
            vec![
                TextRun {
                    len: marked.start,
                    ..base_run.clone()
                },
                TextRun {
                    len: marked.end - marked.start,
                    underline: Some(UnderlineStyle {
                        color: Some(base_run.color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..base_run.clone()
                },
                TextRun {
                    len: text.len() - marked.end,
                    ..base_run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect()
        } else {
            vec![base_run]
        };
        let scale = layout.as_ref().map_or(1., |layout| layout.scale);
        let font_size = layout.as_ref().map_or_else(
            || style.font_size.to_pixels(window.rem_size()),
            |layout| px(layout.font_size),
        );
        let line_height = layout
            .as_ref()
            .map_or_else(|| window.line_height(), |layout| px(layout.line_height));
        let indent = px(if layout.as_ref().is_some_and(|layout| layout.bullets) {
            28.
        } else {
            0.
        });
        let paragraph_gap = px(if layout.as_ref().is_some_and(|layout| layout.bullets) {
            8.
        } else {
            0.
        });
        let wrap_width = layout.as_ref().map_or(Some(bounds.size.width), |layout| {
            (!layout.code).then(|| px(layout.width) - indent)
        });
        let origin = layout
            .as_ref()
            .map_or(bounds.origin, |layout| layout.placement.origin(scale));
        let bounds = Bounds::new(origin, bounds.size);
        let shaped = window
            .text_system()
            .shape_text(text.clone(), font_size, &runs, wrap_width, None)
            .unwrap_or_default();
        let mut offset = 0;
        let lines = shaped
            .into_iter()
            .map(|line| {
                let start = offset;
                offset += line.text.len() + usize::from(offset + line.text.len() < text.len());
                (line, start)
            })
            .collect::<Vec<_>>();
        let lines = Arc::new(lines);
        let line_position = |byte_offset: usize| {
            let mut line_top = px(0.0);
            for (line_index, (line, start)) in lines.iter().enumerate() {
                let end = start + line.len();
                if byte_offset <= end || line_index + 1 == lines.len() {
                    let local_offset = byte_offset.saturating_sub(*start).min(line.len());
                    let position = line
                        .position_for_index(local_offset, line_height)
                        .unwrap_or(point(px(0.0), px(0.0)));
                    return point(position.x + indent, position.y + line_top) * scale;
                }
                line_top += line.size(line_height).height + paragraph_gap;
            }
            point(indent, line_top) * scale
        };
        let rectangles = |selected_range: &Range<usize>, underline: bool| {
            let mut selection = Vec::new();
            if !selected_range.is_empty() {
                for (line, start) in lines.iter() {
                    let end = start + line.len();
                    let selected_start = selected_range.start.max(*start);
                    let selected_end = selected_range.end.min(end);
                    if selected_start >= selected_end {
                        continue;
                    }
                    for (char_offset, character) in line.text.char_indices() {
                        let char_end = char_offset + character.len_utf8();
                        let absolute_start = start + char_offset;
                        let absolute_end = start + char_end;
                        if absolute_start >= selected_end || absolute_end <= selected_start {
                            continue;
                        }
                        let start_position = line_position(absolute_start);
                        let end_position = line_position(absolute_end);
                        let right = if end_position.y > start_position.y {
                            wrap_width.unwrap_or(line.size(line_height).width) * scale
                                + indent * scale
                        } else {
                            end_position.x
                        };
                        selection.push(fill(
                            Bounds::from_corners(
                                point(
                                    bounds.left() + start_position.x,
                                    bounds.top()
                                        + start_position.y
                                        + if underline {
                                            line_height * scale - px(1.)
                                        } else {
                                            px(0.)
                                        },
                                ),
                                point(
                                    bounds.left() + right.max(start_position.x + px(1.0)),
                                    bounds.top() + start_position.y + line_height * scale,
                                ),
                            ),
                            if underline {
                                rgba(0x222222ff)
                            } else {
                                rgba(0x3311ff30)
                            },
                        ));
                    }
                }
            }
            selection
        };
        let selection = rectangles(&selected_range, false);
        let underline = if layout.is_some() {
            marked
                .as_ref()
                .map_or_else(Vec::new, |range| rectangles(range, true))
        } else {
            Vec::new()
        };
        let cursor = if selected_range.is_empty() {
            let position = line_position(cursor_offset);
            Some(fill(
                Bounds::new(
                    point(bounds.left() + position.x, bounds.top() + position.y),
                    size(px(2.0), line_height * scale),
                ),
                gpui::blue(),
            ))
        } else {
            None
        };
        let highlights = layout
            .as_ref()
            .filter(|layout| layout.code)
            .map_or_else(Vec::new, |layout| {
                super::code_highlights(&display.text, layout.language.as_deref())
            });
        PrepaintState {
            lines,
            cursor,
            selection,
            underline,
            bounds,
            display,
            scale,
            font_size,
            line_height,
            indent,
            paragraph_gap,
            canvas: layout.is_some(),
            color: text_color,
            highlights,
            canvas_origin: layout
                .as_ref()
                .map_or(bounds.origin, |layout| layout.placement.canvas_origin.get()),
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let bounds = prepaint.bounds;
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        for selection in prepaint.selection.drain(..) {
            window.paint_quad(selection);
        }
        let mut origin = bounds.origin;
        let mut all_glyphs = Vec::new();
        for (line, start) in prepaint.lines.iter() {
            if prepaint.canvas {
                let glyphs = paint_scaled_line(
                    line,
                    origin + point(prepaint.indent * prepaint.scale, px(0.)),
                    prepaint.font_size,
                    prepaint.line_height,
                    prepaint.scale,
                    prepaint.color,
                    &prepaint.highlights,
                    *start,
                    window,
                );
                if prepaint.indent > px(0.) {
                    record_canvas_glyphs(&line.text, &glyphs, prepaint, window);

                    let run = window.text_style().to_run("•".len());
                    let markers = window
                        .text_system()
                        .shape_text("•".into(), prepaint.font_size, &[run], None, None)
                        .unwrap_or_default();
                    for marker in &markers {
                        let glyphs = paint_scaled_line(
                            marker,
                            origin,
                            prepaint.font_size,
                            prepaint.line_height,
                            prepaint.scale,
                            prepaint.color,
                            &[],
                            0,
                            window,
                        );
                        record_canvas_glyphs("•", &glyphs, prepaint, window);
                    }
                } else {
                    all_glyphs.extend(glyphs);
                }
            } else {
                line.paint(
                    origin,
                    prepaint.line_height,
                    gpui::TextAlign::Left,
                    Some(bounds),
                    window,
                    cx,
                )
                .unwrap();
            }
            origin.y +=
                (line.size(prepaint.line_height).height + prepaint.paragraph_gap) * prepaint.scale;
        }
        if prepaint.canvas && prepaint.indent == px(0.) {
            record_canvas_glyphs(&prepaint.display.text, &all_glyphs, prepaint, window);
        }
        for underline in prepaint.underline.drain(..) {
            window.paint_quad(underline);
        }
        if focus_handle.is_focused(window)
            && let Some(cursor) = prepaint.cursor.take()
        {
            window.paint_quad(cursor);
        }
        self.input.update(cx, |input, _| {
            input.last_layout = Some(prepaint.lines.clone());
            input.last_line_height = prepaint.line_height;
            input.last_bounds = Some(bounds);
            input.last_scale = prepaint.scale;
            input.last_indent = prepaint.indent;
            input.last_paragraph_gap = prepaint.paragraph_gap;
            input.last_display = prepaint.display.clone();
            input.record_acceptance_state(window);
        });
    }
}

fn paint_scaled_line(
    line: &WrappedLine,
    origin: Point<Pixels>,
    font_size: Pixels,
    line_height: Pixels,
    scale: f32,
    color: gpui::Hsla,
    highlights: &[(Range<usize>, gpui::HighlightStyle)],
    byte_offset: usize,
    window: &mut Window,
) -> Vec<(gpui::GlyphId, Point<Pixels>)> {
    let mut glyphs = Vec::new();
    let layout = &line.unwrapped_layout;
    let baseline = (line_height - layout.ascent - layout.descent) / 2. + layout.ascent;
    let mut row = 0;
    let mut wrap_start = px(0.);
    for (run_ix, run) in layout.runs.iter().enumerate() {
        for (glyph_ix, glyph) in run.glyphs.iter().enumerate() {
            if line
                .wrap_boundaries
                .iter()
                .any(|boundary| boundary.run_ix == run_ix && boundary.glyph_ix == glyph_ix)
            {
                row += 1;
                wrap_start = glyph.position.x;
            }
            let position = origin
                + point(
                    glyph.position.x - wrap_start,
                    line_height * row as f32 + baseline + glyph.position.y,
                ) * scale;
            let color = highlights
                .iter()
                .find(|(range, _)| range.contains(&(byte_offset + glyph.index)))
                .and_then(|(_, style)| style.color)
                .unwrap_or(color);
            glyphs.push((glyph.id, position));
            window
                .paint_glyph(position, run.font_id, glyph.id, font_size * scale, color)
                .expect("paint canvas input glyph");
        }
    }
    glyphs
}

fn record_canvas_glyphs(
    text: &str,
    glyphs: &[(gpui::GlyphId, Point<Pixels>)],
    prepaint: &PrepaintState,
    window: &Window,
) {
    let Some(path) = std::env::var_os("PSYCHO_GLYPH_TRACE") else {
        return;
    };
    use std::{fmt::Write, io::Write as _};
    let text_hex = text
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let mut evidence = String::new();
    let viewport = window.viewport_size();
    for (index, (glyph, origin)) in glyphs.iter().enumerate() {
        let _ = writeln!(
            evidence,
            "{},{},{},{},{},{},{},{},{},{},{},{}",
            f32::from(viewport.width),
            f32::from(viewport.height),
            f32::from(prepaint.font_size),
            text_hex,
            index,
            glyph.0,
            f32::from(origin.x),
            f32::from(origin.y),
            prepaint.scale,
            f32::from(prepaint.canvas_origin.x),
            f32::from(prepaint.canvas_origin.y),
            window.scale_factor()
        );
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = file.write_all(evidence.as_bytes());
    }
}

fn trace_timestamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
