//! The confirm dialog (`.modal` in overlays.css): a dimmed backdrop, a card, Cancel and the action button.

use gpui::{Context, Div, FontWeight, MouseButton, div, prelude::*, px, rgb, rgba};

use crate::theme::{self, ERROR, FG, MUTED, UI_FONT};
use crate::workspace_app::DroverApp;

fn button(label: &str, danger: bool) -> Div {
    div().h(px(30.)).px(px(14.)).rounded(px(7.)).flex().items_center().text_size(px(13.)).font_weight(FontWeight::MEDIUM)
        .when(danger, |d| d.bg(rgb(ERROR)).text_color(rgb(theme::ON_ACCENT)))
        .when(!danger, |d| d.border_1().border_color(theme::border2()).text_color(rgb(FG)).hover(|d| d.bg(theme::hover())))
        .child(label.to_string())
}

pub fn confirm_overlay(app: &DroverApp, cx: &mut Context<DroverApp>) -> Option<Div> {
    let c = app.confirm.as_ref()?;
    let card = div().w(px(420.)).p(px(22.)).flex().flex_col().gap(px(10.)).rounded(px(12.)).bg(rgb(0x1d1f23))
        .border_1().border_color(theme::border2()).shadow_lg().font_family(UI_FONT)
        .child(div().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).text_color(rgb(FG)).child(c.title.clone()))
        .child(div().text_size(px(13.)).text_color(rgb(MUTED)).child(c.detail.clone()))
        .child(div().flex().justify_end().gap(px(8.)).pt(px(8.))
            .child(button("Cancel", false)
                .on_mouse_down(MouseButton::Left, cx.listener(|app, _, window, cx| app.answer_confirm(false, window, cx))))
            .child(button(c.ok, true)
                .on_mouse_down(MouseButton::Left, cx.listener(|app, _, window, cx| app.answer_confirm(true, window, cx)))));
    Some(div().absolute().top_0().left_0().size_full().flex().items_center().justify_center().bg(rgba(0x00000073))
        .occlude().child(card))
}
