//! The confirm dialog (`.modal` in overlays.css): a dimmed backdrop, a card, Cancel and the action button.

use gpui::{Context, Div, Focusable, FontWeight, KeyDownEvent, MouseButton, Window, div, prelude::*, rgb, rgba};
use crate::ui_scale::px;

use crate::palette_search::fold;
use crate::theme::{self, error, fg, muted, UI_FONT};
use crate::workspace_app::DroverApp;

fn button(label: &str, danger: bool) -> Div {
    div().h(px(30.)).px(px(14.)).rounded(px(7.)).flex().items_center().text_size(px(13.)).font_weight(FontWeight::MEDIUM)
        .when(danger, |d| d.bg(rgb(error())).text_color(rgb(theme::on_accent())))
        .when(!danger, |d| d.border_1().border_color(theme::border2()).text_color(rgb(fg())).hover(|d| d.bg(theme::hover())))
        .child(label.to_string())
}

impl DroverApp {
    /// Open the shortcuts sheet with an empty filter and the cursor in it.
    pub fn open_shortcuts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_shortcuts = true;
        self.shortcuts_filter.update(cx, |i, cx| i.set_text("", cx));
        window.focus(&self.shortcuts_filter.read(cx).focus_handle(cx));
        cx.notify();
    }

    pub fn close_shortcuts(&mut self, cx: &mut Context<Self>) {
        self.show_shortcuts = false;
        self.want_term_focus = true;
        cx.notify();
    }
}

/// The shortcuts that match what was typed in the sheet's filter (name or keys, accents and case ignored).
pub fn matching_shortcuts<'a>(all: &'a [(&'a str, &'a str)], query: &str) -> Vec<&'a (&'a str, &'a str)> {
    let q = fold(query.trim());
    all.iter().filter(|(keys, what)| q.is_empty() || fold(&format!("{what} {keys}")).contains(&q)).collect()
}

/// The keyboard shortcuts sheet (Ctrl+K Ctrl+S, or the status bar's hint): filter box on top; click outside or Escape closes it.
pub fn shortcuts_overlay(app: &DroverApp, cx: &mut Context<DroverApp>) -> Option<Div> {
    if !app.show_shortcuts {
        return None;
    }
    let query = app.shortcuts_filter.read(cx).value();
    let shown = matching_shortcuts(&crate::keys::SHORTCUTS, &query);
    let mut list = div().flex().flex_col().gap(px(2.)).max_h(px(420.)).overflow_hidden();
    for (keys, what) in &shown {
        list = list.child(div().flex().items_center().gap(px(12.)).py(px(4.))
            .child(div().w(px(230.)).flex_none().text_size(px(12.)).text_color(rgb(theme::accent())).child(*keys))
            .child(div().text_size(px(13.)).text_color(rgb(fg())).child(*what)));
    }
    if shown.is_empty() {
        list = list.child(div().py(px(6.)).text_size(px(13.)).text_color(rgb(muted())).child(format!("No shortcut matches \"{}\".", query.trim())));
    }
    let card = div().w(px(560.)).p(px(22.)).flex().flex_col().gap(px(12.)).rounded(px(12.)).bg(rgb(crate::theme::card()))
        .border_1().border_color(theme::border2()).shadow_lg().font_family(UI_FONT)
        .child(div().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).text_color(rgb(fg())).child("Keyboard shortcuts"))
        .child(div().text_size(px(12.)).text_color(rgb(muted())).child("VS Code's keys. Ctrl+K W: press Ctrl+K, let go, then W. They work inside terminals too."))
        .child(app.shortcuts_filter.clone())
        .child(list)
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
    Some(div().absolute().top_0().left_0().size_full().flex().items_center().justify_center().bg(rgba(0x00000073))
        .occlude().child(card)
        .on_key_down(cx.listener(|app, ev: &KeyDownEvent, _, cx| {
            if ev.keystroke.key == "escape" { app.close_shortcuts(cx); cx.stop_propagation(); }
        }))
        .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.close_shortcuts(cx))))
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

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [(&str, &str); 3] = [("Ctrl+P", "Jump to a terminal"), ("Ctrl+Shift+G", "Changes panel"), ("F8", "Next agent that needs you")];

    #[test]
    fn filters_by_name_or_keys_ignoring_case() {
        assert_eq!(matching_shortcuts(&ALL, "").len(), 3);
        assert_eq!(matching_shortcuts(&ALL, "JUMP")[0].0, "Ctrl+P");
        assert_eq!(matching_shortcuts(&ALL, "f8")[0].1, "Next agent that needs you");
        assert_eq!(matching_shortcuts(&ALL, "shift").len(), 1);
        assert!(matching_shortcuts(&ALL, "zzz").is_empty());
    }
}
