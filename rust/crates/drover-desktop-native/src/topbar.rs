//! Tab bar (`.tabs`, `.tab`, `.pb` in workspace.css) and status bar (`.status`).

use gpui::{Context, Div, FontWeight, MouseButton, div, prelude::*, px, rgb};

use crate::theme::{self, accent, chrome, dim, done, error, fg, muted, raised, UI_FONT, tint};
use crate::widgets::{icon, key_cap, kind_icon, short_path, state_mark};
use crate::views::View;
use crate::workspace_app::DroverApp;

fn pb(view: View, on: bool) -> Div {
    div().h(px(26.)).flex().items_center().gap(px(5.)).px(px(8.)).rounded(px(6.)).text_size(px(12.))
        .text_color(rgb(if on { fg() } else { muted() })).when(on, |d| d.bg(theme::selected()))
        .hover(|d| d.bg(theme::hover()).text_color(rgb(fg()))).child(icon(view.icon(), 14., if on { fg() } else { muted() }))
        .child(view.title())
}

pub fn tab_bar(app: &DroverApp, cx: &mut Context<DroverApp>) -> Div {
    let mut bar = div().h(px(42.)).flex_none().flex().items_center().gap(px(3.)).pl(px(10.)).pr(px(8.))
        .bg(rgb(chrome())).border_b_1().border_color(theme::border()).font_family(UI_FONT);
    if let Some(space) = app.space.as_deref() {
        for t in app.store.tabs_of(space).filter(|t| !app.settled.contains(&t.id)) {
            let on = app.view == View::Terminals && app.tab.as_deref() == Some(t.id.as_str());
            let (id, close_id) = (t.id.clone(), t.id.clone());
            // The tab shows its most urgent agent, else its first pane.
            let lead = app.store.panes.iter().filter(|p| p.tab_id == t.id)
                .max_by_key(|p| (p.kind != "shell", drover_proto::store::urgency(&p.state)));
            let (kind, state, exit) = lead.map(|p| (p.kind.clone(), p.state.clone(), p.exit_code)).unwrap_or_default();
            bar = bar.child(
                div().h(px(29.)).flex_none().flex().items_center().gap(px(7.)).px(px(10.)).rounded(px(7.)).border_1()
                    .text_size(px(12.8)).text_color(rgb(if on { fg() } else { muted() }))
                    .when(on, |d| d.bg(rgb(raised())).border_color(theme::border()))
                    .when(!on, |d| d.border_color(gpui::transparent_black()).hover(|d| d.bg(theme::hover())))
                    .child(kind_icon(&kind))
                    .child(t.name.clone())
                    .child(state_mark(&state, exit, gpui::SharedString::from(format!("tab-{}", t.id))))
                    .when(on, |d| d.child(div().rounded(px(4.)).hover(|d| d.bg(theme::hover())).child(icon("close", 12., dim()))
                        .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| { cx.stop_propagation(); app.ask_close_tab(&close_id, cx); }))))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| app.select_tab(id.clone(), cx))),
            );
        }
        let away = app.store.tabs_of(space).filter(|t| app.settled.contains(&t.id)).count();
        bar = bar.child(div().size(px(26.)).flex().items_center().justify_center().rounded(px(6.))
            .hover(|d| d.bg(theme::hover())).child(icon("plus", 14., dim()))
            .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.new_tab(cx))))
            .when(away > 0, |d| d.child(div().h(px(22.)).px(px(8.)).rounded(px(6.)).flex().items_center().text_size(px(11.5)).text_color(rgb(dim()))
                .hover(|d| d.bg(theme::hover()).text_color(rgb(fg()))).child(format!("{away} put away"))
                .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.unsettle_all(cx)))));
    }
    let toggle = |view: View| cx.listener(move |app: &mut DroverApp, _: &gpui::MouseDownEvent, _: &mut gpui::Window, cx: &mut Context<DroverApp>| app.toggle_panel(view, cx));
    let open = |view: View| app.view == View::Terminals && app.panel == Some(view);
    bar.child(div().flex_1())
        .child(div().flex().items_center().gap(px(2.))
            .child(div().size(px(26.)).rounded(px(6.)).flex().items_center().justify_center().hover(|d| d.bg(theme::hover()))
                .child(icon("sidebar", 15., dim()))
                .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| { app.sidebar_open = !app.sidebar_open; cx.notify(); })))
            .child(pb(View::Agents, open(View::Agents)).on_mouse_down(MouseButton::Left, toggle(View::Agents)))
            .child(pb(View::Files, open(View::Files)).on_mouse_down(MouseButton::Left, toggle(View::Files)))
            .child(pb(View::Changes, open(View::Changes)).on_mouse_down(MouseButton::Left, toggle(View::Changes))))
}

pub fn status_bar(app: &DroverApp, cx: &mut Context<DroverApp>) -> Div {
    let blocked = app.store.panes.iter().filter(|p| p.state == "blocked").count();
    let space = app.space.as_deref().and_then(|s| app.store.spaces.iter().find(|x| x.id == s));
    let pane = app.focused.as_deref().and_then(|p| app.store.pane(p));
    let mut bar = div().h(px(28.)).flex_none().flex().items_center().gap(px(16.)).px(px(14.)).bg(rgb(chrome()))
        .border_t_1().border_color(theme::border()).text_size(px(11.8)).text_color(rgb(muted())).font_family(UI_FONT)
        .child(div().flex().items_center().gap(px(6.))
            .child(div().size(px(6.)).rounded_full().bg(rgb(if app.connected { done() } else { error() })))
            .child(app.status.clone()))
        .child(format!("This computer · {} spaces · {} agents", app.store.spaces.len(), app.store.panes.len()))
        .when(blocked > 0, |d| d.child(div().text_color(rgb(crate::theme::blocked())).child(format!("{blocked} need you"))));
    if let Some(branch) = space.and_then(|s| s.branch.clone()) {
        let dirty = space.and_then(|s| s.dirty).unwrap_or(0);
        bar = bar.child(div().h(px(20.)).px(px(7.)).rounded(px(5.)).flex().items_center().gap(px(5.)).max_w(px(240.))
            .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| { app.panel = Some(View::Changes); app.view = View::Terminals; cx.notify(); }))
            .bg(tint(accent(), 0.14)).text_color(rgb(accent())).text_size(px(11.)).font_weight(FontWeight::SEMIBOLD)
            .child(icon("branch", 12., accent())).child(div().truncate().child(branch))
            .when(dirty > 0, |d| d.child(div().text_color(rgb(crate::theme::blocked())).child(format!("●{dirty}")))));
    }
    if let Some(p) = pane {
        bar = bar.child(div().min_w_0().truncate().child(short_path(&p.cwd)));
    }
    bar.child(div().flex_1())
        .when(app.chord, |d| d.child(div().text_color(rgb(accent())).child("Ctrl+K… waiting for the second key")))
        .child(div().flex().items_center().gap(px(5.)).child(key_cap("Ctrl+K")).child(key_cap("Ctrl+S"))
            .child(div().text_color(rgb(dim())).child("shortcuts"))
            .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| { app.show_shortcuts = true; cx.notify(); })))
}
