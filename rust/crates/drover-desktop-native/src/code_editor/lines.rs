//! Line commands of the editor, as Ace (the web editor) and VS Code have them: toggle a comment, move, duplicate and
//! delete the selected lines, and find text. Each is one undo step on the `Buffer`; unit-tested, no drawing.

use std::ops::Range;

use super::buffer::{Buffer, line_bounds};

/// Byte range of the whole lines the selection touches (the caret's line without one), without the last `\n`.
/// A selection that ends at the start of a line does not take that line.
fn block(buf: &Buffer) -> Range<usize> {
    let r = buf.selection().unwrap_or(buf.cursor..buf.cursor);
    let end = if r.end > r.start && buf.text[..r.end].ends_with('\n') { r.end - 1 } else { r.end };
    line_bounds(&buf.text, r.start).0..line_bounds(&buf.text, end).1
}

/// Select `from..to` (anchor first, caret last).
fn select(buf: &mut Buffer, from: usize, to: usize) {
    buf.anchor = Some(from);
    buf.cursor = to;
}

/// Ctrl+/: put `token` and a space at the lines' common indent, or take them off when every non-empty line has them.
pub fn toggle_comment(buf: &mut Buffer, token: &str) {
    let r = block(buf);
    let text = buf.text[r.clone()].to_string();
    let lines: Vec<&str> = text.split('\n').collect();
    let body: Vec<&&str> = lines.iter().filter(|l| !l.trim().is_empty()).collect();
    if body.is_empty() { return }
    let all = body.iter().all(|l| l.trim_start().starts_with(token));
    let indent = body.iter().map(|l| l.len() - l.trim_start().len()).min().unwrap_or(0);
    let out: Vec<String> = lines.iter().map(|l| {
        if l.trim().is_empty() { l.to_string() }
        else if all {
            let at = l.len() - l.trim_start().len();
            let rest = &l[at + token.len()..];
            format!("{}{}", &l[..at], rest.strip_prefix(' ').unwrap_or(rest))
        } else { format!("{}{token} {}", &l[..indent], &l[indent..]) }
    }).collect();
    let new = out.join("\n");
    let start = r.start;
    buf.edit(r, &new);
    select(buf, start, start + new.len());
}

/// Alt+Up / Alt+Down: the lines swap places with the line above / below. False at the top or bottom of the file.
pub fn move_lines(buf: &mut Buffer, up: bool) -> bool {
    let r = block(buf);
    let (sel_a, sel_c) = (buf.anchor, buf.cursor);
    let moved = buf.text[r.clone()].to_string();
    if up {
        if r.start == 0 { return false }
        let (ps, pe) = line_bounds(&buf.text, r.start - 1);
        let prev = buf.text[ps..pe].to_string();
        buf.edit(ps..r.end, &format!("{moved}\n{prev}"));
        let by = prev.len() + 1;
        buf.anchor = sel_a.map(|a| a - by);
        buf.cursor = sel_c - by;
    } else {
        if r.end >= buf.text.len() { return false }
        let (ns, ne) = line_bounds(&buf.text, r.end + 1);
        let next = buf.text[ns..ne].to_string();
        buf.edit(r.start..ne, &format!("{next}\n{moved}"));
        let by = next.len() + 1;
        buf.anchor = sel_a.map(|a| a + by);
        buf.cursor = sel_c + by;
    }
    true
}

/// Ctrl+Shift+D: a copy of the lines right below them; the caret goes to the copy.
pub fn duplicate_lines(buf: &mut Buffer) {
    let r = block(buf);
    let (sel_a, sel_c) = (buf.anchor, buf.cursor);
    let copy = buf.text[r.clone()].to_string();
    buf.edit(r.end..r.end, &format!("\n{copy}"));
    let by = copy.len() + 1;
    buf.anchor = sel_a.map(|a| a + by);
    buf.cursor = sel_c + by;
}

/// Ctrl+Shift+K: delete the lines, the caret at the start of the line that takes their place.
pub fn delete_lines(buf: &mut Buffer) {
    let r = block(buf);
    let last = r.end >= buf.text.len();
    let range = if last && r.start > 0 { r.start - 1..r.end } else if last { r } else { r.start..r.end + 1 };
    let start = range.start;
    buf.edit(range, "");
    let (s, _) = line_bounds(&buf.text, start.min(buf.text.len()));
    buf.cursor = s;
    buf.anchor = None;
}

/// The next (or previous) match of `needle` from `from`, wrapping round the file; case ignored.
pub fn find(text: &str, needle: &str, from: usize, forward: bool) -> Option<Range<usize>> {
    if needle.is_empty() { return None }
    let (hay, n) = (text.to_lowercase(), needle.to_lowercase());
    // Lowercasing can change byte lengths for a few letters: only search when offsets still line up.
    if hay.len() != text.len() { return text.find(needle).map(|i| i..i + needle.len()) }
    let from = from.min(hay.len());
    let hit = if forward {
        hay[from..].find(&n).map(|i| from + i).or_else(|| hay.find(&n))
    } else {
        hay[..from].rfind(&n).or_else(|| hay.rfind(&n))
    };
    hit.map(|i| i..i + n.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(text: &str, at: usize) -> Buffer { let mut b = Buffer::new(text); b.cursor = at; b }

    #[test]
    fn comment_toggles_on_and_off_at_the_common_indent() {
        let mut b = buf("a\n  b\n\n  c", 0);
        b.anchor = Some(b.text.len());
        toggle_comment(&mut b, "//");
        assert_eq!(b.text, "// a\n//   b\n\n//   c");
        toggle_comment(&mut b, "//");
        assert_eq!(b.text, "a\n  b\n\n  c");
    }

    #[test]
    fn lines_move_and_stop_at_the_ends() {
        let mut b = buf("one\ntwo\nthree", 5);
        assert!(move_lines(&mut b, true));
        assert_eq!(b.text, "two\none\nthree");
        assert_eq!(b.cursor, 1);
        assert!(!move_lines(&mut b, true));
        assert!(move_lines(&mut b, false) && move_lines(&mut b, false));
        assert_eq!(b.text, "one\nthree\ntwo");
        assert!(!move_lines(&mut b, false));
    }

    #[test]
    fn duplicate_and_delete_lines() {
        let mut b = buf("a\nb", 0);
        duplicate_lines(&mut b);
        assert_eq!((b.text.as_str(), b.cursor), ("a\na\nb", 2));
        delete_lines(&mut b);
        assert_eq!(b.text, "a\nb");
        let mut last = buf("a\nb", 3);
        delete_lines(&mut last);
        assert_eq!(last.text, "a");
        let mut only = buf("x", 0);
        delete_lines(&mut only);
        assert_eq!(only.text, "");
    }

    #[test]
    fn find_wraps_and_ignores_case() {
        let t = "Foo bar foo";
        assert_eq!(find(t, "foo", 0, true), Some(0..3));
        assert_eq!(find(t, "foo", 1, true), Some(8..11));
        assert_eq!(find(t, "foo", 9, true), Some(0..3));
        assert_eq!(find(t, "foo", 8, false), Some(0..3));
        assert_eq!(find(t, "zzz", 0, true), None);
    }
}
