//! A one-line text field (`.input` in the web UI): typing and Vietnamese Telex through the input method, the keys
//! a field needs (arrows, Home/End, Backspace/Delete, Enter, Escape, Ctrl+V), drawn with the app theme.
//! Emits `Changed` on every edit, `Submit` on Enter and `Cancel` on Escape.

use std::ops::Range;

use gpui::{
    App, Bounds, Context, ElementInputHandler, EventEmitter, FocusHandle, Focusable, KeyDownEvent, MouseButton, Pixels,
    SharedString, UTF16Selection, Window, canvas, div, prelude::*, px, rgb,
};

use crate::theme::{self, ACCENT, DIM, FG};

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
}

impl EventEmitter<InputEvent> for TextInput {}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl TextInput {
    pub fn new(placeholder: &str, cx: &mut Context<Self>) -> Self {
        TextInput { text: String::new(), cursor: 0, marked: None, after_commit: vec![], focus: cx.focus_handle(), placeholder: placeholder.to_string().into() }
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
        let with = with.replace(['\r', '\n', '\t'], " "); // one line: a pasted newline would submit nothing useful
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
        let edit_key = matches!(key, "backspace" | "delete" | "left" | "right" | "home" | "end" | "enter");
        if self.marked.is_some() && edit_key && !m.control && !m.alt {
            self.after_commit.push(key.to_string());
            cx.stop_propagation();
            return;
        }
        if self.apply_key(key, m.control && !m.alt, cx) {
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
            "home" => self.cursor = 0,
            "end" => self.cursor = self.text.len(),
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
        let caret = div().w(px(1.5)).h(px(15.)).flex_none().bg(rgb(FG));
        div().relative().h(px(28.)).px(px(9.)).flex().items_center().overflow_hidden().rounded(px(7.)).border_1()
            .border_color(if focused { theme::tint(ACCENT, 0.7) } else { theme::border2() }).bg(theme::ink(0.03))
            .text_size(px(12.5)).text_color(rgb(FG)).cursor_text()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| { window.focus(&this.focus); cx.notify(); }))
            .child(canvas(|_, _, _| {}, move |bounds, _, window, cx| {
                window.handle_input(&focus, ElementInputHandler::new(bounds, entity), cx);
            }).absolute().top_0().left_0().size_full())
            .when(empty && !focused, |d| d.child(div().text_color(rgb(DIM)).child(self.placeholder.clone())))
            .when(!empty || focused, |d| {
                d.child(div().flex_none().whitespace_nowrap().child(self.text[..self.cursor].to_string()))
                    .children(self.marked.clone().map(|m| div().flex_none().underline().child(m)))
                    .when(focused, |d| d.child(caret))
                    .child(div().flex_none().whitespace_nowrap().child(self.text[self.cursor..].to_string()))
            })
    }
}
