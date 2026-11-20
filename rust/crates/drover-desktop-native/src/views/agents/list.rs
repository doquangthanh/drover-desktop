//! Agents view, left panel (`agent-tree-list.tsx`): counts per state, a name filter, "you", then who started whom.

use drover_proto::agent_tree::{self, TreeRow};
use drover_proto::wire::Pane;
use gpui::{Context, Div, FontWeight, MouseButton, div, prelude::*, px, rgb};

use crate::theme::{self, BLOCKED, DIM, DONE, FG, MUTED, WORKING};
use crate::views::ui;
use crate::widgets::{age, kind_icon, state_mark};
use crate::workspace_app::DroverApp;

fn count(n: usize, color: u32) -> Div {
    div().flex().items_center().gap(px(4.)).text_color(rgb(color))
        .child(div().size(px(6.)).rounded_full().bg(rgb(color))).child(n.to_string())
}

fn meta(p: &Pane) -> String {
    if p.state == "idle" && p.parent.is_some() {
        return "waits".into();
    }
    if !p.machine.is_empty() && p.machine != "local" {
        return p.machine.clone();
    }
    age(p.created_at)
}

/// Connector column for one depth: a vertical line while that ancestor has siblings below.
fn guide(on: bool) -> Div {
    div().w(px(14.)).h_full().flex_none().flex().justify_center()
        .child(div().w(px(1.)).h_full().when(on, |d| d.bg(theme::border2())))
}

fn row(app: &DroverApp, r: &TreeRow, p: &Pane, on: bool, cx: &mut Context<DroverApp>) -> Div {
    let sub = if !p.title.is_empty() && p.title != p.name { p.title.clone() } else { p.command.clone() };
    let id = p.id.clone();
    let _ = app;
    let mut line = div().h(px(40.)).flex_none().flex().items_center().pr(px(8.)).rounded(px(7.))
        .when(on, |d| d.bg(theme::selected())).when(!on, |d| d.hover(|d| d.bg(theme::hover())))
        .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| app.pick_agent(id.clone(), cx)));
    for g in &r.guides {
        line = line.child(guide(*g));
    }
    line.child(div().w(px(12.)).flex_none().text_color(rgb(DIM)).text_size(px(11.)).child(if r.last { "└" } else { "├" }))
        .child(div().flex_none().mr(px(8.)).child(kind_icon(&p.kind)))
        .child(div().flex_1().min_w_0().flex().flex_col()
            .child(div().truncate().text_color(rgb(FG)).when(p.parent.is_none(), |d| d.font_weight(FontWeight::SEMIBOLD))
                .child(if p.name.is_empty() { p.id.clone() } else { p.name.clone() }))
            .when(!sub.is_empty(), |d| d.child(div().truncate().text_size(px(11.)).text_color(rgb(DIM)).child(sub))))
        .child(div().flex_none().flex().items_center().gap(px(6.)).ml(px(6.))
            .child(state_mark(&p.state, p.exit_code, gpui::SharedString::from(format!("at-{}", p.id))))
            .child(div().text_size(px(11.)).text_color(rgb(DIM)).child(meta(p))))
}

pub fn list(app: &DroverApp, rows: &[TreeRow], agents: &[&Pane], sel: Option<&str>, cx: &mut Context<DroverApp>) -> Div {
    let query = app.agents_filter.read(cx).text.clone();
    let shown = agent_tree::filter_rows(rows, agents, &query);
    let roots = rows.iter().filter(|r| r.guides.is_empty()).count();
    let head = div().flex_none().flex().flex_col().gap(px(10.)).p(px(12.)).border_b_1().border_color(theme::border())
        .child(div().flex().items_center().justify_between().text_size(px(11.))
            .child(div().text_color(rgb(MUTED)).font_weight(FontWeight::SEMIBOLD).child(format!("AGENTS · {}", agents.len())))
            .child(div().flex().gap(px(10.))
                .child(count(agent_tree::state_count(agents, "working"), WORKING))
                .child(count(agent_tree::state_count(agents, "blocked"), BLOCKED))
                .child(count(agent_tree::state_count(agents, "done"), DONE))
                .child(count(agent_tree::state_count(agents, "idle"), DIM))))
        .child(app.agents_filter.clone());
    let you = div().h(px(40.)).flex_none().flex().items_center().gap(px(8.)).px(px(8.))
        .child(div().size(px(22.)).rounded(px(6.)).bg(rgb(theme::RAISED)).flex().items_center().justify_center()
            .text_size(px(9.)).text_color(rgb(MUTED)).child("you"))
        .child(div().flex().flex_col()
            .child(div().font_weight(FontWeight::SEMIBOLD).child("You"))
            .child(div().text_size(px(11.)).text_color(rgb(DIM)).child(format!("started {roots} agent{}", if roots == 1 { "" } else { "s" }))));
    let mut body = div().id("agents-tree").flex_1().min_h_0().overflow_y_scroll().flex().flex_col().p(px(6.)).child(you);
    for r in &shown {
        if let Some(p) = agents.iter().find(|p| p.id == r.id) {
            body = body.child(row(app, r, p, sel == Some(r.id.as_str()), cx));
        }
    }
    if rows.is_empty() {
        body = body.child(ui::note("No agents.", false));
    } else if shown.is_empty() {
        body = body.child(ui::note(format!("No agent matches “{query}”."), false));
    }
    div().flex().flex_col().flex_1().min_h_0().child(head).child(body)
}
