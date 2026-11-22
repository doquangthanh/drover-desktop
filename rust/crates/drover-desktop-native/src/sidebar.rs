//! The left sidebar, as `shell/sidebar*.tsx` + `styles/sidebar.css`: brand, spaces, agents, footer.

use std::sync::Arc;

use gpui::{Context, Div, FontWeight, Image, ImageFormat, MouseButton, div, img, prelude::*, rgb};
use crate::ui_scale::px;

use crate::theme::{self, chrome, dim, done, fg, muted, UI_FONT};
use crate::ui_config::{AgentCol, SpaceCol};
use crate::widgets::{age, avatar, icon, kind_icon, state_mark};
use crate::scrollbar::with_scrollbar;
use crate::sidebar_fold::{AUTO_FOLD_AGENTS, AUTO_FOLD_SPACES};
use crate::views::View;
use drover_proto::wire::Space;
use crate::workspace_app::DroverApp;

fn ibtn(name: &str) -> Div {
    div().size(px(26.)).rounded(px(6.)).flex().items_center().justify_center().hover(|d| d.bg(theme::hover()))
        .child(icon(name, 15., dim()))
}

fn shead(title: &str) -> Div {
    div().h(px(31.)).flex_none().flex().items_center().justify_between().pl(px(16.)).pr(px(10.))
        .text_size(px(11.5)).text_color(rgb(dim())).child(title.to_string())
}

/// One clickable row (`.srow`): 30 px high, radius 8, hover/selected tints.
fn srow(on: bool) -> Div {
    div().h(px(30.)).flex().items_center().gap(px(9.)).px(px(8.)).rounded(px(8.)).text_color(rgb(fg()))
        .when(on, |d| d.bg(theme::selected())).when(!on, |d| d.hover(|d| d.bg(theme::hover())))
}

/// ▾ open / ▸ folded, with how many rows a folded group hides.
fn fold_mark(folded: bool, count: usize) -> Div {
    div().flex_none().flex().items_center().gap(px(4.)).text_size(px(11.)).text_color(rgb(dim()))
        .when(folded, |d| d.child(count.to_string())).child(if folded { "▸" } else { "▾" })
}

/// The heading over one machine's spaces: icon, name, an optional button, a connected / not dot and the fold mark.
fn group_head(icon_name: &str, label: String, ok: bool, extra: Option<Div>, folded: bool, count: usize) -> Div {
    div().h(px(26.)).flex().items_center().gap(px(7.)).px(px(8.)).rounded(px(6.)).text_size(px(11.)).text_color(rgb(dim()))
        .hover(|d| d.bg(theme::hover()))
        .child(icon(icon_name, 13., dim())).child(div().flex_1().min_w_0().truncate().child(label))
        .children(extra)
        .child(div().size(px(6.)).flex_none().rounded_full().bg(rgb(if ok { done() } else { theme::error() })))
        .child(fold_mark(folded, count))
}

/// One machine's spaces under its heading; a click on the heading folds / opens them (sidebar_fold.rs).
fn space_group(app: &DroverApp, key: String, head: (&str, String, bool), extra: Option<Div>, list: Vec<&Space>, cx: &mut Context<DroverApp>) -> Div {
    let keep_open = list.iter().any(|s| app.space.as_deref() == Some(s.id.as_str()) || app.store.space_state(&s.id) == "blocked");
    let folded = app.is_folded(&key, app.store.spaces.len(), AUTO_FOLD_SPACES, keep_open);
    let head = group_head(head.0, head.1, head.2, extra, folded, list.len())
        .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| app.toggle_fold(key.clone(), folded, cx)));
    // A worktree space sits under its repository's space, indented with a branch mark (space_tree.rs).
    div().flex().flex_col().child(head).when(!folded, |d| d.children(crate::space_tree::ordered(&list).into_iter().map(|(s, nested)| space_row(app, s, nested, cx))))
}

/// One space row: the columns the user asked for (ui_config.rs) in their order; click selects, double click renames,
/// middle click / × closes, right click opens the menu.
fn space_row(app: &DroverApp, s: &drover_proto::wire::Space, nested: bool, cx: &mut Context<DroverApp>) -> Div {
    let on = app.space.as_deref() == Some(s.id.as_str());
    let (id, close_id, middle_id, menu_id) = (s.id.clone(), s.id.clone(), s.id.clone(), s.id.clone());
    let state = app.store.space_state(&s.id).to_string();
    let small = || div().flex_none().text_size(px(11.5)).text_color(rgb(dim()));
    // Shown on the selected row only, so other rows keep their width for the name (middle click closes any).
    let close = div().size(px(18.)).flex_none().rounded(px(5.)).flex().items_center().justify_center()
        .hover(|d| d.bg(theme::hover()))
        .child(icon("close", 11., dim()))
        .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| { cx.stop_propagation(); app.ask_close_space(&close_id, cx); }));
    let mut row = srow(on).when(nested, |d| d.pl(px(26.)));
    for col in &app.ui.space_row {
        row = match col {
            SpaceCol::Avatar if nested => row.child(icon("branch", 13., dim())),
            SpaceCol::Avatar => row.child(avatar(&s.name, s.color)),
            SpaceCol::Name => row.child(div().flex_1().min_w_0().truncate().font_weight(FontWeight::MEDIUM).child(s.name.clone())),
            SpaceCol::Branch => row.children(s.branch.as_ref().map(|b| small().max_w(px(130.)).truncate().child(b.clone()))),
            SpaceCol::Dirty => row.when(s.dirty.unwrap_or(0) > 0, |d| d.child(div().size(px(5.)).flex_none().rounded_full().bg(rgb(theme::yellow())))),
            SpaceCol::State => row.when(matches!(state.as_str(), "working" | "blocked"),
                |d| d.child(small().child(state_mark(&state, None, gpui::SharedString::from(format!("sp-{}", s.id)))))),
        };
    }
    row.when(on, |d| d.child(close))
        .on_mouse_down(MouseButton::Left, cx.listener(move |app, ev: &gpui::MouseDownEvent, window, cx| {
            if ev.click_count >= 2 { app.open_rename_space(Some(id.clone()), window, cx); } else { app.select_space(id.clone(), cx); }
        }))
        .on_mouse_down(MouseButton::Middle, cx.listener(move |app, _, _, cx| app.ask_close_space(&middle_id, cx)))
        .on_mouse_down(MouseButton::Right, cx.listener(move |app, ev: &gpui::MouseDownEvent, _, cx| {
            app.open_menu(ev.position, crate::context_menu::space_items(&menu_id), cx);
        }))
}

pub fn sidebar(app: &DroverApp, cx: &mut Context<DroverApp>) -> Div {
    let logo = Arc::new(Image::from_bytes(ImageFormat::Png, crate::assets::LOGO.to_vec()));
    let brand = div().h(px(46.)).flex_none().flex().items_center().gap(px(8.)).pl(px(14.)).pr(px(10.))
        .child(img(logo).size(px(22.)).flex_none())
        // The brand row also moves the window when the system title bar is hidden (topbar.rs).
        .child(div().flex_1().h_full().flex().items_center().font_weight(FontWeight::SEMIBOLD).text_size(px(13.5)).child("Drover")
            .when(crate::topbar::CUSTOM_TITLEBAR, |d| d.window_control_area(gpui::WindowControlArea::Drag)))
        .child(ibtn("search").on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, _| app.open_web(""))))
        .child(ibtn("sidebar").on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| { app.sidebar_open = false; cx.notify(); })));

    // This computer first, then every machine with the spaces its snapshot carries (as `shell/sidebar.tsx`).
    let plus = div().on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| { cx.stop_propagation(); app.new_space(cx); }))
        .child(icon("plus", 13., dim()));
    let local: Vec<&Space> = app.store.spaces.iter().filter(|s| s.machine.as_deref().is_none_or(|m| m.is_empty() || m == "local")).collect();
    let mut spaces = div().id("sidebar-spaces").flex_1().min_h_0().overflow_y_scroll().track_scroll(&app.sidebar_scroll[0]).flex().flex_col().px(px(8.)).pb(px(10.))
        .child(space_group(app, "m:local".into(), ("monitor", "This computer".into(), app.connected), Some(plus), local, cx));
    for m in app.store.machines.as_array().into_iter().flatten() {
        let (alias, status) = (m["alias"].as_str().unwrap_or("?"), m["status"].as_str().unwrap_or(""));
        let label = if status == "connected" { alias.to_string() } else { format!("{alias} · {status}") };
        let list: Vec<&Space> = app.store.spaces.iter().filter(|s| s.machine.as_deref() == Some(alias)).collect();
        // + opens a new space on that machine; only offered while it is connected.
        let on = alias.to_string();
        let plus = (status == "connected").then(|| div()
            .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| { cx.stop_propagation(); app.new_space_on(Some(&on)); }))
            .child(icon("plus", 13., dim())));
        spaces = spaces.child(space_group(app, format!("m:{alias}"), ("server", label, status == "connected"), plus, list, cx));
    }

    let mut agents = div().id("sidebar-agents").flex_1().min_h_0().overflow_y_scroll().track_scroll(&app.sidebar_scroll[1]).flex().flex_col().px(px(8.)).pb(px(10.));
    let total = app.store.panes.iter().filter(|p| p.kind != "shell").count();
    for s in &app.store.spaces {
        let list: Vec<_> = app.store.panes_of_space(&s.id).filter(|p| p.kind != "shell").collect();
        if list.is_empty() {
            continue;
        }
        // Folds like a machine's spaces; kept open while it holds the focused agent or one that needs you.
        let (key, state) = (format!("s:{}", s.id), app.store.space_state(&s.id).to_string());
        let keep_open = state == "blocked" || list.iter().any(|p| app.focused.as_deref() == Some(p.id.as_str()));
        let folded = app.is_folded(&key, total, AUTO_FOLD_AGENTS, keep_open);
        agents = agents.child(div().h(px(24.)).flex().items_center().gap(px(6.)).px(px(8.)).rounded(px(6.)).text_size(px(11.)).text_color(rgb(dim()))
            .hover(|d| d.bg(theme::hover()))
            .child(div().flex_1().min_w_0().truncate().child(s.name.clone()))
            .when(folded && matches!(state.as_str(), "working" | "blocked"),
                |d| d.child(state_mark(&state, None, gpui::SharedString::from(format!("agf-{}", s.id)))))
            .child(fold_mark(folded, list.len()))
            .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| app.toggle_fold(key.clone(), folded, cx))));
        if folded {
            continue;
        }
        for p in list {
            let on = app.focused.as_deref() == Some(p.id.as_str());
            let (pane_id, menu_id) = (p.id.clone(), p.id.clone());
            let mut row = srow(on);
            for col in &app.ui.agent_row {
                row = match col {
                    AgentCol::Icon => row.child(kind_icon(&p.kind)),
                    AgentCol::Name => row.child(div().flex_1().min_w_0().truncate().when(p.state == "idle", |d| d.text_color(rgb(muted()))).child(p.name.clone())),
                    AgentCol::Age => row.child(div().flex_none().text_size(px(11.)).text_color(rgb(dim())).child(age(p.since))),
                    AgentCol::State => row.child(state_mark(&p.state, p.exit_code, gpui::SharedString::from(format!("ag-{}", p.id)))),
                };
            }
            agents = agents.child(row
                .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, window, cx| {
                    app.show_pane(&pane_id);
                    window.focus(&app.term_focus);
                    cx.notify();
                }))
                .on_mouse_down(MouseButton::Right, cx.listener(move |app, ev: &gpui::MouseDownEvent, _, cx| {
                    app.open_menu(ev.position, crate::context_menu::pane_items(&menu_id, false), cx);
                })));
        }
    }

    let blocked = app.store.panes.iter().filter(|p| p.state == "blocked").count();
    let foot_row = |view: View| {
        let on = app.view == view;
        div().h(px(28.)).flex().items_center().gap(px(9.)).px(px(8.)).rounded(px(8.)).text_color(rgb(if on { fg() } else { muted() }))
            .when(on, |d| d.bg(theme::selected())).hover(|d| d.bg(theme::hover()))
            .child(icon(view.icon(), 15., if on { fg() } else { muted() })).child(div().flex_1().child(view.title()))
    };
    let show = |view: View| cx.listener(move |app: &mut DroverApp, _: &gpui::MouseDownEvent, window: &mut gpui::Window, cx: &mut Context<DroverApp>| app.set_view(view, window, cx));
    let foot = div().flex_none().border_t_1().border_color(theme::border()).pt(px(4.)).px(px(8.)).pb(px(8.))
        .child(foot_row(View::Inbox).when(blocked > 0, |d| d.child(div().text_size(px(11.)).text_color(rgb(crate::theme::blocked())).child(blocked.to_string())))
            .on_mouse_down(MouseButton::Left, show(View::Inbox)))
        .child(foot_row(View::Machines).on_mouse_down(MouseButton::Left, show(View::Machines)))
        .child(foot_row(View::Settings).on_mouse_down(MouseButton::Left, show(View::Settings)));

    let half = || div().flex_1().min_h_0().flex().flex_col().overflow_hidden();
    // The right edge resizes the sidebar (drag.rs).
    let w0 = app.sidebar_width;
    let edge = div().absolute().top_0().right(px(-3.)).h_full().w(px(6.)).cursor_col_resize()
        .on_mouse_down(MouseButton::Left, cx.listener(move |app, ev: &gpui::MouseDownEvent, _, cx| {
            app.start_drag(crate::drag::Drag::Sidebar { x0: f32::from(ev.position.x), w0 }, cx);
        }));
    div().relative().w(px(app.sidebar_width)).flex_none().flex().flex_col().bg(rgb(chrome())).border_r_1().border_color(theme::border())
        .font_family(UI_FONT).text_size(px(13.)).child(edge)
        .child(brand)
        .child(half().child(shead("Spaces").child(div().on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.new_space(cx)))
            .child(icon("plus", 14., dim())))).child(with_scrollbar(spaces, &app.sidebar_scroll[0], 0, cx)))
        .child(half().border_t_1().border_color(theme::border()).child(shead("Agents").child(div().rounded(px(5.)).hover(|d| d.bg(theme::hover()))
            .child(icon("layers", 14., dim()))
            .on_mouse_down(MouseButton::Left, cx.listener(|app, _, window, cx| app.set_view(View::Agents, window, cx))))).child(with_scrollbar(agents, &app.sidebar_scroll[1], 1, cx)))
        .child(foot)
}
