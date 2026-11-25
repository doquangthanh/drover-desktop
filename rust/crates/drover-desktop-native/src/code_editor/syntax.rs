//! A small line-by-line highlighter: comments, strings, numbers and keywords, picked by file extension. No grammar,
//! no state across lines (a block comment colours only its own lines' `/*` .. `*/` parts), so it stays fast on any file.

use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tok { Comment, Str, Num, Key }

pub struct Lang {
    line_comment: &'static [&'static str],
    keywords: &'static [&'static str],
}

const RUST: &[&str] = &["as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "false", "fn", "for", "if", "impl",
    "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true",
    "type", "unsafe", "use", "where", "while", "Some", "None", "Ok", "Err"];
const JS: &[&str] = &["async", "await", "break", "case", "catch", "class", "const", "continue", "default", "delete", "do", "else", "export",
    "extends", "false", "finally", "for", "from", "function", "if", "import", "in", "instanceof", "interface", "let", "new", "null", "of",
    "return", "static", "super", "switch", "this", "throw", "true", "try", "type", "typeof", "undefined", "var", "void", "while", "yield"];
const PY: &[&str] = &["and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif", "else", "except", "False",
    "finally", "for", "from", "global", "if", "import", "in", "is", "lambda", "None", "nonlocal", "not", "or", "pass", "raise", "return",
    "self", "True", "try", "while", "with", "yield"];
const GO_C: &[&str] = &["break", "case", "char", "const", "continue", "default", "defer", "do", "double", "else", "enum", "extern", "false",
    "float", "for", "func", "go", "goto", "if", "import", "int", "interface", "long", "map", "nil", "package", "range", "return", "select",
    "sizeof", "static", "struct", "switch", "true", "type", "typedef", "unsigned", "var", "void", "while", "class", "public", "private",
    "protected", "new", "this", "namespace", "using", "null"];
const SHELL: &[&str] = &["case", "do", "done", "elif", "else", "esac", "export", "fi", "for", "function", "if", "in", "local", "return",
    "then", "until", "while", "true", "false"];
const DATA: &[&str] = &["true", "false", "null"];

impl Lang {
    /// The token a line comment starts with (`//`, `#`…), for Ctrl+/.
    pub fn line_comment_token(&self) -> Option<&'static str> { self.line_comment.first().copied() }
}

/// The language of a file name; None for plain text (Markdown, logs …): drawn without colours.
pub fn lang_for(path: &str) -> Option<Lang> {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    let (line_comment, keywords): (&'static [&'static str], &'static [&'static str]) = match ext.as_str() {
        "rs" => (&["//"], RUST),
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" => (&["//"], JS),
        "py" | "pyi" => (&["#"], PY),
        "go" | "c" | "h" | "cpp" | "hpp" | "cc" | "java" | "kt" | "cs" | "swift" => (&["//"], GO_C),
        "sh" | "bash" | "zsh" | "ps1" => (&["#"], SHELL),
        "toml" | "yaml" | "yml" | "ini" | "conf" => (&["#"], DATA),
        "json" | "css" | "scss" => (&["//"], DATA),
        "sql" | "lua" => (&["--"], DATA),
        _ => return None,
    };
    Some(Lang { line_comment, keywords })
}

/// Coloured parts of one line, as sorted, non-overlapping byte ranges.
pub fn highlight(lang: &Lang, line: &str) -> Vec<(Range<usize>, Tok)> {
    let b = line.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let rest = &line[i..];
        if lang.line_comment.iter().any(|c| rest.starts_with(c)) {
            out.push((i..b.len(), Tok::Comment));
            break;
        }
        if rest.starts_with("/*") {
            let end = rest.find("*/").map(|e| i + e + 2).unwrap_or(b.len());
            out.push((i..end, Tok::Comment));
            i = end;
            continue;
        }
        let c = b[i];
        if c == b'"' || c == b'\'' || c == b'`' {
            // A Rust lifetime ('a) is not a string: only take ' when it closes on this line soon.
            let mut j = i + 1;
            while j < b.len() && b[j] != c { j += if b[j] == b'\\' { 2 } else { 1 }; }
            if c == b'\'' && (j >= b.len() || j - i > 4 && lang.keywords == RUST) { i += 1; continue; }
            let end = (j + 1).min(b.len());
            out.push((i..end, Tok::Str));
            i = end;
            continue;
        }
        if c.is_ascii_digit() && (i == 0 || !is_ident(b[i - 1])) {
            let mut j = i;
            while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'.' || b[j] == b'_') { j += 1; }
            out.push((i..j, Tok::Num));
            i = j;
            continue;
        }
        if is_ident(c) {
            let mut j = i;
            while j < b.len() && is_ident(b[j]) { j += 1; }
            if lang.keywords.contains(&&line[i..j]) { out.push((i..j, Tok::Key)); }
            i = j;
            continue;
        }
        i += line[i..].chars().next().map(char::len_utf8).unwrap_or(1);
    }
    out
}

fn is_ident(c: u8) -> bool { c.is_ascii_alphanumeric() || c == b'_' || c == b'$' }

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(path: &str, line: &str) -> Vec<(&'static str, Tok)> {
        let lang = lang_for(path).unwrap();
        highlight(&lang, line).into_iter().map(|(r, t)| (&*Box::leak(line[r].to_string().into_boxed_str()), t)).collect()
    }

    #[test]
    fn keywords_strings_numbers_and_comments() {
        assert_eq!(kinds("a.rs", r#"let s = "a\"b"; // hi"#),
            vec![("let", Tok::Key), (r#""a\"b""#, Tok::Str), ("// hi", Tok::Comment)]);
        assert_eq!(kinds("a.py", "x = 42  # n"), vec![("42", Tok::Num), ("# n", Tok::Comment)]);
        assert_eq!(kinds("a.ts", "const v2 = 'x' /* c */ + 1"),
            vec![("const", Tok::Key), ("'x'", Tok::Str), ("/* c */", Tok::Comment), ("1", Tok::Num)]);
    }

    #[test]
    fn rust_lifetimes_and_unicode_are_safe() {
        assert_eq!(kinds("a.rs", "fn f<'a>(x: &'a str)"), vec![("fn", Tok::Key)]);
        assert_eq!(kinds("a.rs", "// đỗ"), vec![("// đỗ", Tok::Comment)]);
        assert!(lang_for("README.md").is_none());
        assert_eq!(kinds("a.rs", "\"open"), vec![("\"open", Tok::Str)]); // an unclosed string runs to the end
    }
}
