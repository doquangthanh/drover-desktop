//! Agents view, the selected agent (`agent-tree-inspector.tsx`): who it is, Open / Send / Stop, its tree's state,
//! and who in the tree waits for an answer.

use drover_proto::wire::Pane;
use gpui::{Context, Div, FontWeight, MouseButton, div, prelude::*, px, rgb};

use crate::theme::{self, blocked, dim, done, fg, muted, working, tint};
use crate::views::ui::{self, Kind};
use crate::widgets::{age, kind_icon};
use crate::workspace_app::DroverApp;

pub fn state_label(p: &Pane) -> String {
    match p.state.as_str() {
        "working" => "Working".into(),
        "blocked" => "Needs you".into(),
        "done" => "Done".into(),
        "idle" => "Idle".into(),
        "exited" => p.exit_code.filter(|c| *c != 0).map(|c| format!("exit {c}")).unwrap_or("Exited".into()),
        other => other.to_string(),
    }
}

pub fn state_color(state: &str) -> u32 {
    match state {
        "working" => working(),
        "blocked" => blocked(),
        "done" => done(),
        _ => dim(),
    }
}

pub fn label(p: Option<&Pane>) -> String {
    p.map(|p| if !p.name.is_empty() { p.name.clone() } else if !p.title.is_empty() { p.title.clone() } else { p.id.clone() })
        .unwrap_or_default()
}

fn tile(name: &str, value: String, color: u32) -> Div {
    div().flex_1().min_w(px(90.)).p(px(10.)).flex().flex_col().gap(px(4.)).rounded(px(8.)).border_1()
        .border_color(theme::border()).bg(theme::ink(0.02))
        .child(div().flex().items_center().gap(px(5.)).text_size(px(10.5)).text_color(rgb(dim()))
            .child(div().size(px(6.)).rounded_full().bg(rgb(color))).child(name.to_uppercase()))
        .child(div().text_size(px(18.)).font_weight(FontWeight::SEMIBOLD).text_color(rgb(fg())).child(value))
}

fn fact(name: &str, value: String) -> Div {
    div().flex().gap(px(4.)).child(div().text_color(rgb(dim())).child(name.to_string())).child(div().text_color(rgb(fg())).child(value))
}

pub fn inspector(app: &DroverApp, pane: &Pane, root: &str, tree: &[String], cx: &mut Context<DroverApp>) -> Div {
    let tree_panes: Vec<&Pane> = tree.iter().filter_map(|id| app.store.pane(id)).collect();
    let n = |s: &str| tree_panes.iter().filter(|p| p.state == s).count();
    let root_pane = app.store.pane(root);
    let parent = pane.parent.as_deref().and_then(|id| app.store.pane(id));
    let machine = if pane.machine.is_empty() || pane.machine == "local" { "This computer".to_string() } else { pane.machine.clone() };
    let (open_id, stop_id) = (pane.id.clone(), pane.id.clone());
    let color = state_color(&pane.state);
    let top = div().flex().flex_wrap().items_start().gap(px(12.))
        .child(div().pt(px(3.)).child(kind_icon(&pane.kind)))
        .child(div().flex_1().min_w_0().flex().flex_col().gap(px(5.))
            .child(div().flex().items_center().gap(px(10.))
                .child(div().truncate().text_size(px(17.)).font_weight(FontWeight::SEMIBOLD).child(label(Some(pane))))
                .child(div().h(px(20.)).px(px(8.)).rounded(px(5.)).flex().items_center().text_size(px(11.))
                    .bg(tint(color, 0.16)).text_color(rgb(color)).child(state_label(pane))))
            .when(!pane.title.is_empty() && pane.title != pane.name, |d| d.child(div().truncate().text_color(rgb(muted())).child(pane.title.clone())))
            .child(div().flex().flex_wrap().gap(px(14.)).text_size(px(12.))
                .child(fact("CLI", if pane.kind.is_empty() { "?".into() } else { pane.kind.clone() }))
                .child(fact("on", machine))
                .child(fact("started by", parent.map(|p| label(Some(p))).unwrap_or("you".into())))
                .child(fact("running", age(pane.created_at)))))
        .child(div().flex().gap(px(8.))
            .child(ui::button("Open", Kind::Primary).on_mouse_down(MouseButton::Left, cx.listener(move |app, _, window, cx| {
                app.show_pane(&open_id);
                window.focus(&app.term_focus);
                cx.notify();
            })))
            .child(ui::button("Stop subtree", Kind::Danger).on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| app.ask_stop_subtree(&stop_id, cx)))));
    let send = div().flex().items_center().gap(px(8.))
        .child(div().flex_1().child(app.agents_send.clone()))
        .child(ui::button("Send", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| {
            let text = app.agents_send.read(cx).text.clone();
            app.send_to_selected_agent(text);
            app.agents_send.update(cx, |i, cx| i.set_text("", cx));
        })));
    let tiles = div().flex().flex_wrap().gap(px(8.))
        .child(tile("Helpers", (tree.len().saturating_sub(1)).to_string(), dim()))
        .child(tile("Working", n("working").to_string(), working()))
        .child(tile("Needs you", n("blocked").to_string(), blocked()))
        .child(tile("Done", n("done").to_string(), done()))
        .child(tile("Running for", root_pane.map(|p| age(p.created_at)).unwrap_or_default(), dim()));
    let mut card = div().flex_none().flex().flex_col().gap(px(12.)).p(px(16.)).border_b_1().border_color(theme::border())
        .child(top).child(send).child(tiles);
    for p in tree_panes.iter().filter(|p| p.state == "blocked") {
        let id = p.id.clone();
        card = card.child(div().flex().items_center().gap(px(10.)).p(px(10.)).rounded(px(8.)).bg(tint(blocked(), 0.1))
            .child(div().size(px(7.)).rounded_full().bg(rgb(blocked())))
            .child(div().flex_1().min_w_0().truncate().child(format!("{} needs an answer", label(Some(p)))))
            .child(ui::button("Answer", Kind::Primary).on_mouse_down(MouseButton::Left, cx.listener(move |app, _, window, cx| {
                app.show_pane(&id);
                window.focus(&app.term_focus);
                cx.notify();
            }))));
    }
    card
}
