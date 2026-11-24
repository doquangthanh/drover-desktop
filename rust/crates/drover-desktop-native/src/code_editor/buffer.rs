//! The code editor's text and selection, with no drawing: byte offsets on char boundaries, line/column maths,
//! word moves, and undo / redo of whole edits (typing a word is one step). Unit-tested; the view is in view.rs.

use std::ops::Range;

/// Byte offsets of the start and end (before the `\n`) of the line holding `at`.
pub fn line_bounds(text: &str, at: usize) -> (usize, usize) {
    let start = text[..at].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let end = text[at..].find('\n').map(|i| at + i).unwrap_or(text.len());
    (start, end)
}

/// Cursor after Up (`down` false) or Down: same column (in chars) on the neighbouring line, or that line's end.
pub fn vertical(text: &str, cursor: usize, down: bool) -> usize {
    let (start, end) = line_bounds(text, cursor);
    let col = text[start..cursor].chars().count();
    let target = if down { if end >= text.len() { return cursor } else { end + 1 } } else { if start == 0 { return cursor } else { line_bounds(text, start - 1).0 } };
    let (ts, te) = line_bounds(text, target);
    text[ts..te].char_indices().nth(col).map(|(i, _)| ts + i).unwrap_or(te)
}

fn is_word(c: char) -> bool { c.is_alphanumeric() || c == '_' }

const UNDO_LIMIT: usize = 500;

#[derive(Clone, PartialEq, Debug)]
struct Snapshot { text: String, cursor: usize }

#[derive(Default)]
pub struct Buffer {
    pub text: String,
    /// Byte offset of the caret, on a char boundary.
    pub cursor: usize,
    /// The other end of the selection, when there is one.
    pub anchor: Option<usize>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    /// The last edit was typing at the caret: the next typed char joins its undo step.
    typing: bool,
}

impl Buffer {
    pub fn new(text: &str) -> Self {
        Buffer { text: text.replace("\r\n", "\n"), ..Default::default() }
    }

    /// Replace everything (a reload from disk): history is dropped, the caret stays on the same line when it can.
    pub fn reset(&mut self, text: &str) {
        let line = self.line_col(self.cursor).0;
        *self = Buffer::new(text);
        self.cursor = self.pos(line, 0);
    }

    pub fn selection(&self) -> Option<Range<usize>> {
        let a = self.anchor.filter(|a| *a != self.cursor)?;
        Some(a.min(self.cursor)..a.max(self.cursor))
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn selected_text(&self) -> Option<&str> { self.selection().map(|r| &self.text[r]) }

    /// 0-based line and column (in chars) of a byte offset.
    pub fn line_col(&self, at: usize) -> (usize, usize) {
        let at = at.min(self.text.len());
        let line = self.text[..at].matches('\n').count();
        let start = line_bounds(&self.text, at).0;
        (line, self.text[start..at].chars().count())
    }

    /// Byte offset of 0-based `line` and char column `col`, clamped to the text.
    pub fn pos(&self, line: usize, col: usize) -> usize {
        let start = self.text.split('\n').take(line).map(|l| l.len() + 1).sum::<usize>().min(self.text.len());
        let end = line_bounds(&self.text, start).1;
        self.text[start..end].char_indices().nth(col).map(|(i, _)| start + i).unwrap_or(end)
    }

    pub fn line_count(&self) -> usize { self.text.matches('\n').count() + 1 }

    fn prev(&self, at: usize) -> usize { self.text[..at].char_indices().next_back().map(|(i, _)| i).unwrap_or(0) }
    fn next(&self, at: usize) -> usize { self.text[at..].chars().next().map(|c| at + c.len_utf8()).unwrap_or(at) }

    /// Start of the word before `at` (Ctrl+Left), skipping spaces first.
    pub fn word_left(&self, at: usize) -> usize {
        let before: Vec<(usize, char)> = self.text[..at].char_indices().collect();
        let mut i = before.len();
        while i > 0 && before[i - 1].1.is_whitespace() { i -= 1; }
        let word = i > 0 && is_word(before[i - 1].1);
        while i > 0 && !before[i - 1].1.is_whitespace() && is_word(before[i - 1].1) == word { i -= 1; }
        before.get(i).map(|(b, _)| *b).unwrap_or(at)
    }

    /// End of the word after `at` (Ctrl+Right).
    pub fn word_right(&self, at: usize) -> usize {
        let mut it = self.text[at..].char_indices().peekable();
        while let Some(&(_, c)) = it.peek() { if c.is_whitespace() { it.next(); } else { break } }
        let word = it.peek().is_some_and(|&(_, c)| is_word(c));
        while let Some(&(_, c)) = it.peek() { if !c.is_whitespace() && is_word(c) == word { it.next(); } else { break } }
        it.peek().map(|&(i, _)| at + i).unwrap_or(self.text.len())
    }

    /// The word (letters, digits, `_`) around `at`, empty when `at` is not in or next to one (a double click).
    pub fn word_at(&self, at: usize) -> Range<usize> {
        let start = self.text[..at].char_indices().rev().take_while(|(_, c)| is_word(*c)).last().map(|(i, _)| i).unwrap_or(at);
        let end = self.text[at..].char_indices().find(|(_, c)| !is_word(*c)).map(|(i, _)| at + i).unwrap_or(self.text.len());
        start..end
    }

    /// Move the caret; `extend` keeps (or starts) a selection, otherwise it is dropped.
    pub fn move_to(&mut self, to: usize, extend: bool) {
        if extend { self.anchor.get_or_insert(self.cursor); } else { self.anchor = None; }
        self.cursor = to.min(self.text.len());
        self.typing = false;
    }

    pub fn left(&self) -> usize { self.prev(self.cursor) }
    pub fn right(&self) -> usize { self.next(self.cursor) }
    pub fn up(&self) -> usize { vertical(&self.text, self.cursor, false) }
    pub fn down(&self) -> usize { vertical(&self.text, self.cursor, true) }
    pub fn home(&self) -> usize {
        // First press: after the indentation; on it already: column 0.
        let (start, end) = line_bounds(&self.text, self.cursor);
        let indent = start + self.text[start..end].len() - self.text[start..end].trim_start().len();
        if self.cursor == indent { start } else { indent }
    }
    pub fn end(&self) -> usize { line_bounds(&self.text, self.cursor).1 }

    pub fn select_all(&mut self) {
        self.anchor = Some(0);
        self.cursor = self.text.len();
    }

    fn save(&mut self) {
        self.undo.push(Snapshot { text: self.text.clone(), cursor: self.cursor });
        if self.undo.len() > UNDO_LIMIT { self.undo.remove(0); }
        self.redo.clear();
    }

    /// Replace `range` with `with` as one undo step, caret after it.
    pub fn edit(&mut self, range: Range<usize>, with: &str) {
        let with = with.replace("\r\n", "\n").replace('\r', "\n");
        let typed = range.is_empty() && range.start == self.cursor && with.chars().count() == 1 && !with.contains('\n');
        if !(typed && self.typing) { self.save(); }
        self.text.replace_range(range.clone(), &with);
        self.cursor = range.start + with.len();
        self.anchor = None;
        self.typing = typed && with.chars().all(is_word);
    }

    /// Type `s` at the caret, over the selection when there is one.
    pub fn insert(&mut self, s: &str) {
        let r = self.selection().unwrap_or(self.cursor..self.cursor);
        self.edit(r, s);
    }

    /// Enter: a new line with the same indentation as the current one.
    pub fn newline(&mut self) {
        let (start, end) = line_bounds(&self.text, self.cursor);
        let line = &self.text[start..end];
        let indent: String = line.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
        self.insert(&format!("\n{indent}"));
    }

    /// Backspace (`forward` false) or Delete; `word` deletes a word (Ctrl).
    pub fn delete(&mut self, forward: bool, word: bool) {
        let r = match self.selection() {
            Some(r) => r,
            None if forward => self.cursor..if word { self.word_right(self.cursor) } else { self.right() },
            None => (if word { self.word_left(self.cursor) } else { self.left() })..self.cursor,
        };
        if !r.is_empty() { self.edit(r, ""); }
    }

    /// Tab / Shift+Tab: indent or outdent every line the selection touches; with no selection Tab inserts spaces.
    pub fn indent(&mut self, out: bool, unit: &str) {
        let Some(sel) = self.selection().or(out.then_some(self.cursor..self.cursor)) else { return self.insert(unit) };
        let start = line_bounds(&self.text, sel.start).0;
        let end = line_bounds(&self.text, sel.end).1;
        let block: Vec<String> = self.text[start..end].split('\n').map(|l| if out {
            let n = l.chars().take(unit.len()).take_while(|c| *c == ' ').count();
            l.strip_prefix('\t').unwrap_or(&l[n..]).to_string()
        } else { format!("{unit}{l}") }).collect();
        let new = block.join("\n");
        self.edit(start..end, &new);
        self.anchor = Some(start);
        self.cursor = start + new.len();
    }

    pub fn undo(&mut self) -> bool { self.step(true) }
    pub fn redo(&mut self) -> bool { self.step(false) }

    fn step(&mut self, back: bool) -> bool {
        let (from, to) = if back { (&mut self.undo, &mut self.redo) } else { (&mut self.redo, &mut self.undo) };
        let Some(s) = from.pop() else { return false };
        to.push(Snapshot { text: std::mem::replace(&mut self.text, s.text), cursor: self.cursor });
        self.cursor = s.cursor.min(self.text.len());
        self.anchor = None;
        self.typing = false;
        true
    }
}

#[cfg(test)]
#[path = "buffer_tests.rs"]
mod tests;
