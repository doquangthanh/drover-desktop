//! Agents view, the tree's activity: Flow (one lane per agent, one arrow per message), Messages (newest first)
//! and Policy (`[orchestration]`). Port of `agent-flow-board.tsx` and the tabs of `tree-view.tsx`. The Flow board
//! has a lane per Claude Code sub-agent and a bar per agent showing when it was active (`flow_logic.rs`).

use std::collections::{HashMap, HashSet};

use drover_proto::agent_tree::{self, AgentMessage};
use drover_proto::wire::Pane;
use gpui::{Context, Div, FontWeight, MouseButton, div, prelude::*, px, relative, rgb};

use super::Tab;
use super::flow_logic::{self, SUB_KIND, SubLane, state_text, with_sub_lanes};
use super::inspector::{label, state_color, state_label};
use crate::theme::{self, accent, blocked, dim, done, error, fg, muted, working, tint};
use crate::views::ui;
use crate::widgets::{age, kind_icon};
use crate::workspace_app::DroverApp;

fn kind_color(kind: &str) -> u32 {
    match kind {
        "spawn" => accent(),
        "send" | "keys" => working(),
        "done" | "sub-done" => done(),
        "blocked" => blocked(),
        "denied" | "exited" => error(),
        _ => muted(),
    }
}

const ROW: f32 = 40.;
/// Where the arrow line sits inside a row: the middle of the dot, so bars start and end on it.
const LINE_Y: f32 = 21.;
const GUTTER: f32 = 56.;
const LEGEND: [(&str, &str); 7] = [("spawn", "started · dr run"), ("send", "sends · dr send"), ("done", "finished"), ("wait", "waits · dr wait"),
    ("blocked", "needs you"), ("denied", "blocked by policy"), ("sub-start", "delegates · sub-agent")];

fn flow(app: &DroverApp, messages: &[AgentMessage], lanes: &[String], subs: &[SubLane], focus: Option<&HashSet<String>>) -> Div {
    let n = lanes.len().max(1) as f32;
    let lane = |id: &Option<String>| lanes.iter().position(|l| Some(l) == id.as_ref()).unwrap_or(0);
    let center = |i: usize| (i as f32 + 0.5) / n;
    let mut states: HashMap<String, String> = app.store.panes.iter().map(|p| (p.id.clone(), p.state.clone())).collect();
    states.extend(subs.iter().map(|s| (s.id.clone(), s.state.clone())));

    let mut legend = div().flex().flex_none().flex_wrap().gap(px(12.)).px(px(12.)).py(px(6.)).text_size(px(11.)).text_color(rgb(dim()));
    for (kind, text) in LEGEND {
        legend = legend.child(div().flex().items_center().gap(px(5.)).child(div().size(px(7.)).rounded_full().bg(rgb(kind_color(kind)))).child(text));
    }
    legend = legend.child(div().flex().items_center().gap(px(5.)).child(div().w(px(4.)).h(px(10.)).rounded(px(2.)).bg(rgb(dim()))).child("agent active"));

    let mut heads = div().flex().flex_none().pl(px(GUTTER)).border_b_1().border_color(theme::border());
    for id in lanes {
        let (icon, name, state) = match (app.store.pane(id), subs.iter().find(|s| &s.id == id)) {
            (Some(p), _) => (kind_icon(&p.kind), label(Some(p)), (state_label(p), state_color(&p.state))),
            (None, Some(s)) => (kind_icon(SUB_KIND), s.name.clone(), (format!("sub-agent · {}", state_text(&s.state)), state_color(&s.state))),
            _ => (div().size(px(14.)), "You".into(), (String::new(), dim())),
        };
        heads = heads.child(div().flex_1().min_w_0().flex().flex_col().items_center().gap(px(3.)).py(px(8.)).child(icon)
            .child(div().max_w_full().truncate().text_size(px(12.)).child(name))
            .child(div().max_w_full().truncate().text_size(px(10.5)).text_color(rgb(state.1)).child(state.0)));
    }

    // Behind the rows: a thin life line per lane and a bar where the agent was active.
    let mut overlay = div().absolute().top_0().bottom_0().left(px(GUTTER)).right_0();
    for i in 0..lanes.len() {
        overlay = overlay.child(div().absolute().top_0().bottom_0().w(px(1.)).left(relative(center(i))).bg(theme::border()));
    }
    for b in flow_logic::activity_bars(messages, lanes, &states) {
        let h = ((b.end_row - b.start_row) as f32 * ROW).max(8.) + if b.live { 14. } else { 0. };
        let color = state_color(&b.state);
        overlay = overlay.child(div().absolute().top(px(b.start_row as f32 * ROW + LINE_Y)).h(px(h)).w(px(4.)).ml(px(-2.)).rounded(px(2.))
            .left(relative(center(b.lane))).bg(tint(color, if b.live { 0.9 } else { 0.45 })));
    }

    let mut rows = div().relative().flex().flex_col().child(overlay);
    for m in messages {
        let (a, b) = (lane(&m.source), lane(&m.target));
        let (lo, hi) = (center(a.min(b)), center(a.max(b)));
        let color = kind_color(&m.kind);
        let dim = focus.is_some_and(|f| !agent_tree::touches(m, f));
        let verb = agent_tree::flow_verb(&m.kind).to_string();
        let pill_text = if m.text.is_empty() { verb.clone() } else { format!("{verb} · {}", m.text) };
        let track = div().relative().flex_1().h_full()
            .when(hi > lo, |d| d.child(div().absolute().top(px(20.)).h(px(2.)).left(relative(lo)).w(relative(hi - lo)).bg(rgb(color))))
            .child(div().absolute().top(px(16.)).size(px(10.)).rounded_full().bg(rgb(color)).left(relative(center(a))).ml(px(-5.)))
            .child(div().absolute().top(px(2.)).left(relative(lo)).w(relative((hi - lo).max(1. / n))).flex().justify_center()
                .child(div().max_w_full().truncate().px(px(6.)).rounded(px(5.)).text_size(px(11.)).bg(tint(color, 0.16))
                    .text_color(rgb(color)).child(pill_text)));
        rows = rows.child(div().h(px(ROW)).flex_none().flex().when(dim, |d| d.opacity(0.35))
            .child(div().w(px(GUTTER)).flex_none().pl(px(10.)).pt(px(12.)).text_size(px(11.)).text_color(rgb(crate::theme::dim())).child(age(m.at)))
            .child(track));
    }
    if messages.is_empty() {
        rows = rows.child(ui::note("No agent-to-agent activity yet. Agents use `dr run`, `dr wait`, `dr send`.", false));
    }
    div().flex().flex_col().flex_1().min_h_0().child(legend).child(heads)
        .child(div().flex().flex_col().flex_1().min_h_0().child(div().id("agents-flow").size_full().overflow_y_scroll().child(rows)))
}

fn messages_list(app: &DroverApp, messages: &[AgentMessage]) -> Div {
    let name = |id: &Option<String>| id.as_deref().filter(|s| !s.is_empty()).map(|id| label(app.store.pane(id)))
        .filter(|s| !s.is_empty()).unwrap_or_else(|| id.clone().filter(|s| !s.is_empty()).unwrap_or("you".into()));
    let mut list = div().id("agents-messages").flex_1().min_h_0().overflow_y_scroll().flex().flex_col().px(px(14.)).py(px(6.));
    for m in messages.iter().rev() {
        list = list.child(div().flex().gap(px(10.)).py(px(7.)).border_b_1().border_color(theme::border())
            .child(div().w(px(36.)).flex_none().text_size(px(11.)).text_color(rgb(dim())).child(age(m.at)))
            .child(div().size(px(7.)).mt(px(5.)).flex_none().rounded_full().bg(rgb(kind_color(&m.kind))))
            .child(div().flex_1().min_w_0().flex().flex_col().gap(px(3.))
                .child(div().flex().flex_wrap().gap(px(4.))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(name(&m.source)))
                    .child(div().text_color(rgb(muted())).child(agent_tree::message_verb(&m.kind).to_string()))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(name(&m.target))))
                .when(!m.text.is_empty(), |d| d.child(div().text_size(px(11.5)).text_color(rgb(dim())).child(m.text.clone())))));
    }
    if messages.is_empty() {
        list = list.child(ui::note("No agent-to-agent activity yet. Agents use `dr run`, `dr wait`, `dr send`.", false));
    }
    div().flex().flex_col().flex_1().min_h_0().child(list)
}

fn policy(app: &DroverApp) -> Div {
    let o = &app.store.orchestration;
    let show = |v: &serde_json::Value, none: &str| match v {
        serde_json::Value::Null => none.to_string(),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    let max = o["max_panes"].as_i64().filter(|n| *n > 0).map(|n| n.to_string()).unwrap_or("no limit".into());
    let tile = |k: &str, v: String| div().flex_1().min_w(px(180.)).p(px(12.)).flex().flex_col().gap(px(4.)).rounded(px(8.))
        .border_1().border_color(theme::border())
        .child(div().text_size(px(11.)).text_color(rgb(dim())).child(k.to_string()))
        .child(div().text_size(px(14.)).text_color(rgb(fg())).child(v));
    div().flex().flex_col().gap(px(10.)).p(px(14.))
        .child(div().flex().flex_wrap().gap(px(8.))
            .child(tile("Agents may answer prompts", show(&o["agents_may_answer_permission_prompts"], "ask-user")))
            .child(tile("Max agents", max))
            .child(tile("Max depth", show(&o["max_depth"], "?")))
            .child(tile("Stop helpers with parent", o["kill_children_on_parent_exit"].as_bool().unwrap_or(false).to_string())))
        .child(div().text_size(px(12.)).text_color(rgb(dim())).child("Edit these under [orchestration] in settings.toml."))
}

pub fn activity(app: &DroverApp, pane: &Pane, tree: &[String], cx: &mut Context<DroverApp>) -> Div {
    let tab = app.agents.tab;
    let mut tabs = div().flex().gap(px(4.));
    for (t, name) in [(Tab::Flow, "Flow"), (Tab::Messages, "Messages"), (Tab::Policy, "Policy")] {
        tabs = tabs.child(ui::tab(name, tab == t).on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| app.agents_tab(t, cx))));
    }
    let root = tree.first().cloned().unwrap_or_default();
    let focus: Option<HashSet<String>> = (pane.id != root).then(|| {
        let agents = agent_tree::agents(&app.store.panes);
        agent_tree::subtree_ids(&pane.id, &agents).into_iter().collect()
    });
    let body = match (&app.agents.messages, tab) {
        (_, Tab::Policy) => policy(app),
        (None, _) => ui::note("Loading…", false),
        (Some(Err(e)), _) => ui::note(e.clone(), true),
        (Some(Ok(m)), Tab::Flow) => {
            let states: HashMap<String, String> = app.store.panes.iter().map(|p| (p.id.clone(), p.state.clone())).collect();
            let subs: Vec<SubLane> = flow_logic::sub_lanes(m, &states).into_iter().filter(|s| tree.contains(&s.parent)).collect();
            let lanes: Vec<String> = std::iter::once(String::new()).chain(with_sub_lanes(tree, &subs)).collect();
            flow(app, m, &lanes, &subs, focus.as_ref())
        }
        (Some(Ok(m)), Tab::Messages) => {
            let shown: Vec<AgentMessage> = m.iter().filter(|m| focus.as_ref().is_none_or(|f| agent_tree::touches(m, f))).cloned().collect();
            messages_list(app, &shown)
        }
    };
    let count = match &app.agents.messages {
        Some(Ok(m)) if tab != Tab::Policy => format!("{} event{}", m.len(), if m.len() == 1 { "" } else { "s" }),
        _ => String::new(),
    };
    div().flex().flex_col().flex_1().min_h_0()
        .child(div().h(px(42.)).flex_none().flex().items_center().justify_between().px(px(12.)).border_b_1().border_color(theme::border())
            .child(tabs).child(div().text_size(px(11.5)).text_color(rgb(dim())).child(count)))
        .child(body)
}
