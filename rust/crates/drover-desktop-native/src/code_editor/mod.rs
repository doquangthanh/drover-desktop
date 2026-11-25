//! The code editor of the Files panel (the web UI uses Ace): one file's text with line numbers, git marks in the
//! gutter, simple syntax colours, selection, undo / redo, the clipboard and the input method (Vietnamese Telex too).
//! The Files view owns one per open tab (views/files/docs.rs) and saves through `files.write`.
//!
//! - buffer.rs: text, caret, selection, undo (pure, tested)
//! - input.rs: keys and the input method
//! - view.rs: drawing (only the visible rows), mouse
//! - syntax.rs / marks.rs: colours, gutter marks, the text "Send to agent" pastes

pub mod buffer;
mod display;
pub mod lines;
mod input;
pub mod marks;
pub mod syntax;
mod view;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use gpui::{App, EventEmitter, FocusHandle, Focusable, SharedString, UniformListScrollHandle};

use buffer::Buffer;
use marks::Mark;

pub enum EditorEvent {
    /// The text changed (the tab shows it unsaved).
    Changed,
    /// Ctrl+S.
    Save,
    /// Alt+S: send the selection (or the caret's line) to an agent.
    Send,
    /// Ctrl+J: the editor wide, or the agent's terminal beside it again.
    ToggleDock,
    /// Ctrl+F: find in this file.
    Find,
    /// Ctrl+G: go to a line.
    GotoLine,
}

pub struct CodeEditor {
    pub buf: Buffer,
    /// The text on disk the last time it was read or saved: the tab is unsaved while the buffer differs.
    saved: String,
    pub editable: bool,
    lang: Option<syntax::Lang>,
    pub marks: HashMap<usize, Mark>,
    /// Text the input method is still composing: drawn at the caret, not part of the buffer yet.
    marked: Option<String>,
    /// Keys pressed while a word was composed: they run after the input method commits it.
    after_commit: Vec<String>,
    focus: FocusHandle,
    scroll: UniformListScrollHandle,
    /// Bring the caret into view on the next paint.
    follow: bool,
    pub family: SharedString,
    /// Width of one character of the code font, in window pixels (measured while drawing).
    cell_w: f32,
    /// Window x where the text column starts and how wide it is (measured while drawing), for mouse placement.
    text_box: Arc<Mutex<(f32, f32)>>,
    /// Pixels the lines are scrolled to the left (long lines).
    hscroll: f32,
    dragging: bool,
}

impl EventEmitter<EditorEvent> for CodeEditor {}

impl Focusable for CodeEditor {
    fn focus_handle(&self, _: &App) -> FocusHandle { self.focus.clone() }
}

impl CodeEditor {
    pub fn new(path: &str, text: &str, editable: bool, cx: &mut gpui::Context<Self>) -> Self {
        let buf = Buffer::new(text);
        CodeEditor {
            saved: buf.text.clone(), buf, editable, lang: syntax::lang_for(path), marks: HashMap::new(),
            marked: None, after_commit: vec![], focus: cx.focus_handle(), scroll: UniformListScrollHandle::new(), follow: false,
            family: SharedString::default(), cell_w: 7.5, text_box: Arc::default(), hscroll: 0., dragging: false,
        }
    }

    pub fn dirty(&self) -> bool { self.buf.text != self.saved }

    /// The file changed on disk and has no local edits: show the new text, keep the caret's line.
    pub fn reload(&mut self, text: &str, editable: bool) {
        self.buf.reset(text);
        self.saved = self.buf.text.clone();
        self.editable = editable;
        self.follow = true;
    }

    /// After a save: this text is what is on disk now.
    pub fn mark_saved(&mut self, text: &str) { self.saved = text.replace("\r\n", "\n"); }

    /// Ctrl+/: comment or uncomment the selected lines with the language's line comment.
    pub fn toggle_comment(&mut self) -> bool {
        let Some(token) = self.lang.as_ref().and_then(|l| l.line_comment_token()) else { return false };
        lines::toggle_comment(&mut self.buf, token);
        self.follow = true;
        true
    }

    /// Select the next (or previous) match of `needle`: from the start of the selection when `restart` (typing in the
    /// find box), else after it. False when there is none.
    pub fn find_next(&mut self, needle: &str, forward: bool, restart: bool) -> bool {
        let sel = self.buf.selection();
        let from = match (&sel, forward, restart) {
            (Some(r), _, true) => r.start,
            (Some(r), true, false) => r.end,
            (Some(r), false, false) => r.start,
            (None, ..) => self.buf.cursor,
        };
        match lines::find(&self.buf.text, needle, from, forward) {
            Some(r) => { self.buf.anchor = Some(r.start); self.buf.cursor = r.end; self.follow = true; true }
            None => false,
        }
    }

    /// The selected text on one line (the box's start text for Ctrl+F), empty when nothing or several lines.
    pub fn selected_word(&self) -> String {
        self.buf.selected_text().filter(|t| !t.contains('\n')).unwrap_or("").to_string()
    }

    /// Put the caret on 1-based `line` (and `col`) and scroll there.
    pub fn goto(&mut self, line: usize, col: Option<usize>) {
        let at = self.buf.pos(line.saturating_sub(1), col.unwrap_or(1).saturating_sub(1));
        self.buf.move_to(at, false);
        self.follow = true;
    }

    /// 1-based first and last line of the selection (the caret's line without one) and the selected text.
    pub fn selection_lines(&self) -> (usize, usize, String) {
        let r = self.buf.selection().unwrap_or(self.buf.cursor..self.buf.cursor);
        // A selection that ends at the start of a line does not take that line.
        let end = if r.end > r.start && self.buf.text[..r.end].ends_with('\n') { r.end - 1 } else { r.end };
        let (a, b) = (self.buf.line_col(r.start).0 + 1, self.buf.line_col(end).0 + 1);
        (a, b, self.buf.text[r].to_string())
    }
}
