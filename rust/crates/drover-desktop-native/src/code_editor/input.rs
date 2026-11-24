//! The editor's keys (VS Code's: arrows, Ctrl+arrows by word, Shift to select, Ctrl+Z / Ctrl+Y, Ctrl+C / X / V,
//! Tab / Shift+Tab, Ctrl+S, Alt+S, Ctrl+J) and the input method, which brings every typed character (and composes
//! Telex words) through `replace_text_in_range`. Keys it does not use go on to the app's shortcuts.

use std::ops::Range;

use gpui::{Bounds, ClipboardItem, Context, KeyDownEvent, Pixels, UTF16Selection, Window};

use super::{CodeEditor, EditorEvent};

const INDENT: &str = "    ";
const PAGE: usize = 30;

impl CodeEditor {
    pub(super) fn on_key(&mut self, ev: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let m = &ev.keystroke.modifiers;
        let key = ev.keystroke.key.as_str();
        let edit_key = matches!(key, "backspace" | "delete" | "left" | "right" | "home" | "end" | "enter" | "up" | "down" | "tab");
        if self.marked.is_some() && edit_key && !m.control && !m.alt {
            self.after_commit.push(key.to_string());
            cx.stop_propagation();
            return;
        }
        if self.apply_key(key, m.control || m.platform, m.shift, m.alt, cx) {
            self.follow = true;
            cx.stop_propagation();
            cx.notify();
        }
    }

    /// Run one key. False when it is not the editor's (it goes on to the app).
    fn apply_key(&mut self, key: &str, ctrl: bool, shift: bool, alt: bool, cx: &mut Context<Self>) -> bool {
        let b = &mut self.buf;
        let ed = self.editable;
        let changed = match (ctrl, alt, key) {
            (false, false, "left") => { let to = if !shift && b.selection().is_some() { b.selection().map(|r| r.start).unwrap_or(0) } else { b.left() }; b.move_to(to, shift); false }
            (false, false, "right") => { let to = if !shift && b.selection().is_some() { b.selection().map(|r| r.end).unwrap_or(0) } else { b.right() }; b.move_to(to, shift); false }
            (true, false, "left") => { let to = b.word_left(b.cursor); b.move_to(to, shift); false }
            (true, false, "right") => { let to = b.word_right(b.cursor); b.move_to(to, shift); false }
            (false, false, "up") => { let to = b.up(); b.move_to(to, shift); false }
            (false, false, "down") => { let to = b.down(); b.move_to(to, shift); false }
            (false, false, "pageup" | "pagedown") => {
                let mut to = b.cursor;
                for _ in 0..PAGE { to = super::buffer::vertical(&b.text, to, key == "pagedown"); }
                b.move_to(to, shift);
                false
            }
            (false, false, "home") => { let to = b.home(); b.move_to(to, shift); false }
            (false, false, "end") => { let to = b.end(); b.move_to(to, shift); false }
            (true, false, "home") => { b.move_to(0, shift); false }
            (true, false, "end") => { let n = b.text.len(); b.move_to(n, shift); false }
            (true, false, "a") => { b.select_all(); false }
            (true, false, "c") => { self.copy(false, cx); false }
            (true, false, "x") if ed => self.copy(true, cx),
            (true, false, "v") if ed => match cx.read_from_clipboard().and_then(|i| i.text()) {
                Some(t) => { self.buf.insert(&t); true }
                None => false,
            },
            (true, false, "z") if ed && !shift => self.buf.undo(),
            (true, false, "y") if ed => self.buf.redo(),
            (true, false, "z") if ed => self.buf.redo(),
            (true, false, "s") => { cx.emit(EditorEvent::Save); false }
            (true, false, "j") => { cx.emit(EditorEvent::ToggleDock); false }
            (false, true, "s") => { cx.emit(EditorEvent::Send); false }
            (false, false, "backspace") if ed => { b.delete(false, false); true }
            (true, false, "backspace") if ed => { b.delete(false, true); true }
            (false, false, "delete") if ed => { b.delete(true, false); true }
            (true, false, "delete") if ed => { b.delete(true, true); true }
            (false, false, "enter") if ed => { b.newline(); true }
            (false, false, "tab") if ed => { b.indent(shift, INDENT); true }
            // Swallowed so they do not reach the terminal behind (read-only file, or nothing to do).
            (false, false, "backspace" | "delete" | "enter" | "tab") => false,
            (true, false, "x" | "v" | "z" | "y") => false,
            _ => return false,
        };
        if changed { cx.emit(EditorEvent::Changed); }
        true
    }

    /// Ctrl+C (or Ctrl+X with `cut`): the selection, or the whole line when nothing is selected (as VS Code).
    fn copy(&mut self, cut: bool, cx: &mut Context<Self>) -> bool {
        let b = &self.buf;
        let r = b.selection().unwrap_or_else(|| {
            let (s, e) = super::buffer::line_bounds(&b.text, b.cursor);
            s..(e + 1).min(b.text.len())
        });
        let text = b.text[r.clone()].to_string();
        let whole_line = b.selection().is_none() && !text.ends_with('\n');
        cx.write_to_clipboard(ClipboardItem::new_string(if whole_line { format!("{text}\n") } else { text }));
        if cut && !r.is_empty() { self.buf.edit(r, ""); return true }
        false
    }

    fn run_waiting_keys(&mut self, cx: &mut Context<Self>) {
        for key in std::mem::take(&mut self.after_commit) {
            self.apply_key(&key, false, false, false, cx);
        }
        cx.notify();
    }

    fn utf16_at(&self, byte: usize) -> usize { self.buf.text[..byte.min(self.buf.text.len())].encode_utf16().count() }

    fn byte_at(&self, units: usize) -> usize {
        let mut n = 0;
        for (i, ch) in self.buf.text.char_indices() {
            if n >= units { return i }
            n += ch.len_utf16();
        }
        self.buf.text.len()
    }

    fn byte_range(&self, r: &Range<usize>) -> Range<usize> { self.byte_at(r.start.min(r.end))..self.byte_at(r.start.max(r.end)) }
}

impl gpui::EntityInputHandler for CodeEditor {
    fn text_for_range(&mut self, r: Range<usize>, adjusted: &mut Option<Range<usize>>, _: &mut Window, _: &mut Context<Self>) -> Option<String> {
        let b = self.byte_range(&r);
        *adjusted = Some(self.utf16_at(b.start)..self.utf16_at(b.end));
        Some(self.buf.text[b].to_string())
    }

    fn selected_text_range(&mut self, _: bool, _: &mut Window, _: &mut Context<Self>) -> Option<UTF16Selection> {
        let r = self.buf.selection().unwrap_or(self.buf.cursor..self.buf.cursor);
        Some(UTF16Selection { range: self.utf16_at(r.start)..self.utf16_at(r.end), reversed: self.buf.anchor.is_some_and(|a| a > self.buf.cursor) })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        let c = self.utf16_at(self.buf.cursor);
        self.marked.as_ref().map(|t| c..c + t.encode_utf16().count())
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) { self.marked = None; }

    fn replace_text_in_range(&mut self, r: Option<Range<usize>>, text: &str, _: &mut Window, cx: &mut Context<Self>) {
        // While a word is composed its range is the marked text, which is not in the buffer: commit at the caret.
        let composing = self.marked.take().is_some();
        if !self.editable { return self.run_waiting_keys(cx) }
        match r.filter(|_| !composing).map(|r| self.byte_range(&r)) {
            Some(range) if !range.is_empty() => self.buf.edit(range, text),
            _ => self.buf.insert(text),
        }
        self.follow = true;
        cx.emit(EditorEvent::Changed);
        self.run_waiting_keys(cx);
    }

    fn replace_and_mark_text_in_range(&mut self, _: Option<Range<usize>>, text: &str, _: Option<Range<usize>>, _: &mut Window, cx: &mut Context<Self>) {
        self.marked = (!text.is_empty() && self.editable).then(|| text.to_string());
        if self.marked.is_none() { self.run_waiting_keys(cx); }
        cx.notify();
    }

    fn bounds_for_range(&mut self, _: Range<usize>, element: Bounds<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<Bounds<Pixels>> {
        Some(element)
    }

    fn character_index_for_point(&mut self, _: gpui::Point<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<usize> { None }
}
