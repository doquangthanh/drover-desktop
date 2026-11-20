//! The confirm dialog (`.modal` in overlays.css): a dimmed backdrop, a card, Cancel and the action button.

use gpui::{Context, Div, FontWeight, MouseButton, div, prelude::*, rgb, rgba};
use crate::ui_scale::px;

use crate::theme::{self, error, fg, muted, UI_FONT};
use crate::workspace_app::DroverApp;

fn button(label: &str, danger: bool) -> Div {
    div().h(px(30.)).px(px(14.)).rounded(px(7.)).flex().items_center().text_size(px(13.)).font_weight(FontWeight::MEDIUM)
        .when(danger, |d| d.bg(rgb(error())).text_color(rgb(theme::on_accent())))
        .when(!danger, |d| d.border_1().border_color(theme::border2()).text_color(rgb(fg())).hover(|d| d.bg(theme::hover())))
        .child(label.to_string())
}

/// The keyboard shortcuts sheet (Ctrl+K Ctrl+S, or the status bar's hint): click outside or Escape closes it.
pub fn shortcuts_overlay(app: &DroverApp, cx: &mut Context<DroverApp>) -> Option<Div> {
    if !app.show_shortcuts {
        return None;
    }
    let mut list = div().flex().flex_col().gap(px(2.));
    for (keys, what) in crate::keys::SHORTCUTS {
        list = list.child(div().flex().items_center().gap(px(12.)).py(px(4.))
            .child(div().w(px(230.)).flex_none().text_size(px(12.)).text_color(rgb(theme::accent())).child(keys))
            .child(div().text_size(px(13.)).text_color(rgb(fg())).child(what)));
    }
    let card = div().w(px(560.)).p(px(22.)).flex().flex_col().gap(px(12.)).rounded(px(12.)).bg(rgb(crate::theme::card()))
        .border_1().border_color(theme::border2()).shadow_lg().font_family(UI_FONT)
        .child(div().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).text_color(rgb(fg())).child("Keyboard shortcuts"))
        .child(div().text_size(px(12.)).text_color(rgb(muted())).child("VS Code's keys. Ctrl+K W: press Ctrl+K, let go, then W. They work inside terminals too."))
        .child(list)
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
    Some(div().absolute().top_0().left_0().size_full().flex().items_center().justify_center().bg(rgba(0x00000073))
        .occlude().child(card)
        .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| { app.show_shortcuts = false; cx.notify(); })))
}

pub fn confirm_overlay(app: &DroverApp, cx: &mut Context<DroverApp>) -> Option<Div> {
    let c = app.confirm.as_ref()?;
    let card = div().w(px(420.)).p(px(22.)).flex().flex_col().gap(px(10.)).rounded(px(12.)).bg(rgb(crate::theme::card()))
        .border_1().border_color(theme::border2()).shadow_lg().font_family(UI_FONT)
        .child(div().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).text_color(rgb(fg())).child(c.title.clone()))
        .child(div().text_size(px(13.)).text_color(rgb(muted())).child(c.detail.clone()))
        .child(div().flex().justify_end().gap(px(8.)).pt(px(8.))
            .child(button("Cancel", false)
                .on_mouse_down(MouseButton::Left, cx.listener(|app, _, window, cx| app.answer_confirm(false, window, cx))))
            .child(button(c.ok, true)
                .on_mouse_down(MouseButton::Left, cx.listener(|app, _, window, cx| app.answer_confirm(true, window, cx)))));
    Some(div().absolute().top_0().left_0().size_full().flex().items_center().justify_center().bg(rgba(0x00000073))
        .occlude().child(card))
}
