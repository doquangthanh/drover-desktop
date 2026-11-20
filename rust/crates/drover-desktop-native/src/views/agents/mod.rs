//! Agents view (port of `frontend/src/views/tree-view.tsx`): who started whom on the left, the selected agent and
//! its tree's state on the right, then its tree's activity (Flow, Messages, Policy).
//! Messages come from `messages.list`, fetched again when the tree, a state in it, or the "messages" tick changes.

mod activity;
mod flow_logic;
mod inspector;
mod list;

use drover_proto::agent_tree::{self, AgentMessage, TreeRow};
use drover_proto::wire::Pane;
use gpui::{Context, Div, div, prelude::*};
use serde_json::json;

use crate::workspace_app::DroverApp;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Flow,
    Messages,
    Policy,
}

/// What the view keeps while it is closed: selection, tab, the last messages and what they were fetched for.
#[derive(Default)]
pub struct AgentsState {
    pub selected: Option<String>,
    pub tab: Tab,
    /// `messages.list` of `fetched_for`'s tree: None while loading.
    pub messages: Option<Result<Vec<AgentMessage>, String>>,
    fetched_for: String,
    /// The focused pane last seen: focusing another agent selects it (the panel follows the focus, as in Herdr).
    last_focus: Option<String>,
}

/// The selected agent: the picked one while it exists, else the first root with helpers, else the first agent.
pub fn selected(app: &DroverApp, rows: &[TreeRow], agents: &[&Pane]) -> Option<String> {
    app.agents.selected.clone().filter(|id| agents.iter().any(|p| &p.id == id))
        .or_else(|| rows.iter().find(|r| agents.iter().any(|p| p.parent.as_deref() == Some(&r.id))).map(|r| r.id.clone()))
        .or_else(|| rows.first().map(|r| r.id.clone()))
}

/// Fetch the selected tree's messages when the tree, its states or the "messages" tick changed.
pub fn refresh(app: &mut DroverApp) {
    if app.focused != app.agents.last_focus {
        app.agents.last_focus = app.focused.clone();
        // Follow the focus; a shell has no agent, so nothing stays selected from the previous pane.
        app.agents.selected = app.focused.as_deref().and_then(|id| app.store.pane(id)).filter(|p| p.kind != "shell").map(|p| p.id.clone());
    }
    let agents = agent_tree::agents(&app.store.panes);
    let rows = agent_tree::tree_rows(&agents);
    let Some(sel) = selected(app, &rows, &agents) else { return };
    let root = agent_tree::root_of(&sel, &agents);
    let tree = agent_tree::subtree_ids(&root, &agents);
    let states: Vec<&str> = tree.iter().filter_map(|id| app.store.pane(id)).map(|p| p.state.as_str()).collect();
    let key = format!("{root}|{}|{}|{}", app.store.messages_tick, tree.join(","), states.join(","));
    if key == app.agents.fetched_for {
        return;
    }
    let root_changed = !app.agents.fetched_for.starts_with(&format!("{root}|"));
    app.agents.fetched_for = key.clone();
    if root_changed {
        app.agents.messages = None; // never show another tree's messages
    }
    app.rpc_reply("messages.list", json!({"pane": root}), move |app, r| {
        if app.agents.fetched_for != key {
            return; // a newer fetch is on its way
        }
        app.agents.messages = Some(r.and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string())));
    });
}

impl DroverApp {
    /// Select an agent. Beside the terminals (the panel) it also takes you to that agent: its tab, keys, read mark.
    /// In the full Agents view it only selects, to inspect it (the inspector has Open).
    pub fn pick_agent(&mut self, id: String, window: &mut gpui::Window, cx: &mut Context<Self>) {
        if self.view == crate::views::View::Terminals {
            self.show_pane(&id);
            window.focus(&self.term_focus);
        }
        self.agents.selected = Some(id);
        cx.notify();
    }

    pub fn agents_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.agents.tab = tab;
        cx.notify();
    }

    /// Text typed in the inspector's send box goes to the selected agent (`pane.send`, like `dr send`).
    pub fn send_to_selected_agent(&mut self, text: String) {
        let Some(pane) = self.agents.selected.clone().filter(|_| !text.trim().is_empty()) else { return };
        self.rpc("pane.send", json!({"pane": pane, "text": text}));
    }
}

/// The panel: the selected agent (the focused one, else the last picked) and its tree's activity.
pub fn panel_body(app: &DroverApp, cx: &mut Context<DroverApp>) -> Div {
    let agents = agent_tree::agents(&app.store.panes);
    // Beside the terminals this panel is the focused pane's agent, not the first one found.
    let focused = app.focused.as_deref().and_then(|id| app.store.pane(id));
    if focused.is_none_or(|p| p.kind == "shell") {
        let what = focused.map(|p| p.name.clone()).unwrap_or_default();
        return crate::views::ui::note(if what.is_empty() { "No pane focused.".to_string() }
            else { format!("{what} is a shell, not an agent. Focus an agent to see it here.") }, false);
    }
    match focused {
        Some(pane) => {
            let root = agent_tree::root_of(&pane.id, &agents);
            let tree = agent_tree::subtree_ids(&root, &agents);
            div().flex().flex_col().flex_1().min_h_0()
                .child(inspector::inspector(app, pane, &root, &tree, cx))
                .child(activity::activity(app, pane, &tree, cx))
        }
        None => crate::views::ui::note("No agents yet. Start one with `dr run` or from a shell.", false),
    }
}

/// The whole Agents view (from the sidebar's Agents heading): every tree on the left, the selected agent on the right.
pub fn body(app: &DroverApp, cx: &mut Context<DroverApp>) -> Div {
    let agents = agent_tree::agents(&app.store.panes);
    let rows = agent_tree::tree_rows(&agents);
    let sel = selected(app, &rows, &agents);
    let right = match sel.as_deref().and_then(|id| app.store.pane(id)) {
        Some(pane) => {
            let root = agent_tree::root_of(&pane.id, &agents);
            let tree = agent_tree::subtree_ids(&root, &agents);
            div().flex().flex_col().flex_1().min_h_0()
                .child(inspector::inspector(app, pane, &root, &tree, cx))
                .child(activity::activity(app, pane, &tree, cx))
        }
        None => crate::views::ui::note("No agents yet. Start one with `dr run` or from a shell.", false),
    };
    crate::views::ui::split(330., list::list(app, &rows, &agents, sel.as_deref(), cx), right)
}
