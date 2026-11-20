//! Inbox (port of `inbox-view.tsx` + `inbox-card.tsx`): agents sorted by who needs you, quick answers without
//! opening the pane, and policy prompts (an agent tried to answer for another), a machine filter, and a side panel
//! (`inbox-side-panel.tsx`) with what a notification looks like and which events notify you.

use drover_proto::wire::Pane;
use gpui::{Context, Div, FontWeight, MouseButton, div, prelude::*, rgb};
use crate::ui_scale::px;
use serde_json::{Value, json};

use crate::theme::{self, blocked, dim, fg, muted, working, tint};
use crate::views::ui::{self, Kind};
use crate::widgets::{age, kind_icon, state_mark};
use crate::toasts::{Tone, card_face};
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

fn in_machine(p: &Pane, machine: &str) -> bool {
    machine.is_empty() || if machine == "local" { p.machine.is_empty() || p.machine == "local" } else { p.machine == machine }
}

/// "" (all), "local", then each remote machine's alias.
fn machine_options(app: &DroverApp) -> Vec<String> {
    let aliases = app.store.machines.as_array().into_iter().flatten().filter_map(|m| m["alias"].as_str()).map(String::from);
    ["".to_string(), "local".to_string()].into_iter().chain(aliases).collect()
}

fn machine_label(m: &str) -> String {
    match m { "" => "All machines".into(), "local" => "This computer".into(), other => other.to_string() }
}

/// "Toast · Desktop", "Off" when empty, "All" when every channel is on.
fn channel_label(v: &Value, all: bool) -> String {
    let Some(a) = v.as_array().filter(|a| !a.is_empty()) else { return "Off".into() };
    if all && a.len() >= 3 { return "All".into(); }
    a.iter().filter_map(Value::as_str).map(|x| { let mut c = x.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() })
        .collect::<Vec<_>>().join(" · ")
}

fn side_panel(app: &DroverApp, cx: &mut Context<DroverApp>) -> gpui::Stateful<Div> {
    let n = &app.store.notify;
    let pick = |state: &str| app.store.panes.iter().find(|p| p.state == state && p.kind != "shell");
    let sample = |p: &Pane, tone: Tone, text: &str| card_face(&format!("{} {text}", p.name), if p.title.is_empty() { &p.command } else { &p.title }, &p.kind, tone);
    let row = |name: &str, value: String, dim: bool| div().flex().items_center().gap(px(8.)).h(px(26.))
        .child(div().flex_1().text_color(rgb(muted())).child(name.to_string()))
        .child(div().text_color(rgb(if dim { crate::theme::dim() } else { theme::accent() })).child(value));
    let channels = |k: &str, default: &[&str]| n.get(k).cloned().unwrap_or_else(|| json!(default));
    let quiet = n["quiet_hours"].as_str().unwrap_or("").replace('-', " – ");
    let mut col = div().id("inbox-side").w(px(320.)).flex_none().overflow_y_scroll().flex().flex_col().gap(px(10.)).p(px(16.))
        .border_l_1().border_color(theme::border())
        .child(div().text_size(px(11.5)).text_color(rgb(dim())).child("In-app toast"));
    let (blocked, done) = (pick("blocked"), pick("done"));
    if let Some(p) = blocked { col = col.child(sample(p, Tone::Blocked, "needs you")); }
    if let Some(p) = done { col = col.child(sample(p, Tone::Done, "finished")); }
    if blocked.is_none() && done.is_none() {
        col = col.child(div().text_size(px(12.)).text_color(rgb(dim())).child("Toasts appear bottom-right when an agent needs you or finishes."));
    }
    col.child(div().pt(px(6.)).text_size(px(11.5)).text_color(rgb(dim())).child("Notify me when"))
        .child(div().flex().flex_col()
            .child(row("An agent needs me", channel_label(&channels("blocked", &["toast", "desktop", "sound"]), false), false))
            .child(row("An agent finishes", channel_label(&channels("done", &["toast", "desktop"]), false), false))
            .child(row("An agent crashes", channel_label(&channels("exited", &["toast", "desktop"]), true), false))
            .child(row("Quiet hours", if quiet.is_empty() { "Off".into() } else { quiet.clone() }, quiet.is_empty())))
        .child(div().text_size(px(11.5)).text_color(rgb(dim())).child("Edit these in Settings ▸ Notifications."))
        .child(div().flex().child(act("Send a test notification", Kind::Plain, cx, |app| app.test_notification())))
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
    div().flex().flex_col().gap(px(8.)).p(px(12.)).rounded(px(10.)).bg(tint(blocked(), 0.1)).border_1().border_color(tint(blocked(), 0.3))
        .child(div().font_weight(FontWeight::SEMIBOLD).child(format!("{} tried to answer “{}” for {}.", name("source"),
            p["text"].as_str().unwrap_or(""), name("target"))))
        .child(div().text_size(px(12.)).text_color(rgb(muted())).child("Your policy says agents can't approve permission prompts, so it's waiting for you."))
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
        .child(div().flex_none().font_weight(FontWeight::SEMIBOLD).text_color(rgb(fg())).child(p.name.clone()))
        .child(div().flex_1().min_w_0().truncate().text_size(px(12.)).text_color(rgb(dim())).child(format!("{space} · {machine} · {detail}")))
        .child(state_mark(&p.state, p.exit_code, gpui::SharedString::from(format!("ib-{}", p.id))))
        .when(p.unread, |d| d.child(div().size(px(7.)).rounded_full().bg(rgb(working()))));
    let id = p.id.clone();
    if !matches!(p.state.as_str(), "blocked" | "done" | "exited") {
        return ui::row(false).h(px(40.)).px(px(12.)).child(div().flex_1().min_w_0().child(top))
            .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, window, cx| { app.show_pane(&id); window.focus(&app.term_focus); cx.notify(); }));
    }
    let write = |data: &'static str, id: String| move |app: &mut DroverApp| app.rpc("pane.write", json!({"pane": id, "data": data}));
    let rpc = |method: &'static str, id: String| move |app: &mut DroverApp| app.rpc(method, json!({"pane": id}));
    let body = match p.state.as_str() {
        "blocked" => div().flex().flex_col().gap(px(8.))
            .child(div().text_color(rgb(muted())).child(if p.title.is_empty() { "Waiting for your answer".into() } else { p.title.clone() }))
            .child(div().flex().gap(px(8.))
                .child(act("1 · Yes", Kind::Primary, cx, write("1", id.clone())))
                .child(act("2", Kind::Plain, cx, write("2", id.clone())))
                .child(act("3 · No…", Kind::Plain, cx, write("3", id.clone())))
                .child(div().flex_1())
                .child(act("Send Esc", Kind::Plain, cx, write("\x1b", id.clone())))
                .child(open("Open", id.clone(), cx))),
        "exited" => div().flex().items_center().gap(px(8.))
            .child(div().flex_1().text_size(px(12.)).text_color(rgb(dim())).child(format!("exit code {}", p.exit_code.map(|c| c.to_string()).unwrap_or("?".into()))))
            .child(act("Restart & resume", Kind::Plain, cx, rpc("pane.restart", id.clone())))
            .child(act("Dismiss", Kind::Plain, cx, rpc("pane.close", id.clone()))),
        _ => div().flex().flex_col().gap(px(8.))
            .when(!p.title.is_empty(), |d| d.child(div().text_color(rgb(dim())).child(p.title.clone())))
            .child(div().flex().gap(px(8.)).child(div().flex_1())
                .child(act("Mark read", Kind::Plain, cx, rpc("pane.focus", id.clone())))
                .child(open("Open", id.clone(), cx))),
    };
    div().flex().flex_col().gap(px(10.)).p(px(12.)).rounded(px(10.)).border_1()
        .border_color(if app.focused.as_deref() == Some(p.id.as_str()) { theme::tint(theme::accent(), 0.6) } else { theme::border() })
        .child(top).child(body)
}

pub fn body(app: &DroverApp, cx: &mut Context<DroverApp>) -> Div {
    let agents: Vec<&Pane> = app.store.panes.iter().filter(|p| p.kind != "shell" && in_machine(p, &app.inbox_machine)).collect();
    let f = app.inbox_filter;
    let n = |g: Filter| agents.iter().filter(|p| matches(p, g)).count();
    let mut bar = div().h(px(42.)).flex_none().flex().items_center().gap(px(4.)).px(px(14.)).border_b_1().border_color(theme::border());
    for (g, label) in [(Filter::All, "All"), (Filter::Blocked, "Needs you"), (Filter::Done, "Done"), (Filter::Unread, "Unread")] {
        bar = bar.child(ui::tab(&format!("{label} {}", n(g)), f == g)
            .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| { app.inbox_filter = g; cx.notify(); })));
    }
    let options = machine_options(app);
    bar = bar.child(div().flex_1()).child(act(&machine_label(&app.inbox_machine), Kind::Plain, cx, move |app| {
        let i = options.iter().position(|m| *m == app.inbox_machine).map(|i| i + 1).unwrap_or(0);
        app.inbox_machine = options[i % options.len()].clone();
    })).child(act("Mark all read", Kind::Plain, cx, |app| {
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
        list = list.child(div().pt(px(6.)).text_size(px(11.5)).text_color(rgb(dim())).font_weight(FontWeight::SEMIBOLD).child(title.to_uppercase()));
        for p in items { list = list.child(card(app, p, cx)); }
    }
    if agents.is_empty() {
        list = list.child(ui::note("No agents yet.", false));
    }
    div().flex().flex_row().flex_1().min_h_0()
        .child(div().flex().flex_col().flex_1().min_w_0().min_h_0().child(bar).child(list))
        .child(side_panel(app, cx))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane(machine: &str) -> Pane { Pane { machine: machine.into(), ..Default::default() } }

    #[test]
    fn machine_filter_matches_all_local_or_one_alias() {
        assert!(in_machine(&pane("box"), ""));
        assert!(in_machine(&pane(""), "local") && in_machine(&pane("local"), "local"));
        assert!(!in_machine(&pane("box"), "local"));
        assert!(in_machine(&pane("box"), "box") && !in_machine(&pane("other"), "box"));
    }

    #[test]
    fn channel_label_lists_capitalised_channels() {
        assert_eq!(channel_label(&json!(["toast", "desktop"]), false), "Toast · Desktop");
        assert_eq!(channel_label(&json!([]), false), "Off");
        assert_eq!(channel_label(&Value::Null, false), "Off");
        assert_eq!(channel_label(&json!(["toast", "desktop", "sound"]), true), "All");
    }
}
