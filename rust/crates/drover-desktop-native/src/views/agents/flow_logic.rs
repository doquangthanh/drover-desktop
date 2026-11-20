//! Pure helpers of the Flow board (port of `agent-flow-logic.ts`): the lanes of Claude Code sub-agents and the activity
//! bars, both built from `messages.list`. No gpui, so they are easy to test. Rows are indexes into the message list.

use std::collections::HashMap;

use drover_proto::agent_tree::AgentMessage;

/// Lane kind of a Claude Code sub-agent (Task / Agent tool): its id is `<pane>/<agent>` and it has no real pane.
pub const SUB_KIND: &str = "sub";

/// A sub-agent drawn as a lane of its own.
#[derive(Debug, Clone, PartialEq)]
pub struct SubLane {
    pub id: String,
    pub name: String,
    pub parent: String,
    pub state: String,
}

/// One lane per "sub-start": named by the agent type, done once "sub-done" arrived, else working while its pane works
/// (a closed or idle pane leaves it idle). `states` maps pane id -> state.
pub fn sub_lanes(messages: &[AgentMessage], states: &HashMap<String, String>) -> Vec<SubLane> {
    let done: Vec<&str> = messages.iter().filter(|m| m.kind == "sub-done").filter_map(|m| m.source.as_deref()).collect();
    let mut subs: Vec<SubLane> = Vec::new();
    for m in messages.iter().filter(|m| m.kind == "sub-start") {
        let (Some(target), Some(source)) = (m.target.as_deref().filter(|s| !s.is_empty()), m.source.as_deref().filter(|s| !s.is_empty())) else { continue };
        if subs.iter().any(|s| s.id == target) { continue }
        let owner = states.get(source).map(String::as_str);
        let state = if done.contains(&target) { "done" } else if matches!(owner, Some("working" | "blocked")) { "working" } else { "idle" };
        subs.push(SubLane { id: target.into(), name: if m.text.is_empty() { "agent".into() } else { m.text.clone() }, parent: source.into(), state: state.into() });
    }
    subs
}

/// The tree's lanes with each pane's sub-agents right after it, in the order they started.
pub fn with_sub_lanes(tree: &[String], subs: &[SubLane]) -> Vec<String> {
    let mut lanes = Vec::new();
    for id in tree {
        lanes.push(id.clone());
        lanes.extend(subs.iter().filter(|s| &s.parent == id).map(|s| s.id.clone()));
    }
    lanes
}

/// When an agent was active, as a bar on its lane.
#[derive(Debug, Clone, PartialEq)]
pub struct Bar {
    pub lane: usize,
    pub start_row: usize,
    pub end_row: usize,
    pub live: bool,
    pub state: String,
}

/// From the row that started each agent to its last "done" / "exited" (to the end while it runs). `states` holds
/// every lane that is a pane or a sub-agent; the "you" lane (id "") has no bar.
pub fn activity_bars(messages: &[AgentMessage], lane_ids: &[String], states: &HashMap<String, String>) -> Vec<Bar> {
    let last = messages.len().saturating_sub(1);
    let mut bars = Vec::new();
    for (lane, id) in lane_ids.iter().enumerate() {
        let Some(state) = states.get(id) else { continue };
        let mentions = |m: &AgentMessage| m.source.as_deref() == Some(id.as_str()) || m.target.as_deref() == Some(id.as_str());
        let Some(start) = messages.iter().position(|m| m.kind == "spawn" && m.target.as_deref() == Some(id.as_str()))
            .or_else(|| messages.iter().position(mentions)) else { continue };
        // No "done" / "exited" row (evicted, or an older daemon): the bar ends at the agent's last mention.
        let (mut end, mut last_mention) = (None, start);
        for (i, m) in messages.iter().enumerate() {
            if matches!(m.kind.as_str(), "done" | "exited") && m.source.as_deref() == Some(id.as_str()) { end = Some(i) }
            if mentions(m) { last_mention = i }
        }
        let live = matches!(state.as_str(), "working" | "blocked");
        let end_row = if live { last } else { end.filter(|e| *e >= start).unwrap_or(last_mention) };
        bars.push(Bar { lane, start_row: start, end_row, live, state: state.clone() });
    }
    bars
}

/// Header text of a state ("Working", "Needs you"...) for lanes that are not panes (no exit code to show).
pub fn state_text(state: &str) -> String {
    match state {
        "working" => "Working".into(),
        "blocked" => "Needs you".into(),
        "done" => "Done".into(),
        "idle" => "Idle".into(),
        "exited" => "Exited".into(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(kind: &str, source: Option<&str>, target: Option<&str>, text: &str) -> AgentMessage {
        AgentMessage { at: 0.0, source: source.map(String::from), target: target.map(String::from), kind: kind.into(), text: text.into() }
    }

    fn msgs() -> Vec<AgentMessage> {
        vec![m("spawn", None, Some("lead"), ""), m("sub-start", Some("lead"), Some("lead/a"), "planner"),
             m("sub-start", Some("lead"), Some("lead/b"), "tester"), m("sub-done", Some("lead/a"), Some("lead"), "plan ready")]
    }

    fn lead(state: &str) -> HashMap<String, String> {
        HashMap::from([("lead".to_string(), state.to_string())])
    }

    #[test]
    fn one_lane_per_sub_agent_named_by_its_type() {
        let subs = sub_lanes(&msgs(), &lead("working"));
        let got: Vec<_> = subs.iter().map(|s| (s.id.as_str(), s.name.as_str(), s.parent.as_str(), s.state.as_str())).collect();
        assert_eq!(got, vec![("lead/a", "planner", "lead", "done"), ("lead/b", "tester", "lead", "working")]);
    }

    #[test]
    fn a_sub_agent_whose_pane_stopped_is_not_live() {
        assert_eq!(sub_lanes(&msgs(), &lead("idle"))[1].state, "idle");
        assert_eq!(sub_lanes(&msgs(), &HashMap::new())[1].state, "idle");
    }

    #[test]
    fn sub_lanes_follow_their_pane() {
        let subs = sub_lanes(&msgs(), &lead("working"));
        let tree = vec!["lead".to_string(), "helper".to_string()];
        assert_eq!(with_sub_lanes(&tree, &subs), vec!["lead", "lead/a", "lead/b", "helper"]);
    }

    #[test]
    fn bars_run_from_start_to_done_or_to_the_end_while_live() {
        let messages = msgs();
        let mut states = lead("working");
        let subs = sub_lanes(&messages, &states);
        for s in &subs { states.insert(s.id.clone(), s.state.clone()); }
        let lanes: Vec<String> = std::iter::once(String::new()).chain(with_sub_lanes(&["lead".to_string()], &subs)).collect();
        let bars = activity_bars(&messages, &lanes, &states);
        let bar = |lane| bars.iter().find(|b| b.lane == lane).cloned();
        assert_eq!(bar(0), None); // "you"
        assert_eq!(bar(1).map(|b| (b.start_row, b.end_row, b.live)), Some((0, 3, true)));
        assert_eq!(bar(2).map(|b| (b.start_row, b.end_row, b.live)), Some((1, 3, false)));
        assert_eq!(bar(3).map(|b| (b.start_row, b.end_row, b.live)), Some((2, 3, true)));
    }

    #[test]
    fn without_a_done_row_the_bar_ends_at_the_last_mention() {
        let messages = vec![m("spawn", None, Some("a"), ""), m("send", Some("a"), Some("b"), "x"), m("send", Some("b"), None, "y")];
        let states = HashMap::from([("a".to_string(), "done".to_string())]);
        let lanes = vec![String::new(), "a".to_string()];
        assert_eq!(activity_bars(&messages, &lanes, &states).first().map(|b| (b.start_row, b.end_row)), Some((0, 1)));
    }
}
