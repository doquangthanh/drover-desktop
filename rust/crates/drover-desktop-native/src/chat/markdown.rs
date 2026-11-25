//! The few markdown blocks a chat reply needs, drawn without inline styling: fenced code in a monospace box,
//! headings (bold), bullets and paragraphs. Inline `code` and **bold** are left as typed.

use gpui::{Div, FontWeight, SharedString, div, prelude::*, rgb};

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

pub fn render(text: &str, mono: &SharedString) -> Div {
    let mut col = div().flex().flex_col().gap(px(8.)).text_size(px(13.5)).text_color(rgb(fg())).line_height(px(20.));
    for b in parse(text) {
        col = col.child(match b {
            Block::Para(t) => div().child(t),
            Block::Heading(t) => div().font_weight(FontWeight::SEMIBOLD).text_size(px(15.)).child(t),
            Block::Bullet(t) => div().flex().gap(px(8.)).child(div().flex_none().child("•")).child(div().flex_1().min_w_0().child(t)),
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
    fn an_open_fence_is_kept_and_plain_text_is_one_paragraph() {
        assert_eq!(parse("```\nunfinished"), vec![Block::Code("unfinished".into())]);
        assert_eq!(parse("a\nb"), vec![Block::Para("a\nb".into())]);
        assert!(parse("").is_empty());
        assert_eq!(parse("3.14 is pi"), vec![Block::Para("3.14 is pi".into())]);
    }
}
