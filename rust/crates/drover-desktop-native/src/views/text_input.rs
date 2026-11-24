//! A one-line text field (`.input` in the web UI): typing and Vietnamese Telex through the input method, the keys
//! a field needs (arrows, Home/End, Backspace/Delete, Enter, Escape, Ctrl+V), drawn with the app theme.
//! Emits `Changed` on every edit, `Submit` on Enter and `Cancel` on Escape. `new_multiline` makes a small text editor
//! (settings files): Enter inserts a line, Up / Down / Home / End work by line, Tab inserts two spaces, Ctrl+S emits
//! `Submit`. No selection, no undo and no mouse placement of the caret yet.

use std::ops::Range;

use gpui::{App, Bounds, Context, ElementInputHandler, EventEmitter, FocusHandle, Focusable, KeyDownEvent, MouseButton, Pixels, ScrollHandle, SharedString, UTF16Selection, Window, canvas, div, prelude::*, rgb};
use crate::ui_scale::px;

use crate::theme::{self, accent, dim, fg};

pub enum InputEvent {
    Changed,
    Submit,
    Cancel,
}

pub struct TextInput {
    pub text: String,
    /// Byte offset in `text`, always on a char boundary.
    cursor: usize,
    /// Text the input method is still composing: drawn at the cursor, not part of `text` yet.
    marked: Option<String>,
    /// A key pressed while a word was being composed: the input method commits the word after the key reaches us,
    /// so the key waits for that commit (else Left / Enter would act before the word is in the text).
    after_commit: Vec<String>,
    focus: FocusHandle,
    placeholder: SharedString,
    multiline: bool,
    /// Font of a multi-line editor (the code font); the app sets it every frame.
    pub family: Option<SharedString>,
    scroll: ScrollHandle,
    /// Scroll the caret's line into view on the next paint.
    follow: bool,
}

// Line maths shared with the code editor.
use crate::code_editor::buffer::{line_bounds, vertical};

/// Height of one line of a multi-line editor.
const LINE_H: f32 = 19.;

impl EventEmitter<InputEvent> for TextInput {}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl TextInput {
    pub fn new(placeholder: &str, cx: &mut Context<Self>) -> Self {
        TextInput { text: String::new(), cursor: 0, marked: None, after_commit: vec![], focus: cx.focus_handle(), placeholder: placeholder.to_string().into(),
                    multiline: false, family: None, scroll: ScrollHandle::new(), follow: false }
    }

    pub fn new_multiline(cx: &mut Context<Self>) -> Self {
        TextInput { multiline: true, ..Self::new("", cx) }
    }

    fn line_bounds(&self, at: usize) -> (usize, usize) {
        line_bounds(&self.text, at)
    }

    /// Cursor to the same column on the line above (`down` false) or below.
    fn move_line(&mut self, down: bool) {
        self.cursor = vertical(&self.text, self.cursor, down);
    }

    #[allow(dead_code)] // used by the views' forms (phase 01+)
    pub fn set_text(&mut self, text: &str, cx: &mut Context<Self>) {
        self.text = text.to_string();
        self.cursor = self.text.len();
        self.marked = None;
        self.after_commit.clear();
        cx.emit(InputEvent::Changed);
        cx.notify();
    }

    /// What the field shows: its text with the word being composed at the cursor (a search filters on it live).
    pub fn value(&self) -> String {
        match &self.marked {
            Some(m) => format!("{}{m}{}", &self.text[..self.cursor], &self.text[self.cursor..]),
            None => self.text.clone(),
        }
    }

    fn edit(&mut self, range: Range<usize>, with: &str, cx: &mut Context<Self>) {
        let with = if self.multiline { with.replace('\r', "").replace('\t', "  ") } else { with.replace(['\r', '\n', '\t'], " ") }; // one line: a pasted newline would submit nothing useful
        self.text.replace_range(range.clone(), &with);
        self.cursor = range.start + with.len();
        cx.emit(InputEvent::Changed);
        cx.notify();
    }

    fn prev(&self, at: usize) -> usize {
        self.text[..at].char_indices().next_back().map(|(i, _)| i).unwrap_or(0)
    }

    fn next(&self, at: usize) -> usize {
        self.text[at..].chars().next().map(|c| at + c.len_utf8()).unwrap_or(at)
    }

    fn on_key(&mut self, ev: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        crate::actions::trace(&format!("input key {:?} char {:?}", ev.keystroke.key, ev.keystroke.key_char));
        let m = &ev.keystroke.modifiers;
        let key = ev.keystroke.key.as_str();
        let edit_key = matches!(key, "backspace" | "delete" | "left" | "right" | "home" | "end" | "enter" | "up" | "down" | "tab");
        if self.marked.is_some() && edit_key && !m.control && !m.alt {
            self.after_commit.push(key.to_string());
            cx.stop_propagation();
            return;
        }
        if self.apply_key(key, m.control && !m.alt, cx) {
            self.follow = true;
            cx.stop_propagation();
            cx.notify();
        }
    }

    /// Run one editing key. False when it is not ours (it goes on to the app).
    fn apply_key(&mut self, key: &str, ctrl: bool, cx: &mut Context<Self>) -> bool {
        let c = self.cursor;
        match key {
            "backspace" if c > 0 => self.edit(self.prev(c)..c, "", cx),
            "delete" if c < self.text.len() => self.edit(c..self.next(c), "", cx),
            "left" => self.cursor = self.prev(c),
            "right" => self.cursor = self.next(c),
            "home" => self.cursor = if self.multiline { self.line_bounds(c).0 } else { 0 },
            "end" => self.cursor = if self.multiline { self.line_bounds(c).1 } else { self.text.len() },
            "up" | "down" if self.multiline => self.move_line(key == "down"),
            "tab" if self.multiline => self.edit(c..c, "  ", cx),
            "enter" if self.multiline => self.edit(c..c, "\n", cx),
            "s" if ctrl && self.multiline => cx.emit(InputEvent::Submit),
            "enter" => cx.emit(InputEvent::Submit),
            "escape" => {
                self.after_commit.clear();
                cx.emit(InputEvent::Cancel);
                return false; // also reaches the app: Escape leaves the view
            }
            "v" if ctrl => {
                if let Some(t) = cx.read_from_clipboard().and_then(|i| i.text()) {
                    self.edit(c..c, &t, cx);
                }
            }
            "backspace" | "delete" | "tab" => {}
            _ => return false, // text comes through the input method below; other keys go on to the app
        }
        true
    }

    fn run_waiting_key(&mut self, cx: &mut Context<Self>) {
        for key in std::mem::take(&mut self.after_commit) {
            self.apply_key(&key, false, cx);
        }
        cx.notify();
    }

    fn utf16_at_byte(&self, byte: usize) -> usize {
        self.text[..byte].encode_utf16().count()
    }

    fn byte_at_utf16(&self, units: usize) -> usize {
        let mut n = 0;
        for (i, ch) in self.text.char_indices() {
            if n >= units {
                return i;
            }
            n += ch.len_utf16();
        }
        self.text.len()
    }
}

impl gpui::EntityInputHandler for TextInput {
    fn text_for_range(&mut self, r: Range<usize>, adjusted: &mut Option<Range<usize>>, _: &mut Window, _: &mut Context<Self>) -> Option<String> {
        let (a, b) = (self.byte_at_utf16(r.start.min(r.end)), self.byte_at_utf16(r.start.max(r.end)));
        *adjusted = Some(self.utf16_at_byte(a)..self.utf16_at_byte(b));
        Some(self.text[a..b].to_string())
    }

    fn selected_text_range(&mut self, _: bool, _: &mut Window, _: &mut Context<Self>) -> Option<UTF16Selection> {
        let c = self.utf16_at_byte(self.cursor);
        Some(UTF16Selection { range: c..c, reversed: false })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        let c = self.utf16_at_byte(self.cursor);
        self.marked.as_ref().map(|t| c..c + t.encode_utf16().count())
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked = None;
    }

    fn replace_text_in_range(&mut self, r: Option<Range<usize>>, text: &str, _: &mut Window, cx: &mut Context<Self>) {
        crate::actions::trace(&format!("input commit {text:?} range {r:?} cursor {}", self.cursor));
        // While a word is composed its range is the marked text, which is not in `text`: commit at the cursor.
        // Otherwise the input method may replace text it already committed (some Telex engines do): honour its range.
        let composing = self.marked.take().is_some();
        let range = match r.filter(|_| !composing) {
            Some(r) => self.byte_at_utf16(r.start.min(r.end))..self.byte_at_utf16(r.start.max(r.end)),
            None => self.cursor..self.cursor,
        };
        self.edit(range, text, cx);
        self.run_waiting_key(cx);
    }

    fn replace_and_mark_text_in_range(&mut self, _: Option<Range<usize>>, text: &str, _: Option<Range<usize>>,
                                      _: &mut Window, cx: &mut Context<Self>) {
        crate::actions::trace(&format!("input preedit {text:?}"));
        self.marked = (!text.is_empty()).then(|| text.to_string());
        if self.marked.is_none() {
            self.run_waiting_key(cx); // the word was dropped, not committed: the key still counts
        }
        cx.emit(InputEvent::Changed);
        cx.notify();
    }

    fn bounds_for_range(&mut self, _: Range<usize>, element: Bounds<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<Bounds<Pixels>> {
        Some(element)
    }

    fn character_index_for_point(&mut self, _: gpui::Point<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        None
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.focus.is_focused(window);
        let (entity, focus) = (cx.entity(), self.focus.clone());
        let empty = self.text.is_empty() && self.marked.is_none();
        let caret = div().w(px(1.5)).h(px(15.)).flex_none().bg(rgb(fg()));
        if self.multiline {
            return self.render_editor(focused, entity, focus, caret, cx).into_any_element();
        }
        div().relative().h(px(28.)).px(px(9.)).flex().items_center().overflow_hidden().rounded(px(7.)).border_1()
            .border_color(if focused { theme::tint(accent(), 0.7) } else { theme::border2() }).bg(theme::ink(0.03))
            .text_size(px(12.5)).text_color(rgb(fg())).cursor_text()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| { window.focus(&this.focus); cx.notify(); }))
            .child(canvas(|_, _, _| {}, move |bounds, _, window, cx| {
                window.handle_input(&focus, ElementInputHandler::new(bounds, entity), cx);
            }).absolute().top_0().left_0().size_full())
            .when(empty && !focused, |d| d.child(div().text_color(rgb(dim())).child(self.placeholder.clone())))
            .when(!empty || focused, |d| {
                d.child(div().flex_none().whitespace_nowrap().child(self.text[..self.cursor].to_string()))
                    .children(self.marked.clone().map(|m| div().flex_none().underline().child(m)))
                    .when(focused, |d| d.child(caret))
                    .child(div().flex_none().whitespace_nowrap().child(self.text[self.cursor..].to_string()))
            })
            .into_any_element()
    }
}

impl TextInput {
    /// The multi-line editor: one row per line (so the scroll handle can bring the caret's row into view).
    fn render_editor(&mut self, focused: bool, entity: gpui::Entity<TextInput>, focus: FocusHandle, caret: gpui::Div, cx: &mut Context<Self>) -> impl IntoElement {
        let cur_line = self.text[..self.cursor].matches('\n').count();
        if std::mem::take(&mut self.follow) {
            self.scroll.scroll_to_item(cur_line);
        }
        let (ls, le) = self.line_bounds(self.cursor);
        let mut rows = Vec::new();
        for (i, line) in self.text.split('\n').enumerate() {
            let row = div().min_h(px(LINE_H)).flex().items_center();
            rows.push(if i == cur_line && (focused || self.marked.is_some()) {
                let (a, b) = (&self.text[ls..self.cursor], &self.text[self.cursor..le]);
                row.child(div().flex_none().whitespace_nowrap().child(a.to_string()))
                    .children(self.marked.clone().map(|m| div().flex_none().underline().child(m)))
                    .child(div().w(px(1.5)).h(px(15.)).flex_none().bg(rgb(fg())))
                    .child(div().whitespace_nowrap().child(b.to_string()))
            } else {
                row.child(div().whitespace_nowrap().child(line.to_string()))
            });
        }
        let _ = caret;
        div().relative().flex_1().min_h_0().min_w_0().overflow_hidden().rounded(px(7.)).border_1()
            .border_color(if focused { theme::tint(accent(), 0.7) } else { theme::border2() }).bg(theme::ink(0.03))
            .text_size(px(12.5)).text_color(rgb(fg())).cursor_text()
            .when_some(self.family.clone(), |d, f| d.font_family(f))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| { window.focus(&this.focus); cx.notify(); }))
            .child(canvas(|_, _, _| {}, move |bounds, _, window, cx| {
                window.handle_input(&focus, ElementInputHandler::new(bounds, entity), cx);
            }).absolute().top_0().left_0().size_full())
            .child(div().id("editor-lines").size_full().overflow_y_scroll().track_scroll(&self.scroll).py(px(6.)).px(px(9.)).children(rows))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_bounds_cover_first_middle_and_last_lines() {
        let t = "ab\ncde\n\nf";
        assert_eq!(line_bounds(t, 1), (0, 2));
        assert_eq!(line_bounds(t, 5), (3, 6));
        assert_eq!(line_bounds(t, 7), (7, 7)); // the empty line
        assert_eq!(line_bounds(t, 9), (8, 9));
    }

    #[test]
    fn up_and_down_keep_the_column_or_stop_at_the_line_end() {
        let t = "abcd\nxy\nlmnop"; // a0 b1 c2 d3 \n4 x5 y6 \n7 l8 m9 n10 o11 p12
        assert_eq!(vertical(t, 3, true), 7); // "abc|d": column 3 of "xy" is past its end: the end of that line
        assert_eq!(vertical(t, 7, true), 10); // "xy|": column 2 of "lmnop"
        assert_eq!(vertical(t, 11, false), 7); // column 3 on "xy" again: its end
        assert_eq!(vertical(t, 2, false), 2); // already on the first line: stays
        assert_eq!(vertical(t, 13, true), 13); // already on the last line: stays
    }

    #[test]
    fn columns_count_chars_not_bytes() {
        let t = "đỗ\nabc"; // đ and ỗ are 2 and 3 bytes
        let cursor = "đỗ".len();
        assert_eq!(vertical(t, cursor, true), "đỗ\nab".len());
    }
}
