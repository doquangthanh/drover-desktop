//! A finished agent: a banner in its pane and a "Review" pill in the sidebar, both one click from its changes.
//! The same as the web UI's `workspace/review-banner.tsx`.

use gpui::{Context, Div, FontWeight, MouseButton, div, prelude::*, rgb};
use crate::ui_scale::px;

use crate::theme::{self, accent, dim, done, fg, on_accent, tint, UI_FONT};
use crate::views::View;
use crate::widgets::key_cap;
use crate::workspace_app::DroverApp;

/// Whether a pane shows the banner: it is done, and this "done" (identified by `since`) was not dismissed.
pub fn banner_visible(state: &str, since: f64, dismissed: Option<i64>) -> bool {
    state == "done" && dismissed != Some(since as i64)
}

impl DroverApp {
    /// Go to the pane and open its changes beside it.
    pub fn review_pane(&mut self, pane: &str, cx: &mut Context<Self>) {
        self.show_pane(pane);
        self.set_panel(Some(View::Changes));
        cx.notify();
    }
}

/// The strip under a finished agent's header, or nothing.
pub fn banner(app: &DroverApp, pane_id: &str, cx: &mut Context<DroverApp>) -> Option<Div> {
    let p = app.store.pane(pane_id)?;
    if !banner_visible(&p.state, p.since, app.banner_dismissed.get(pane_id).copied()) {
        return None;
    }
    let (review_id, dismiss_id, since) = (pane_id.to_string(), pane_id.to_string(), p.since as i64);
    Some(div().min_h(px(36.)).flex_none().flex().items_center().gap(px(8.)).pl(px(12.)).pr(px(6.)).py(px(4.)).font_family(UI_FONT)
        .bg(tint(done(), 0.09)).border_b_1().border_color(theme::border()).text_size(px(12.5)).text_color(rgb(theme::tfg()))
        .child(div().size(px(6.)).rounded_full().flex_none().bg(rgb(done())))
        .child(div().flex_1().min_w_0().truncate().child("Finished. Look over what it changed."))
        .child(div().h(px(26.)).px(px(10.)).rounded(px(7.)).flex().items_center().bg(rgb(accent())).text_color(rgb(on_accent()))
            .font_weight(FontWeight::SEMIBOLD).text_size(px(12.)).child("Review changes")
            .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| { cx.stop_propagation(); app.review_pane(&review_id, cx); })))
        .child(key_cap("Ctrl+Shift+G"))
        .child(div().size(px(22.)).rounded(px(5.)).flex().items_center().justify_center().text_color(rgb(dim())).text_size(px(11.))
            .hover(|d| d.bg(theme::hover()).text_color(rgb(fg()))).child("✕")
            .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| {
                cx.stop_propagation();
                app.banner_dismissed.insert(dismiss_id.clone(), since);
                cx.notify();
            }))))
}

/// The "Review" pill of a finished agent's sidebar row.
pub fn review_pill(pane_id: &str, cx: &mut Context<DroverApp>) -> Div {
    let id = pane_id.to_string();
    crate::widgets::pill("Review".into(), done())
        .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| { cx.stop_propagation(); app.review_pane(&id, cx); }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_only_for_a_done_agent_until_that_done_is_dismissed() {
        assert!(banner_visible("done", 100.4, None));
        assert!(!banner_visible("working", 100.4, None));
        assert!(!banner_visible("done", 100.4, Some(100)), "dismissed");
        assert!(banner_visible("done", 205.0, Some(100)), "a later done shows again");
    }
}
