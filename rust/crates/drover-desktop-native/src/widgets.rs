//! Small building blocks shared by the shell: icons, avatars, agent badges, state marks, labels.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gpui::{Animation, AnimationExt, Div, IntoElement, Styled, Svg, Transformation, div, percentage, prelude::*, px, rgb, svg};

use crate::theme::{self, AVATAR, blocked, dim, done, error, working, kind_badge, tint};

pub fn icon(name: &str, size: f32, color: u32) -> Svg {
    svg().path(format!("icons/{name}.svg")).size(px(size)).flex_none().text_color(rgb(color))
}

/// Square letter badge for a space (`space-avatar.tsx`): 18 px, radius 5.
pub fn avatar(name: &str, color: i64) -> Div {
    let letter = name.chars().find(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_uppercase().to_string()).unwrap_or("?".into());
    div().size(px(18.)).rounded(px(5.)).flex_none().flex().items_center().justify_center()
        .bg(rgb(AVATAR[(color.rem_euclid(8)) as usize])).text_color(rgb(0xffffff)).text_size(px(10.5)).font_weight(gpui::FontWeight::SEMIBOLD)
        .child(letter)
}

/// Agent kind badge (`kind-icon.tsx`): 14 px, radius 4.
pub fn kind_icon(kind: &str) -> Div {
    let (letter, bg, fg) = kind_badge(kind);
    div().size(px(14.)).rounded(px(4.)).flex_none().flex().items_center().justify_center()
        .bg(rgb(bg)).text_color(rgb(fg)).text_size(px(9.)).font_weight(gpui::FontWeight::BOLD).child(letter)
}

fn pill(text: String, color: u32) -> Div {
    div().h(px(18.)).px(px(6.)).rounded(px(5.)).flex_none().flex().items_center()
        .bg(tint(color, 0.16)).text_color(rgb(color)).text_size(px(10.5)).font_weight(gpui::FontWeight::SEMIBOLD).child(text)
}

/// What a pane's state looks like next to its name (`state-mark.tsx`).
pub fn state_mark(state: &str, exit_code: Option<i64>, id: impl Into<gpui::ElementId>) -> gpui::AnyElement {
    match state {
        "blocked" => pill("Needs you".into(), blocked()).into_any_element(),
        "working" => icon("spin", 11., working())
            .with_animation(id, Animation::new(Duration::from_millis(900)).repeat(),
                            |s, t| s.with_transformation(Transformation::rotate(percentage(t))))
            .into_any_element(),
        "done" => div().size(px(6.)).rounded_full().flex_none().bg(rgb(done())).into_any_element(),
        "exited" => pill(exit_code.filter(|c| *c != 0).map(|c| format!("exit {c}")).unwrap_or("exited".into()), error()).into_any_element(),
        _ => div().size(px(7.)).rounded_full().flex_none().border(px(1.5)).border_color(rgb(dim())).into_any_element(),
    }
}

/// "3h", "2d", "<1m" since a unix time (seconds).
pub fn age(since: f64) -> String {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(since);
    let s = (now - since).max(0.0) as u64;
    match s {
        0..60 => "<1m".into(),
        60..3600 => format!("{}m", s / 60),
        3600..86400 => format!("{}h", s / 3600),
        _ => format!("{}d", s / 86400),
    }
}

/// "…/me/herd": the last two folders of a path, as the pane header shows it.
pub fn short_path(path: &str) -> String {
    let parts: Vec<&str> = path.trim_end_matches('/').rsplit('/').take(2).collect();
    match parts.as_slice() {
        [last, parent] => format!("../{parent}/{last}"),
        [last] => (*last).to_string(),
        _ => path.to_string(),
    }
}

/// A key cap like the status bar's `Ctrl+K` hint.
pub fn key_cap(text: &str) -> Div {
    div().h(px(17.)).px(px(5.)).rounded(px(4.)).flex().items_center().border_1().border_color(theme::border2())
        .text_size(px(10.5)).text_color(rgb(dim())).child(text.to_string())
}
