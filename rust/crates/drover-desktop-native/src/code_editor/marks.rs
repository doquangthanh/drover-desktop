//! Git marks for the editor's gutter, from a unified diff of the file against HEAD (as the web UI's diff-marks.ts),
//! and the text "Send to agent" pastes into an agent's prompt (send-to-agent-logic.ts).

use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark { Added, Modified, Deleted }

/// Mark per 0-based line of the NEW file. Deleted: something was removed just before that line.
pub fn diff_marks(diff: &str) -> HashMap<usize, Mark> {
    let mut out = HashMap::new();
    let (mut n, mut in_hunk, mut removed) = (0usize, false, 0usize);
    let flush = |out: &mut HashMap<usize, Mark>, n: usize, removed: &mut usize| {
        if *removed > 0 { out.entry(n.max(1) - 1).or_insert(Mark::Deleted); *removed = 0; }
    };
    for line in diff.split('\n') {
        if let Some(start) = hunk_start(line) {
            flush(&mut out, n, &mut removed);
            n = start;
            in_hunk = true;
            continue;
        }
        if !in_hunk || line.starts_with('\\') { continue; }
        if line.starts_with('-') { removed += 1; continue; }
        if line.starts_with('+') {
            // An added line right after removed ones replaces one of them: a modification.
            let m = if removed > 0 { removed -= 1; Mark::Modified } else { Mark::Added };
            out.insert(n.max(1) - 1, m);
            n += 1;
            continue;
        }
        flush(&mut out, n, &mut removed);
        n += 1;
    }
    flush(&mut out, n, &mut removed);
    out
}

/// The new file's first line number of a "@@ -a,b +c,d @@" header.
fn hunk_start(line: &str) -> Option<usize> {
    let rest = line.strip_prefix("@@ -")?;
    let plus = rest.split_whitespace().nth(1)?.strip_prefix('+')?;
    plus.split(',').next()?.parse().ok()
}

/// "path:41-42" and the code in a fence; with no selection just the location and a space to type after.
pub fn format_selection(path: &str, start_line: usize, end_line: usize, text: &str) -> String {
    let at = if end_line > start_line { format!("{path}:{start_line}-{end_line}") } else { format!("{path}:{start_line}") };
    if text.trim().is_empty() { return format!("{at} ") }
    format!("{at}\n```\n{}\n```\n", text.strip_suffix('\n').unwrap_or(text))
}

/// Bracketed paste, so a snippet lands in the prompt instead of running line by line. Control characters other
/// than tab and newline are dropped: an ESC in the file could end the paste early and type the rest as keys.
pub fn as_paste(s: &str) -> String {
    let clean: String = s.replace("\r\n", "\n").chars().filter(|c| *c == '\n' || *c == '\t' || !c.is_control()).collect();
    format!("\x1b[200~{clean}\x1b[201~")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn added_modified_and_deleted_lines() {
        let diff = "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1,4 +1,5 @@\n a\n-b\n+B\n+new\n c\n-d\n e\n";
        let m = diff_marks(diff);
        assert_eq!(m.get(&1), Some(&Mark::Modified)); // b -> B on line 2
        assert_eq!(m.get(&2), Some(&Mark::Added)); // "new" on line 3
        assert_eq!(m.get(&4), Some(&Mark::Deleted)); // d removed before "e" (line 5)
        assert_eq!(m.len(), 3);
        assert!(diff_marks("").is_empty());
    }

    #[test]
    fn selection_text_and_safe_paste() {
        assert_eq!(format_selection("src/a.rs", 4, 5, "x\ny\n"), "src/a.rs:4-5\n```\nx\ny\n```\n");
        assert_eq!(format_selection("a.rs", 7, 7, ""), "a.rs:7 ");
        assert_eq!(as_paste("a\x1b[201~b\r\nc\td"), "\x1b[200~a[201~b\nc\td\x1b[201~");
    }
}
