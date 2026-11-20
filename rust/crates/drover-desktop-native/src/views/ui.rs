//! Building blocks the views share (`views.css` in the web UI): view header, split columns, list rows, buttons,
//! tab strip, toggle, empty / loading / error states.

#![allow(dead_code)] // shared by the views ported in phases 01-05; not all are used yet

use gpui::{Div, FontWeight, div, prelude::*, px, rgb};

use crate::theme::{self, accent, chrome, dim, error, fg, muted, on_accent, UI_FONT};
use crate::widgets::icon;

/// The bar on top of a view: icon, title, then whatever the view adds on the right.
pub fn header(icon_name: &str, title: &str) -> Div {
    div().h(px(42.)).flex_none().flex().items_center().gap(px(8.)).px(px(14.)).bg(rgb(chrome()))
        .border_b_1().border_color(theme::border()).font_family(UI_FONT)
        .child(icon(icon_name, 15., muted()))
        .child(div().text_size(px(13.5)).font_weight(FontWeight::SEMIBOLD).child(title.to_string()))
        .child(div().flex_1())
}

/// A fixed-width left column and a flexible right one, separated by a border.
pub fn split(left_width: f32, left: impl IntoElement, right: impl IntoElement) -> Div {
    div().flex().flex_row().flex_1().min_h_0().min_w_0()
        .child(div().w(px(left_width)).flex_none().flex().flex_col().min_h_0().border_r_1().border_color(theme::border()).child(left))
        .child(div().flex().flex_col().flex_1().min_w_0().min_h_0().child(right))
}

/// A clickable list row: 30 px high, radius 7, hover / selected tints (`.srow`).
pub fn row(selected: bool) -> Div {
    div().h(px(30.)).flex_none().flex().items_center().gap(px(8.)).px(px(9.)).rounded(px(7.)).text_color(rgb(fg()))
        .when(selected, |d| d.bg(theme::selected())).when(!selected, |d| d.hover(|d| d.bg(theme::hover())))
}

pub enum Kind {
    Primary,
    Plain,
    Danger,
}

pub fn button(label: &str, kind: Kind) -> Div {
    let b = div().h(px(28.)).px(px(12.)).flex_none().flex().items_center().gap(px(6.)).rounded(px(7.))
        .text_size(px(12.5)).font_weight(FontWeight::MEDIUM);
    match kind {
        Kind::Primary => b.bg(rgb(accent())).text_color(rgb(on_accent())),
        Kind::Danger => b.bg(rgb(error())).text_color(rgb(on_accent())),
        Kind::Plain => b.border_1().border_color(theme::border2()).text_color(rgb(fg())).hover(|d| d.bg(theme::hover())),
    }
    .child(label.to_string())
}

/// One tab of a tab strip (`.seg`): the caller adds the click handler.
pub fn tab(label: &str, on: bool) -> Div {
    div().h(px(26.)).px(px(10.)).flex().items_center().rounded(px(6.)).text_size(px(12.))
        .text_color(rgb(if on { fg() } else { muted() }))
        .when(on, |d| d.bg(theme::selected())).when(!on, |d| d.hover(|d| d.bg(theme::hover())))
        .child(label.to_string())
}

pub fn toggle(on: bool) -> Div {
    div().w(px(30.)).h(px(17.)).flex_none().rounded_full().p(px(2.)).flex()
        .when(on, |d| d.bg(rgb(accent())).justify_end()).when(!on, |d| d.bg(theme::ink(0.16)))
        .child(div().size(px(13.)).rounded_full().bg(rgb(if on { on_accent() } else { fg() })))
}

/// Centered grey text for "nothing here", "loading…" and errors (red).
pub fn note(text: impl Into<String>, error: bool) -> Div {
    div().flex_1().flex().items_center().justify_center().p(px(20.)).text_size(px(12.5))
        .text_color(rgb(if error { crate::theme::error() } else { dim() })).child(text.into())
}

/// A small grey label above a group of rows.
pub fn label(text: &str) -> Div {
    div().h(px(24.)).flex_none().flex().items_center().px(px(9.)).text_size(px(11.)).text_color(rgb(dim())).child(text.to_string())
}
