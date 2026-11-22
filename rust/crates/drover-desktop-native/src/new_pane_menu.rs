//! The tab bar's + menu (as Herdr's): a terminal, an agent from `[agents]` in settings (1–9 start one; an agent that
//! is not installed on the space's machine is greyed), a new space, a new task worktree, or the worktree list.

use gpui::{Context, Pixels, Point};
use serde_json::{Value, json};

use crate::context_menu::{Act, Entry, Icon, Menu};
use crate::views::View;
use crate::workspace_app::DroverApp;

/// The menu lines; `kinds` is `agent.kinds` (empty from a daemon that does not know it).
fn entries(kinds: &[Value]) -> Vec<Entry> {
    let line = |label: &str, icon: Icon, hint: &str, act: Act| Entry::Item {
        label: label.into(), icon: Some(icon), hint: hint.into(), key: None, act: Some(act),
    };
    let mut out = vec![line("Terminal", Icon::Kind("shell".into()), "Ctrl+N", Act::NewShell)];
    if !kinds.is_empty() {
        out.push(Entry::Head("Start an agent".into()));
    }
    let mut number = 0;
    for k in kinds {
        // `installed` missing (a machine that could not check, see open_new_menu): offered, as if installed.
        let (kind, installed) = (k["kind"].as_str().unwrap_or(""), k["installed"] != false);
        let label = k["label"].as_str().unwrap_or(kind).to_string();
        let key = installed.then(|| { number += 1; number }).filter(|n| *n <= 9).map(|n| n.to_string());
        let hint = if installed { key.clone().unwrap_or_default() } else { "not installed".into() };
        let act = installed.then(|| Act::StartAgent(k["command"].as_str().unwrap_or(kind).to_string()));
        out.push(Entry::Item { label, icon: Some(Icon::Kind(kind.into())), hint, key, act });
    }
    out.extend([
        Entry::Rule,
        line("New space", Icon::Name("plus"), "", Act::NewSpace),
        line("New worktree…", Icon::Name("branch"), "", Act::NewWorktree),
        line("Open worktree…", Icon::Name("branch"), "", Act::OpenWorktrees),
    ]);
    out
}

impl DroverApp {
    /// The current space's folder and, for a space on another machine, that machine's alias.
    fn space_target(&self) -> Option<(String, Option<String>)> {
        let s = self.space.as_ref().and_then(|id| self.store.spaces.iter().find(|x| &x.id == id))?;
        Some((s.cwd.clone(), s.machine.clone().filter(|m| !m.is_empty() && m != "local")))
    }

    /// `command` ("" = a shell) in a new tab of the current space, on that space's machine; with `worktree` the daemon
    /// first makes a task worktree (branch dr/<task>) and starts it there. No space yet: a new space instead.
    pub fn start_in_space(&mut self, command: &str, worktree: Option<&str>, cx: &mut Context<Self>) {
        let Some((cwd, machine)) = self.space_target() else { return self.new_space(cx) };
        let mut params = json!({"command": command, "cwd": cwd});
        if let Some(m) = machine {
            params["machine"] = json!(m);
        }
        if let Some(task) = worktree {
            params["worktree"] = json!({"task": task});
        }
        self.rpc_then("pane.create", params, Self::focus_created);
    }

    /// Open the + menu at `at`, once the space's machine said which agents it has. A remote machine on an older dr
    /// (no `agent.kinds`) cannot say: then this computer's `[agents]` are offered without the installed check.
    pub fn open_new_menu(&mut self, at: Point<Pixels>) {
        let Some(machine) = self.space_target().and_then(|t| t.1) else {
            return self.rpc_reply("agent.kinds", json!({}), move |app, r| app.show_new_menu(at, r.ok()));
        };
        self.rpc_reply("agent.kinds", json!({"machine": machine}), move |app, r| match r {
            Ok(v) => app.show_new_menu(at, Some(v)),
            Err(_) => app.rpc_reply("agent.kinds", json!({}), move |app, r| {
                let unchecked = r.ok().map(|v| json!(v.as_array().into_iter().flatten().map(|k| {
                    let mut k = k.clone();
                    k["installed"] = Value::Null;
                    k
                }).collect::<Vec<_>>()));
                app.show_new_menu(at, unchecked);
            }),
        });
    }

    fn show_new_menu(&mut self, at: Point<Pixels>, kinds: Option<Value>) {
        let kinds = kinds.and_then(|v| v.as_array().cloned()).unwrap_or_default();
        self.menu = Some(Menu { at, entries: entries(&kinds) });
    }

    /// The Changes panel on its Worktrees tab, for the focused pane.
    pub fn open_worktrees(&mut self) {
        self.set_panel(Some(View::Changes));
        self.view = View::Terminals;
        self.changes.show_trees = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn act_of(e: &Entry) -> Option<(&str, Option<&str>, bool)> {
        match e {
            Entry::Item { label, key, act, .. } => Some((label.as_str(), key.as_deref(), act.is_some())),
            _ => None,
        }
    }

    #[test]
    fn installed_agents_get_numbers_and_missing_ones_are_greyed() {
        let kinds = vec![
            json!({"kind": "claude", "label": "Claude Code", "command": "claude", "installed": true}),
            json!({"kind": "gemini", "label": "Gemini", "command": "gemini", "installed": false}),
            json!({"kind": "codex", "label": "Codex", "command": "codex", "installed": true}),
        ];
        let lines = entries(&kinds);
        let items: Vec<_> = lines.iter().filter_map(act_of).collect();
        assert_eq!(items[0], ("Terminal", None, true));
        assert_eq!(&items[1..4], &[("Claude Code", Some("1"), true), ("Gemini", None, false), ("Codex", Some("2"), true)]);
        assert_eq!(items.last().unwrap().0, "Open worktree…");
    }

    #[test]
    fn agents_a_machine_could_not_check_are_offered() {
        let lines = entries(&[json!({"kind": "codex", "label": "Codex", "command": "codex", "installed": null})]);
        assert!(lines.iter().filter_map(act_of).any(|i| i == ("Codex", Some("1"), true)));
    }

    #[test]
    fn no_agent_heading_without_agent_kinds() {
        assert!(!entries(&[]).iter().any(|e| matches!(e, Entry::Head(_))));
    }
}
