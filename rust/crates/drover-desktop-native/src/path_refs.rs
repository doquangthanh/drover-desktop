//! File references in terminal output ("src/core/keys.ts:41", "Update(src/a.tsx)", "C:\proj\a.py(12,3)").
//! The same rules as the web UI's `frontend/src/files/path-refs.ts` (and its tests): a name with a known extension,
//! optionally followed by `:line[:col]` or `(line[,col])`; web addresses are left to the link handler.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathRef {
    /// Char indexes of the reference in the line (end exclusive), including its `:line:col` suffix.
    pub start: usize,
    pub end: usize,
    pub path: String,
    pub line: Option<u32>,
    pub col: Option<u32>,
}

const EXT: [&str; 40] = [
    "ts", "tsx", "js", "jsx", "mjs", "cjs", "json", "md", "py", "rs", "go", "toml", "yaml", "yml", "css", "scss", "html", "htm", "sh", "sql",
    "java", "c", "h", "cpp", "hpp", "cs", "php", "rb", "kt", "swift", "lua", "ini", "cfg", "txt", "xml", "svg", "lock", "ps1", "bat", "vue",
];

fn is_token(c: char) -> bool { c.is_alphanumeric() || "_.@~/\\:-".contains(c) }

fn digits(s: &[char]) -> Option<u32> {
    if s.is_empty() || !s.iter().all(|c| c.is_ascii_digit()) { return None }
    s.iter().collect::<String>().parse().ok()
}

/// Every file reference in one terminal line.
pub fn find_path_refs(line: &[char]) -> Vec<PathRef> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < line.len() {
        if !is_token(line[i]) { i += 1; continue }
        let start = i;
        while i < line.len() && is_token(line[i]) { i += 1 }
        let mut end = i;
        while end > start && matches!(line[end - 1], ':' | '.' | '-') { end -= 1 } // "keys.ts:" at the end of a sentence
        if end <= start { continue }
        let tok = &line[start..end];
        if tok.windows(3).any(|w| w == [':', '/', '/']) { continue } // https://..., ftp://...
        // `:line` and `:line:col` at the end.
        let mut core = tok.len();
        let mut nums: Vec<u32> = Vec::new();
        for _ in 0..2 {
            let Some(pos) = tok[..core].iter().rposition(|c| *c == ':') else { break };
            match digits(&tok[pos + 1..core]) {
                Some(n) if pos >= 2 || (pos == 1 && !tok[0].is_ascii_alphabetic()) => { nums.push(n); core = pos; }
                _ => break,
            }
        }
        nums.reverse();
        let (mut line_no, mut col_no) = (nums.first().copied(), nums.get(1).copied());
        // `(line)` or `(line,col)` right after the name.
        if nums.is_empty() && line.get(end) == Some(&'(') {
            if let Some(close) = line[end..].iter().position(|c| *c == ')') {
                let inner: String = line[end + 1..end + close].iter().collect();
                let mut parts = inner.split(',');
                if let Some(l) = parts.next().and_then(|p| p.trim().parse::<u32>().ok()) {
                    line_no = Some(l);
                    col_no = parts.next().and_then(|p| p.trim().parse::<u32>().ok());
                    end += close + 1;
                }
            }
        }
        let name = &tok[..core];
        let text: String = name.iter().collect();
        let rest = if name.len() > 1 && name[1] == ':' && name[0].is_ascii_alphabetic() { &text[2..] } else { &text[..] };
        if rest.contains(':') { continue }
        let base = rest.rsplit(['/', '\\']).next().unwrap_or("");
        let ok = base.rsplit_once('.').is_some_and(|(stem, ext)| !stem.is_empty() && EXT.contains(&ext));
        if ok {
            out.push(PathRef { start, end, path: text, line: line_no, col: col_no });
        }
    }
    out
}

fn slash(p: &str) -> String {
    let mut out = String::with_capacity(p.len());
    for c in p.replace('\\', "/").chars() {
        if c == '/' && out.ends_with('/') { continue }
        out.push(c);
    }
    out
}

fn is_abs(p: &str) -> bool {
    let b = p.as_bytes();
    p.starts_with('/') || (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'/')
}

/// Collapse "." and ".." segments of a slash-separated path.
fn collapse(p: &str) -> String {
    let root = if p.starts_with('/') { "/" } else if is_abs(p) { &p[..3] } else { "" };
    let mut parts: Vec<&str> = Vec::new();
    for seg in p[root.len()..].split('/') {
        match seg {
            "" | "." => {}
            ".." => { parts.pop(); }
            s => parts.push(s),
        }
    }
    format!("{root}{}", parts.join("/"))
}

/// Path of `reference` relative to the space folder (the form the web Files view uses), or None when it is outside the
/// space. A relative reference is resolved against the pane's folder.
pub fn to_space_path(reference: &str, pane_cwd: &str, space_cwd: &str) -> Option<String> {
    if space_cwd.is_empty() { return None }
    let space = collapse(&slash(space_cwd));
    let space = space.trim_end_matches('/').to_string();
    let base = if pane_cwd.is_empty() { space.clone() } else { collapse(&slash(pane_cwd)) };
    let p = slash(reference);
    let full = collapse(&if is_abs(&p) { p } else { format!("{base}/{p}") });
    // Drive paths compare case-insensitively. ASCII-only lowering keeps byte offsets, so `full` can be sliced below
    // (full Unicode lowering can change a char's byte length, e.g. the Kelvin sign).
    let win = space.as_bytes().get(1) == Some(&b':');
    let (a, b) = if win { (full.to_ascii_lowercase(), space.to_ascii_lowercase()) } else { (full.clone(), space.clone()) };
    a.strip_prefix(&format!("{b}/")).map(|_| full[space.len() + 1..].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refs(s: &str) -> Vec<PathRef> { find_path_refs(&s.chars().collect::<Vec<_>>()) }
    fn slice(s: &str, r: &PathRef) -> String { s.chars().skip(r.start).take(r.end - r.start).collect() }

    #[test]
    fn finds_a_path_with_line_and_column() {
        let s = "error in src/core/keys.ts:41:7 here";
        let r = &refs(s)[0];
        assert_eq!((r.path.as_str(), r.line, r.col), ("src/core/keys.ts", Some(41), Some(7)));
        assert_eq!(slice(s, r), "src/core/keys.ts:41:7");
    }

    #[test]
    fn finds_a_path_inside_an_agent_tool_line() {
        let s = "● Update(src/changes/diff-view.tsx)";
        let r = &refs(s)[0];
        assert_eq!(r.path, "src/changes/diff-view.tsx");
        assert_eq!(slice(s, r), "src/changes/diff-view.tsx");
    }

    #[test]
    fn reads_the_paren_form_and_windows_paths() {
        let r = &refs("C:\\proj\\a.py(12,3): error")[0];
        assert_eq!((r.path.as_str(), r.line, r.col), ("C:\\proj\\a.py", Some(12), Some(3)));
        assert_eq!(refs("src/b.ts(10,2): error TS1")[0].line, Some(10));
    }

    #[test]
    fn prefers_the_whole_extension() {
        assert_eq!(refs("a/b.tsx")[0].path, "a/b.tsx");
        assert_eq!(refs("styles/x.css")[0].path, "styles/x.css");
    }

    #[test]
    fn leaves_urls_and_plain_words_alone() {
        assert!(refs("see https://github.com/a/b/c.ts for more").is_empty());
        assert!(refs("v1.2 e.g. done, see github.com").is_empty());
    }

    #[test]
    fn finds_relative_paths_and_several_per_line() {
        let r = refs("./a.ts and ../b/c.py:3");
        assert_eq!(r.iter().map(|x| x.path.as_str()).collect::<Vec<_>>(), vec!["./a.ts", "../b/c.py"]);
        assert_eq!(r[1].line, Some(3));
    }

    #[test]
    fn trailing_punctuation_is_not_part_of_it() {
        let s = "failed in keys.ts:41: boom";
        let r = &refs(s)[0];
        assert_eq!((r.path.as_str(), r.line), ("keys.ts", Some(41)));
        assert_eq!(refs("open keys.ts.")[0].path, "keys.ts");
    }

    #[test]
    fn resolves_against_the_pane_folder() {
        assert_eq!(to_space_path("keys.ts", "/w/p/src/core", "/w/p").as_deref(), Some("src/core/keys.ts"));
        assert_eq!(to_space_path("../a.ts", "/w/p/src/core", "/w/p").as_deref(), Some("src/a.ts"));
    }

    #[test]
    fn rejects_paths_outside_the_space() {
        assert_eq!(to_space_path("/w/p/x/y.ts", "/w/p", "/w/p").as_deref(), Some("x/y.ts"));
        assert_eq!(to_space_path("/other/y.ts", "/w/p", "/w/p"), None);
        assert_eq!(to_space_path("../../y.ts", "/w/p", "/w/p"), None);
        assert_eq!(to_space_path("a.ts", "/x", ""), None);
    }

    #[test]
    fn drive_paths_ignore_ascii_case_only() {
        assert_eq!(to_space_path("c:\\W\\P\\x.ts", "", "C:\\w\\p").as_deref(), Some("x.ts"));
        // U+212A (Kelvin sign, 3 bytes) lowercases to `k` (1 byte): it must not match `k`, or the slice is off.
        assert_eq!(to_space_path("C:/W/kx/ab.ts", "", "C:/W/\u{212A}x"), None);
    }

    #[test]
    fn windows_drive_paths_compare_case_insensitively() {
        assert_eq!(to_space_path("src\\a.ts", "C:\\Users\\me\\proj", "C:\\Users\\me\\proj").as_deref(), Some("src/a.ts"));
        assert_eq!(to_space_path("c:/users/me/proj/src/a.ts", "C:\\Users\\me\\proj", "C:\\Users\\me\\proj").as_deref(), Some("src/a.ts"));
    }
}
