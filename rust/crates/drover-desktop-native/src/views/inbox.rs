//! Inbox (port of `inbox-view.tsx` + `inbox-card.tsx`): agents sorted by who needs you, quick answers without
//! opening the pane, and policy prompts (an agent tried to answer for another). No notification side panel and no
//! machine filter yet.

use drover_proto::wire::Pane;
use gpui::{Context, Div, FontWeight, MouseButton, div, prelude::*, px, rgb};
use serde_json::{Value, json};

use crate::theme::{self, BLOCKED, DIM, FG, MUTED, WORKING, tint};
use crate::views::ui::{self, Kind};
use crate::widgets::{age, kind_icon, state_mark};
use crate::workspace_app::DroverApp;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Filter {
    #[default]
    All,
    Blocked,
    Done,
    Unread,
}

const GROUPS: [(&str, &[&str]); 4] =
    [("Needs you", &["blocked"]), ("Done", &["done"]), ("Working", &["working"]), ("Idle · Exited", &["idle", "exited"])];

fn matches(p: &Pane, f: Filter) -> bool {
    match f {
        Filter::All => true,
        Filter::Blocked => p.state == "blocked",
        Filter::Done => p.state == "done",
        Filter::Unread => p.unread,
    }
}

fn act(label: &str, kind: Kind, cx: &mut Context<DroverApp>, f: impl Fn(&mut DroverApp) + 'static) -> Div {
    ui::button(label, kind).on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| { f(app); cx.notify(); }))
}

fn open(label: &str, id: String, cx: &mut Context<DroverApp>) -> Div {
    ui::button(label, Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(move |app, _, window, cx| {
        app.show_pane(&id);
        window.focus(&app.term_focus);
        cx.notify();
    }))
}

fn policy_card(app: &DroverApp, p: &Value, cx: &mut Context<DroverApp>) -> Div {
    let id = p["id"].as_str().unwrap_or("").to_string();
    let name = |k: &str| { let id = p[k].as_str().unwrap_or(""); app.store.pane(id).map(|x| x.name.clone()).unwrap_or(id.to_string()) };
    let machine = id.split_once(':').map(|(m, _)| m.to_string());
    let decide = move |allow: bool| {
        let (id, machine) = (id.clone(), machine.clone());
        move |app: &mut DroverApp| {
            let mut params = json!({"id": id, "allow": allow});
            if let Some(m) = &machine { params["machine"] = json!(m); }
            app.rpc("policy.decide", params);
        }
    };
    let target = p["target"].as_str().unwrap_or("").to_string();
    div().flex().flex_col().gap(px(8.)).p(px(12.)).rounded(px(10.)).bg(tint(BLOCKED, 0.1)).border_1().border_color(tint(BLOCKED, 0.3))
        .child(div().font_weight(FontWeight::SEMIBOLD).child(format!("{} tried to answer “{}” for {}.", name("source"),
            p["text"].as_str().unwrap_or(""), name("target"))))
        .child(div().text_size(px(12.)).text_color(rgb(MUTED)).child("Your policy says agents can't approve permission prompts, so it's waiting for you."))
        .child(div().flex().gap(px(8.))
            .child(act("Allow once", Kind::Primary, cx, decide(true)))
            .child(act("Deny", Kind::Plain, cx, decide(false)))
            .child(open("Open", target, cx)))
}

fn card(app: &DroverApp, p: &Pane, cx: &mut Context<DroverApp>) -> Div {
    let space = app.store.spaces.iter().find(|s| s.id == p.space_id).map(|s| s.name.clone()).unwrap_or_default();
    let machine = if p.machine.is_empty() || p.machine == "local" { "This computer".to_string() } else { p.machine.clone() };
    let helpers = app.store.panes.iter().filter(|x| x.parent.as_deref() == Some(p.id.as_str())).count();
    let when = match p.state.as_str() { "blocked" => "waiting ", "done" => "finished ", "exited" => "exited ", _ => "" };
    let since = if p.since > 0. { p.since } else { p.created_at };
    let detail = if helpers > 0 && p.state == "working" { format!("spawned {helpers} helper{}", if helpers > 1 { "s" } else { "" }) }
        else { format!("{when}{}{}", age(since), if !when.is_empty() && p.state != "blocked" { " ago" } else { "" }) };
    let top = div().flex().items_center().gap(px(8.))
        .child(kind_icon(&p.kind))
        .child(div().flex_none().font_weight(FontWeight::SEMIBOLD).text_color(rgb(FG)).child(p.name.clone()))
        .child(div().flex_1().min_w_0().truncate().text_size(px(12.)).text_color(rgb(DIM)).child(format!("{space} · {machine} · {detail}")))
        .child(state_mark(&p.state, p.exit_code, gpui::SharedString::from(format!("ib-{}", p.id))))
        .when(p.unread, |d| d.child(div().size(px(7.)).rounded_full().bg(rgb(WORKING))));
    let id = p.id.clone();
    if !matches!(p.state.as_str(), "blocked" | "done" | "exited") {
        return ui::row(false).h(px(40.)).px(px(12.)).child(div().flex_1().min_w_0().child(top))
            .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, window, cx| { app.show_pane(&id); window.focus(&app.term_focus); cx.notify(); }));
    }
    let write = |data: &'static str, id: String| move |app: &mut DroverApp| app.rpc("pane.write", json!({"pane": id, "data": data}));
    let rpc = |method: &'static str, id: String| move |app: &mut DroverApp| app.rpc(method, json!({"pane": id}));
    let body = match p.state.as_str() {
        "blocked" => div().flex().flex_col().gap(px(8.))
            .child(div().text_color(rgb(MUTED)).child(if p.title.is_empty() { "Waiting for your answer".into() } else { p.title.clone() }))
            .child(div().flex().gap(px(8.))
                .child(act("1 · Yes", Kind::Primary, cx, write("1", id.clone())))
                .child(act("2", Kind::Plain, cx, write("2", id.clone())))
                .child(act("3 · No…", Kind::Plain, cx, write("3", id.clone())))
                .child(div().flex_1())
                .child(act("Send Esc", Kind::Plain, cx, write("\x1b", id.clone())))
                .child(open("Open", id.clone(), cx))),
        "exited" => div().flex().items_center().gap(px(8.))
            .child(div().flex_1().text_size(px(12.)).text_color(rgb(DIM)).child(format!("exit code {}", p.exit_code.map(|c| c.to_string()).unwrap_or("?".into()))))
            .child(act("Restart & resume", Kind::Plain, cx, rpc("pane.restart", id.clone())))
            .child(act("Dismiss", Kind::Plain, cx, rpc("pane.close", id.clone()))),
        _ => div().flex().flex_col().gap(px(8.))
            .when(!p.title.is_empty(), |d| d.child(div().text_color(rgb(DIM)).child(p.title.clone())))
            .child(div().flex().gap(px(8.)).child(div().flex_1())
                .child(act("Mark read", Kind::Plain, cx, rpc("pane.focus", id.clone())))
                .child(open("Open", id.clone(), cx))),
    };
    div().flex().flex_col().gap(px(10.)).p(px(12.)).rounded(px(10.)).border_1()
        .border_color(if app.focused.as_deref() == Some(p.id.as_str()) { theme::tint(theme::ACCENT, 0.6) } else { theme::border() })
        .child(top).child(body)
}

pub fn body(app: &DroverApp, cx: &mut Context<DroverApp>) -> Div {
    let agents: Vec<&Pane> = app.store.panes.iter().filter(|p| p.kind != "shell").collect();
    let f = app.inbox_filter;
    let n = |g: Filter| agents.iter().filter(|p| matches(p, g)).count();
    let mut bar = div().h(px(42.)).flex_none().flex().items_center().gap(px(4.)).px(px(14.)).border_b_1().border_color(theme::border());
    for (g, label) in [(Filter::All, "All"), (Filter::Blocked, "Needs you"), (Filter::Done, "Done"), (Filter::Unread, "Unread")] {
        bar = bar.child(ui::tab(&format!("{label} {}", n(g)), f == g)
            .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| { app.inbox_filter = g; cx.notify(); })));
    }
    bar = bar.child(div().flex_1()).child(act("Mark all read", Kind::Plain, cx, |app| {
        let unread: Vec<String> = app.store.panes.iter().filter(|p| p.unread).map(|p| p.id.clone()).collect();
        for id in unread { app.rpc("pane.focus", json!({"pane": id})); }
    }));
    let mut list = div().id("inbox").flex_1().min_h_0().overflow_y_scroll().flex().flex_col().gap(px(10.)).p(px(16.)).max_w(px(900.));
    for p in &app.store.policy_pending {
        list = list.child(policy_card(app, p, cx));
    }
    for (title, states) in GROUPS {
        let items: Vec<&&Pane> = agents.iter().filter(|p| matches(p, f) && states.contains(&p.state.as_str())).collect();
        if items.is_empty() { continue; }
        list = list.child(div().pt(px(6.)).text_size(px(11.5)).text_color(rgb(DIM)).font_weight(FontWeight::SEMIBOLD).child(title.to_uppercase()));
        for p in items { list = list.child(card(app, p, cx)); }
    }
    if agents.is_empty() {
        list = list.child(ui::note("No agents yet.", false));
    }
    div().flex().flex_col().flex_1().min_h_0().child(bar).child(list)
}
