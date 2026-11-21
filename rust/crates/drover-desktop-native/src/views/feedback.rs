//! Feedback (Herdr's "Feedback" frame): write what went wrong or what you want, and open a prefilled GitHub issue in
//! your browser. Nothing is sent from here: you read the issue on GitHub and press its button yourself. The only
//! thing added to your text is one visible line with the app version and your OS.

use gpui::{ClipboardItem, Context, Div, Entity, FontWeight, MouseButton, div, prelude::*, rgb};

use crate::actions::open_in_browser;
use crate::theme::{self, dim, done};
use crate::ui_scale::px;
use crate::views::text_input::TextInput;
use crate::views::ui::{self, Kind};
use crate::workspace_app::DroverApp;

const ISSUES: &str = "https://github.com/doquangthanh/drover-desktop/issues/new";
/// A URL longer than this is refused by some servers: the text is cut here (in characters, before encoding).
const MAX_BODY_CHARS: usize = 4000;

pub struct FeedbackForm {
    pub title: Entity<TextInput>,
    pub body: Entity<TextInput>,
    pub note: Option<String>,
}

impl FeedbackForm {
    pub fn new(cx: &mut Context<DroverApp>) -> Self {
        FeedbackForm { title: cx.new(|cx| TextInput::new("Short summary", cx)), body: cx.new(|cx| TextInput::new_multiline(cx)), note: None }
    }
}

/// The line added under the text: which build and OS the report is about.
pub fn environment() -> String {
    format!("Drover native {} · {} {}", env!("CARGO_PKG_VERSION"), std::env::consts::OS, std::env::consts::ARCH)
}

/// Percent-encode for a URL query: letters, digits and `-_.~` stay, every other byte (UTF-8 included) becomes `%XX`.
pub fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 3);
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') { out.push(b as char) } else { out.push_str(&format!("%{b:02X}")) }
    }
    out
}

/// The prefilled "new issue" address.
pub fn issue_url(title: &str, body: &str, env: &str) -> String {
    let body: String = body.chars().take(MAX_BODY_CHARS).collect();
    let full = if body.trim().is_empty() { format!("---\n{env}") } else { format!("{}\n\n---\n{env}", body.trim_end()) };
    format!("{ISSUES}?title={}&body={}", encode(title.trim()), encode(&full))
}

pub fn body(app: &DroverApp, cx: &mut Context<DroverApp>) -> Div {
    let f = &app.feedback;
    let head = |t: &str| div().text_size(px(11.)).text_color(rgb(dim())).font_weight(FontWeight::SEMIBOLD).child(t.to_string());
    let open = cx.listener(|app, _, _, cx| {
        let (title, body) = (app.feedback.title.read(cx).text.clone(), app.feedback.body.read(cx).text.clone());
        open_in_browser(&issue_url(&title, &body, &environment()));
        app.feedback.note = Some("Opened in your browser. Check the issue there and press “Submit new issue”.".into());
        cx.notify();
    });
    let copy = cx.listener(|app, _, _, cx| {
        let (title, body) = (app.feedback.title.read(cx).text.clone(), app.feedback.body.read(cx).text.clone());
        cx.write_to_clipboard(ClipboardItem::new_string(format!("{title}\n\n{body}\n\n{}", environment())));
        app.feedback.note = Some("Copied.".into());
        cx.notify();
    });
    div().flex().flex_col().flex_1().min_h_0().gap(px(10.)).p(px(18.)).max_w(px(820.))
        .child(head("TITLE"))
        .child(f.title.clone())
        .child(head("WHAT HAPPENED, OR WHAT DO YOU WANT?"))
        .child(div().flex().flex_col().flex_1().min_h_0().child(f.body.clone()))
        .child(div().text_size(px(12.)).text_color(rgb(dim())).child(format!("Added at the end: {}", environment())))
        .child(div().flex().items_center().gap(px(8.))
            .child(ui::button("Open GitHub issue", Kind::Primary).on_mouse_down(MouseButton::Left, open))
            .child(ui::button("Copy text", Kind::Plain).on_mouse_down(MouseButton::Left, copy))
            .when_some(f.note.clone(), |d, n| d.child(div().text_size(px(12.)).text_color(rgb(done())).child(n))))
        .child(div().text_size(px(12.)).text_color(rgb(theme::dim()))
            .child("Nothing is sent from here: the issue opens on GitHub, public, in the drover-desktop repository."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_reserved_characters_newlines_and_utf8() {
        assert_eq!(encode("a b&c=d"), "a%20b%26c%3Dd");
        assert_eq!(encode("x\ny"), "x%0Ay");
        assert_eq!(encode("đ"), "%C4%91");
        assert_eq!(encode("A-z_0.9~"), "A-z_0.9~");
    }

    #[test]
    fn the_issue_url_carries_title_text_and_one_environment_line() {
        let u = issue_url(" Crash on start ", "It closes.\n", "Drover native 1 · linux x86_64");
        assert!(u.starts_with("https://github.com/doquangthanh/drover-desktop/issues/new?title=Crash%20on%20start&body="));
        assert!(u.contains("It%20closes.%0A%0A---%0ADrover%20native%201"));
        assert!(issue_url("t", "", "env").ends_with("body=---%0Aenv"));
    }

    #[test]
    fn a_long_text_is_cut() {
        let long = "x".repeat(MAX_BODY_CHARS + 500);
        let u = issue_url("t", &long, "env");
        assert_eq!(u.matches('x').count(), MAX_BODY_CHARS);
    }
}
