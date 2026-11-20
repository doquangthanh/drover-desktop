//! The left sidebar, as `shell/sidebar*.tsx` + `styles/sidebar.css`: brand, spaces, agents, footer.

use std::sync::Arc;

use gpui::{Context, Div, FontWeight, Image, ImageFormat, MouseButton, div, img, prelude::*, px, rgb};

use crate::theme::{self, BLOCKED, CHROME, DIM, DONE, FG, MUTED, UI_FONT};
use crate::widgets::{age, avatar, icon, kind_icon, state_mark};
use crate::views::View;
use crate::workspace_app::DroverApp;

fn ibtn(name: &str) -> Div {
    div().size(px(26.)).rounded(px(6.)).flex().items_center().justify_center().hover(|d| d.bg(theme::hover()))
        .child(icon(name, 15., DIM))
}

fn shead(title: &str) -> Div {
    div().h(px(31.)).flex_none().flex().items_center().justify_between().pl(px(16.)).pr(px(10.))
        .text_size(px(11.5)).text_color(rgb(DIM)).child(title.to_string())
}

/// One clickable row (`.srow`): 30 px high, radius 8, hover/selected tints.
fn srow(on: bool) -> Div {
    div().h(px(30.)).flex().items_center().gap(px(9.)).px(px(8.)).rounded(px(8.)).text_color(rgb(FG))
        .when(on, |d| d.bg(theme::selected())).when(!on, |d| d.hover(|d| d.bg(theme::hover())))
}

pub fn sidebar(app: &DroverApp, cx: &mut Context<DroverApp>) -> Div {
    let logo = Arc::new(Image::from_bytes(ImageFormat::Png, crate::assets::LOGO.to_vec()));
    let brand = div().h(px(46.)).flex_none().flex().items_center().gap(px(8.)).pl(px(14.)).pr(px(10.))
        .child(img(logo).size(px(22.)).flex_none())
        .child(div().flex_1().font_weight(FontWeight::SEMIBOLD).text_size(px(13.5)).child("Drover"))
        .child(ibtn("search").on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, _| app.open_web(""))))
        .child(ibtn("sidebar").on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| { app.sidebar_open = false; cx.notify(); })));

    let mut spaces = div().flex().flex_col().px(px(8.)).pb(px(10.));
    spaces = spaces.child(div().h(px(26.)).flex().items_center().gap(px(7.)).px(px(8.)).text_size(px(11.)).text_color(rgb(DIM))
        .child(icon("monitor", 13., DIM)).child(div().flex_1().child("This computer"))
        .child(div().on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.new_space(cx))).child(icon("plus", 13., DIM)))
        .child(div().size(px(6.)).rounded_full().bg(rgb(if app.connected { DONE } else { theme::ERROR })))
    );
    for s in &app.store.spaces {
        let on = app.space.as_deref() == Some(s.id.as_str());
        let (id, close_id) = (s.id.clone(), s.id.clone());
        let state = app.store.space_state(&s.id).to_string();
        let mut git = div().flex_none().ml_auto().flex().items_center().gap(px(5.)).text_size(px(11.5)).text_color(rgb(DIM));
        if let Some(b) = s.branch.as_ref() {
            git = git.child(div().max_w(px(130.)).truncate().child(b.clone()));
        }
        if s.dirty.unwrap_or(0) > 0 {
            git = git.child(div().size(px(5.)).rounded_full().bg(rgb(theme::YELLOW)));
        }
        if matches!(state.as_str(), "working" | "blocked") {
            git = git.child(state_mark(&state, None, gpui::SharedString::from(format!("sp-{}", s.id))));
        }
        // Shown on the selected row only, so other rows keep their width for the name (middle click closes any).
        let close = div().size(px(18.)).flex_none().rounded(px(5.)).flex().items_center().justify_center()
            .hover(|d| d.bg(theme::hover()))
            .child(icon("close", 11., DIM))
            .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| { cx.stop_propagation(); app.ask_close_space(&close_id, cx); }));
        let middle_id = s.id.clone();
        spaces = spaces.child(srow(on).child(avatar(&s.name, s.color))
            .child(div().flex_1().min_w_0().truncate().font_weight(FontWeight::MEDIUM).child(s.name.clone()))
            .child(git).when(on, |d| d.child(close))
            .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| app.select_space(id.clone(), cx)))
            .on_mouse_down(MouseButton::Middle, cx.listener(move |app, _, _, cx| app.ask_close_space(&middle_id, cx))));
    }

    let mut agents = div().flex().flex_col().px(px(8.)).pb(px(10.));
    for s in &app.store.spaces {
        let list: Vec<_> = app.store.panes_of_space(&s.id).filter(|p| p.kind != "shell").collect();
        if list.is_empty() {
            continue;
        }
        agents = agents.child(div().h(px(24.)).flex().items_center().px(px(8.)).text_size(px(11.)).text_color(rgb(DIM)).child(s.name.clone()));
        for p in list {
            let on = app.focused.as_deref() == Some(p.id.as_str());
            let pane_id = p.id.clone();
            agents = agents.child(srow(on).child(kind_icon(&p.kind))
                .child(div().flex_1().min_w_0().truncate().when(p.state == "idle", |d| d.text_color(rgb(MUTED))).child(p.name.clone()))
                .child(div().flex_none().text_size(px(11.)).text_color(rgb(DIM)).child(age(p.since)))
                .child(state_mark(&p.state, p.exit_code, gpui::SharedString::from(format!("ag-{}", p.id))))
                .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, window, cx| {
                    app.show_pane(&pane_id);
                    window.focus(&app.term_focus);
                    cx.notify();
                })));
        }
    }

    let blocked = app.store.panes.iter().filter(|p| p.state == "blocked").count();
    let foot_row = |view: View| {
        let on = app.view == view;
        div().h(px(28.)).flex().items_center().gap(px(9.)).px(px(8.)).rounded(px(8.)).text_color(rgb(if on { FG } else { MUTED }))
            .when(on, |d| d.bg(theme::selected())).hover(|d| d.bg(theme::hover()))
            .child(icon(view.icon(), 15., if on { FG } else { MUTED })).child(div().flex_1().child(view.title()))
    };
    let show = |view: View| cx.listener(move |app: &mut DroverApp, _: &gpui::MouseDownEvent, window: &mut gpui::Window, cx: &mut Context<DroverApp>| app.set_view(view, window, cx));
    let foot = div().flex_none().border_t_1().border_color(theme::border()).pt(px(4.)).px(px(8.)).pb(px(8.))
        .child(foot_row(View::Inbox).when(blocked > 0, |d| d.child(div().text_size(px(11.)).text_color(rgb(BLOCKED)).child(blocked.to_string())))
            .on_mouse_down(MouseButton::Left, show(View::Inbox)))
        .child(foot_row(View::Machines).on_mouse_down(MouseButton::Left, show(View::Machines)))
        .child(foot_row(View::Settings).on_mouse_down(MouseButton::Left, show(View::Settings)));

    let half = || div().flex_1().min_h_0().flex().flex_col().overflow_hidden();
    div().w(px(260.)).flex_none().flex().flex_col().bg(rgb(CHROME)).border_r_1().border_color(theme::border())
        .font_family(UI_FONT).text_size(px(13.))
        .child(brand)
        .child(half().child(shead("Spaces").child(div().on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.new_space(cx)))
            .child(icon("plus", 14., DIM)))).child(spaces))
        .child(half().border_t_1().border_color(theme::border()).child(shead("Agents").child(icon("layers", 14., DIM))).child(agents))
        .child(foot)
}
