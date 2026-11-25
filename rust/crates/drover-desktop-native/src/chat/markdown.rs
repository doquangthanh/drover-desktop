//! The few markdown blocks a chat reply needs, drawn without inline styling: fenced code in a monospace box,
//! headings (bold), bullets and paragraphs. Inline `code` and **bold** are left as typed.

use std::ops::Range;

use gpui::{AnyElement, Div, FontWeight, HighlightStyle, SharedString, StyledText, div, prelude::*, rgb};

use crate::theme::{self, fg};
use crate::ui_scale::px;

#[derive(Clone, Debug, PartialEq)]
pub enum Block { Code(String), Heading(String), Bullet(String), Para(String) }

fn bullet_text(line: &str) -> Option<&str> {
    let t = line.trim_start();
    if let Some(rest) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) { return Some(rest); }
    let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
    (digits > 0).then(|| t[digits..].strip_prefix(". ")).flatten()
}

pub fn parse(text: &str) -> Vec<Block> {
    let (mut out, mut code, mut para): (Vec<Block>, Option<Vec<&str>>, Vec<&str>) = (vec![], None, vec![]);
    let flush = |out: &mut Vec<Block>, para: &mut Vec<&str>| {
        if !para.is_empty() { out.push(Block::Para(para.join("\n"))); para.clear(); }
    };
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            match code.take() {
                Some(lines) => out.push(Block::Code(lines.join("\n"))),
                None => { flush(&mut out, &mut para); code = Some(vec![]); }
            }
        } else if let Some(lines) = &mut code {
            lines.push(line);
        } else if line.trim().is_empty() {
            flush(&mut out, &mut para);
        } else if line.starts_with('#') {
            flush(&mut out, &mut para);
            out.push(Block::Heading(line.trim_start_matches('#').trim().to_string()));
        } else if let Some(b) = bullet_text(line) {
            flush(&mut out, &mut para);
            out.push(Block::Bullet(b.to_string()));
        } else {
            para.push(line);
        }
    }
    if let Some(lines) = code { out.push(Block::Code(lines.join("\n"))); } // a reply cut off inside a fence
    flush(&mut out, &mut para);
    out
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Span { Bold, Code }

/// `**bold**` and `` `code` `` inside a line: the text without the marks, and where each span is. A mark with no
/// closing partner stays as typed.
pub fn inline(text: &str) -> (String, Vec<(Range<usize>, Span)>) {
    let (mut out, mut spans, mut i) = (String::new(), vec![], 0);
    while i < text.len() {
        let rest = &text[i..];
        let marked = rest.strip_prefix("**").map(|r| (r, "**", Span::Bold)).or_else(|| rest.strip_prefix('`').map(|r| (r, "`", Span::Code)));
        if let Some((r, mark, span)) = marked && let Some(end) = r.find(mark).filter(|e| *e > 0) {
            let start = out.len();
            out.push_str(&r[..end]);
            spans.push((start..out.len(), span));
            i += mark.len() * 2 + end;
            continue;
        }
        let ch = rest.chars().next().unwrap_or(' ');
        out.push(ch);
        i += ch.len_utf8();
    }
    (out, spans)
}

fn styled(text: &str) -> AnyElement {
    let (clean, spans) = inline(text);
    let highlights = spans.into_iter().map(|(r, s)| (r, match s {
        Span::Bold => HighlightStyle { font_weight: Some(FontWeight::BOLD), ..Default::default() },
        Span::Code => HighlightStyle { color: Some(rgb(theme::accent()).into()), background_color: Some(theme::ink(0.07).into()), ..Default::default() },
    })).collect::<Vec<_>>();
    StyledText::new(clean).with_highlights(highlights).into_any_element()
}

pub fn render(text: &str, mono: &SharedString) -> Div {
    let mut col = div().flex().flex_col().gap(px(8.)).text_size(px(13.5)).text_color(rgb(fg())).line_height(px(20.));
    for b in parse(text) {
        col = col.child(match b {
            Block::Para(t) => div().child(styled(&t)),
            Block::Heading(t) => div().font_weight(FontWeight::SEMIBOLD).text_size(px(15.)).child(styled(&t)),
            Block::Bullet(t) => div().flex().gap(px(8.)).child(div().flex_none().child("•")).child(div().flex_1().min_w_0().child(styled(&t))),
            Block::Code(t) => div().p(px(10.)).rounded(px(7.)).bg(theme::ink(0.05)).border_1().border_color(theme::border())
                .font_family(mono.clone()).text_size(px(12.5)).line_height(px(18.)).child(t),
        });
    }
    col
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_of_a_reply() {
        let b = parse("# Title\nsome text\nmore\n\n- one\n2. two\n```rust\nfn x() {}\n```\nafter");
        assert_eq!(b, vec![
            Block::Heading("Title".into()), Block::Para("some text\nmore".into()),
            Block::Bullet("one".into()), Block::Bullet("two".into()),
            Block::Code("fn x() {}".into()), Block::Para("after".into()),
        ]);
    }

    #[test]
    fn inline_marks() {
        let (t, s) = inline("use **bold** and `code` here");
        assert_eq!(t, "use bold and code here");
        assert_eq!(s, vec![(4..8, Span::Bold), (13..17, Span::Code)]);
        assert_eq!(inline("2 ** 3 and `open").0, "2 ** 3 and `open");
        assert_eq!(inline("a ``` b").0, "a ``` b");
        assert_eq!(inline("é**ü**").1, vec![(2..4, Span::Bold)]);
    }

    #[test]
    fn an_open_fence_is_kept_and_plain_text_is_one_paragraph() {
        assert_eq!(parse("```\nunfinished"), vec![Block::Code("unfinished".into())]);
        assert_eq!(parse("a\nb"), vec![Block::Para("a\nb".into())]);
        assert!(parse("").is_empty());
        assert_eq!(parse("3.14 is pi"), vec![Block::Para("3.14 is pi".into())]);
    }
}
