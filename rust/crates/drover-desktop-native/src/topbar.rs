//! Tab bar (`.tabs`, `.tab`, `.pb` in workspace.css) and status bar (`.status`).

use gpui::{Context, Div, FontWeight, MouseButton, div, prelude::*, px, rgb};

use crate::theme::{self, ACCENT, BLOCKED, CHROME, DIM, DONE, ERROR, FG, MUTED, RAISED, UI_FONT, tint};
use crate::widgets::{icon, key_cap, kind_icon, short_path, state_mark};
use crate::workspace_app::DroverApp;

fn pb(icon_name: &str, label: &str) -> Div {
    div().h(px(26.)).flex().items_center().gap(px(5.)).px(px(8.)).rounded(px(6.)).text_size(px(12.)).text_color(rgb(MUTED))
        .hover(|d| d.bg(theme::hover()).text_color(rgb(FG))).child(icon(icon_name, 14., MUTED)).child(label.to_string())
}

pub fn tab_bar(app: &DroverApp, cx: &mut Context<DroverApp>) -> Div {
    let mut bar = div().h(px(42.)).flex_none().flex().items_center().gap(px(3.)).pl(px(10.)).pr(px(8.))
        .bg(rgb(CHROME)).border_b_1().border_color(theme::border()).font_family(UI_FONT);
    if let Some(space) = app.space.as_deref() {
        for t in app.store.tabs_of(space) {
            let on = app.tab.as_deref() == Some(t.id.as_str());
            let (id, close_id) = (t.id.clone(), t.id.clone());
            // The tab shows its most urgent agent, else its first pane.
            let lead = app.store.panes.iter().filter(|p| p.tab_id == t.id)
                .max_by_key(|p| (p.kind != "shell", drover_proto::store::urgency(&p.state)));
            let (kind, state, exit) = lead.map(|p| (p.kind.clone(), p.state.clone(), p.exit_code)).unwrap_or_default();
            bar = bar.child(
                div().h(px(29.)).flex_none().flex().items_center().gap(px(7.)).px(px(10.)).rounded(px(7.)).border_1()
                    .text_size(px(12.8)).text_color(rgb(if on { FG } else { MUTED }))
                    .when(on, |d| d.bg(rgb(RAISED)).border_color(theme::border()))
                    .when(!on, |d| d.border_color(gpui::transparent_black()).hover(|d| d.bg(theme::hover())))
                    .child(kind_icon(&kind))
                    .child(t.name.clone())
                    .child(state_mark(&state, exit, gpui::SharedString::from(format!("tab-{}", t.id))))
                    .when(on, |d| d.child(div().rounded(px(4.)).hover(|d| d.bg(theme::hover())).child(icon("close", 12., DIM))
                        .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| { cx.stop_propagation(); app.ask_close_tab(&close_id, cx); }))))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| app.select_tab(id.clone(), cx))),
            );
        }
        bar = bar.child(div().size(px(26.)).flex().items_center().justify_center().rounded(px(6.))
            .hover(|d| d.bg(theme::hover())).child(icon("plus", 14., DIM))
            .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.new_tab(cx))));
    }
    let web = |view: &'static str| cx.listener(move |app: &mut DroverApp, _: &gpui::MouseDownEvent, _: &mut gpui::Window, _: &mut Context<DroverApp>| app.open_web(view));
    bar.child(div().flex_1())
        .child(div().flex().items_center().gap(px(2.))
            .child(div().size(px(26.)).rounded(px(6.)).flex().items_center().justify_center().hover(|d| d.bg(theme::hover()))
                .child(icon("sidebar", 15., DIM))
                .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| { app.sidebar_open = !app.sidebar_open; cx.notify(); })))
            .child(pb("layers", "Agents").on_mouse_down(MouseButton::Left, web("tree")))
            .child(pb("folder", "Files").on_mouse_down(MouseButton::Left, web("files")))
            .child(pb("branch", "Changes").on_mouse_down(MouseButton::Left, web("changes"))))
}

pub fn status_bar(app: &DroverApp, cx: &mut Context<DroverApp>) -> Div {
    let blocked = app.store.panes.iter().filter(|p| p.state == "blocked").count();
    let space = app.space.as_deref().and_then(|s| app.store.spaces.iter().find(|x| x.id == s));
    let pane = app.focused.as_deref().and_then(|p| app.store.pane(p));
    let mut bar = div().h(px(28.)).flex_none().flex().items_center().gap(px(16.)).px(px(14.)).bg(rgb(CHROME))
        .border_t_1().border_color(theme::border()).text_size(px(11.8)).text_color(rgb(MUTED)).font_family(UI_FONT)
        .child(div().flex().items_center().gap(px(6.))
            .child(div().size(px(6.)).rounded_full().bg(rgb(if app.connected { DONE } else { ERROR })))
            .child(app.status.clone()))
        .child(format!("This computer · {} spaces · {} agents", app.store.spaces.len(), app.store.panes.len()))
        .when(blocked > 0, |d| d.child(div().text_color(rgb(BLOCKED)).child(format!("{blocked} need you"))));
    if let Some(branch) = space.and_then(|s| s.branch.clone()) {
        let dirty = space.and_then(|s| s.dirty).unwrap_or(0);
        bar = bar.child(div().h(px(20.)).px(px(7.)).rounded(px(5.)).flex().items_center().gap(px(5.)).max_w(px(240.))
            .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, _| app.open_web("changes")))
            .bg(tint(ACCENT, 0.14)).text_color(rgb(ACCENT)).text_size(px(11.)).font_weight(FontWeight::SEMIBOLD)
            .child(icon("branch", 12., ACCENT)).child(div().truncate().child(branch))
            .when(dirty > 0, |d| d.child(div().text_color(rgb(BLOCKED)).child(format!("●{dirty}")))));
    }
    if let Some(p) = pane {
        bar = bar.child(div().min_w_0().truncate().child(short_path(&p.cwd)));
    }
    bar.child(div().flex_1())
        .child(div().flex().items_center().gap(px(5.)).child(key_cap("Ctrl+K")).child(key_cap("Ctrl+S"))
            .child(div().text_color(rgb(DIM)).child("shortcuts")))
}
