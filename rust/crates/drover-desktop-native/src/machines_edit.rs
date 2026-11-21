//! Changes to a saved machine from its card in Machines (Herdr's `machine rename` and `machine status`): Rename edits the
//! `[machines.<alias>]` heading of machines.toml (the daemon reloads the file and reconnects under the new name; agents
//! there keep running), and Test runs the same SSH check as the add form against what is saved.

use gpui::{Context, Window};
use serde_json::{Value, json};

use crate::picker::Kind;
use crate::toasts::{Target, Tone};
use crate::workspace_app::DroverApp;

/// The same rule the daemon applies to a new alias.
fn alias_ok(alias: &str) -> bool {
    let mut chars = alias.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && alias.len() <= 32
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

fn heading(line: &str, alias: &str) -> bool {
    let h = line.trim();
    h == format!("[machines.{alias}]") || h == format!("[machines.\"{alias}\"]")
}

/// machines.toml with the `[machines.old]` table (and its sub-tables) renamed; None when there is no such table.
fn renamed(text: &str, old: &str, new: &str) -> Option<String> {
    let mut found = false;
    let lines: Vec<String> = text.lines().map(|line| {
        let h = line.trim();
        if heading(line, old) {
            found = true;
            format!("[machines.{new}]")
        } else if let Some(rest) = h.strip_prefix(&format!("[machines.{old}.")) {
            found = true;
            format!("[machines.{new}.{rest}")
        } else {
            line.to_string()
        }
    }).collect();
    found.then(|| lines.join("\n") + if text.ends_with('\n') { "\n" } else { "" })
}

/// `port` and `key` of the machine in machines.toml text (22 and "" when not written).
fn saved_port_and_key(text: &str, alias: &str) -> (u16, String) {
    let (mut inside, mut port, mut key) = (false, 22, String::new());
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') { inside = heading(line, alias); continue }
        let (true, Some((k, v))) = (inside, line.split_once('=')) else { continue };
        match k.trim() {
            "port" => port = v.trim().parse().unwrap_or(22),
            "key" => key = v.split('"').nth(1).unwrap_or("").to_string(),
            _ => {}
        }
    }
    (port, key)
}

fn machines_text(files: &Value) -> String {
    crate::views::settings::file_text(files, "machines.toml")
}

impl DroverApp {
    pub fn open_rename_machine(&mut self, alias: String, window: &mut Window, cx: &mut Context<Self>) {
        self.open_picker_with(Kind::RenameMachine, ("New name", alias.clone()), Some(alias), window, cx);
    }

    pub fn rename_machine(&mut self, old: String, new: String) {
        if new == old { return }
        if !alias_ok(&new) {
            self.toasts.error("A machine name is lower case letters, digits, - and _ (up to 32), starting with a letter or digit.".into());
            return;
        }
        if self.store.machines.as_array().is_some_and(|m| m.iter().any(|m| m["alias"] == new.as_str())) {
            self.toasts.error(format!("There is already a machine called {new}."));
            return;
        }
        self.rpc_reply("settings.files", json!({}), move |app, r| {
            let text = r.map(|d| machines_text(&d)).unwrap_or_default();
            let Some(text) = renamed(&text, &old, &new) else {
                app.toasts.error(format!("{old} is not in machines.toml, so it cannot be renamed here."));
                return;
            };
            app.rpc_reply("settings.write", json!({"name": "machines.toml", "text": text}), move |app, r| match r {
                Ok(_) => app.toasts.push(format!("Renamed to {new}"), "Drover reconnects it within 2 seconds; agents there keep running.".into(),
                                         String::new(), Tone::Done, "", Target::Nothing),
                Err(e) => app.toasts.error(e),
            });
        });
    }

    /// Run the SSH check against a saved machine and say what it found.
    pub fn test_saved_machine(&mut self, alias: String, target: String) {
        self.toasts.push(format!("Testing {alias}…"), target.clone(), String::new(), Tone::Update, "", Target::Nothing);
        self.rpc_reply("settings.files", json!({}), move |app, r| {
            let (port, key) = r.map(|d| saved_port_and_key(&machines_text(&d), &alias)).unwrap_or((22, String::new()));
            app.rpc_reply("machine.test", json!({"target": target, "port": port, "key": key}), move |app, r| match r {
                Ok(v) if v["ok"] == true => {
                    let steps: Vec<&str> = v["steps"].as_array().into_iter().flatten().filter_map(|s| s.as_str()).collect();
                    app.toasts.push(format!("{alias} is reachable"), steps.join(" · "), String::new(), Tone::Done, "", Target::Nothing);
                }
                Ok(v) => app.toasts.error(format!("{alias}: {}", v["error"].as_str().unwrap_or("failed"))),
                Err(e) => app.toasts.error(format!("{alias}: {e}")),
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "# my machines\n[machines.vps-1]\ntarget = \"deploy@vps-1\"\nport = 2222\nkey = \"~/.ssh/id_vps\"\n\n[machines.vps-2]\ntarget = \"a@b\"\n";

    #[test]
    fn renames_only_the_named_table() {
        let out = renamed(FILE, "vps-1", "build").unwrap();
        assert!(out.contains("[machines.build]\ntarget = \"deploy@vps-1\""));
        assert!(out.contains("[machines.vps-2]") && out.ends_with('\n'));
        assert_eq!(renamed(FILE, "nope", "x"), None);
    }

    #[test]
    fn reads_port_and_key_of_one_machine() {
        assert_eq!(saved_port_and_key(FILE, "vps-1"), (2222, "~/.ssh/id_vps".into()));
        assert_eq!(saved_port_and_key(FILE, "vps-2"), (22, String::new()));
    }

    #[test]
    fn checks_alias_rules() {
        assert!(alias_ok("vps-2") && alias_ok("9lives"));
        assert!(!alias_ok("Vps") && !alias_ok("-a") && !alias_ok("") && !alias_ok("a b"));
    }
}
