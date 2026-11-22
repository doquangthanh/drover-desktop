//! Your own keyboard shortcuts. Open Settings → Keyboard (keys.toml) and add a `[desktop_keys]` table: a command's
//! id, then one chord or a list of chords:
//!
//! ```toml
//! [desktop_keys]
//! new_tab = "ctrl+alt+t"
//! copy_mode = ["ctrl+alt+c", "f6"]
//! ```
//!
//! A plugin action is bound the same way, by its quoted name: `"example.notes.save" = "ctrl+alt+n"`.
//! They are added to the built-in shortcuts (keys.rs) and win when a chord is used twice. Ctrl+K chords stay fixed.
//! Saving keys.toml, or the "Reload config" command, applies the change; the ids are the `IDS` below.

use std::collections::HashMap;

use gpui::KeyDownEvent;
use serde_json::json;

use crate::picker::Cmd;
use crate::toasts::{Target, Tone};
use crate::workspace_app::DroverApp;

pub const IDS: [(&str, Cmd); 39] = [
    ("commands", Cmd::Palette), ("jump", Cmd::Jump), ("peek", Cmd::Peek), ("find", Cmd::Find), ("rename", Cmd::Rename), ("rename_space", Cmd::RenameSpace),
    ("rename_tab", Cmd::RenameTab), ("copy_mode", Cmd::CopyMode), ("clear_pane", Cmd::ClearPane), ("select_all", Cmd::SelectAll),
    ("new_tab", Cmd::NewTab), ("close_tab", Cmd::CloseTab), ("next_tab", Cmd::NextTab), ("previous_tab", Cmd::PrevTab),
    ("settle_tab", Cmd::SettleTab), ("unsettle_tabs", Cmd::UnsettleTabs), ("split_right", Cmd::SplitRight), ("split_down", Cmd::SplitDown),
    ("close_pane", Cmd::ClosePane), ("zoom", Cmd::Zoom), ("toggle_sidebar", Cmd::Sidebar), ("agents_panel", Cmd::Agents),
    ("files_panel", Cmd::Files), ("changes_panel", Cmd::Changes), ("inbox", Cmd::Inbox), ("machines", Cmd::Machines),
    ("settings", Cmd::Settings), ("next_blocked", Cmd::NextBlocked), ("next_done", Cmd::NextDone), ("shortcuts", Cmd::Shortcuts),
    ("text_bigger", Cmd::ZoomIn), ("text_smaller", Cmd::ZoomOut), ("text_normal", Cmd::ZoomReset), ("interface_bigger", Cmd::AppZoomIn),
    ("interface_smaller", Cmd::AppZoomOut), ("interface_normal", Cmd::AppZoomReset), ("whats_new", Cmd::WhatsNew),
    ("feedback", Cmd::Feedback), ("reload_config", Cmd::ReloadConfig),
];

/// What a chord does: a built-in command, or a plugin action ("plugin.id.action").
#[derive(Clone, Debug, PartialEq)]
pub enum Binding {
    Cmd(Cmd),
    Plugin(String),
}

#[derive(Default)]
pub struct Keymap {
    map: HashMap<String, Binding>,
}

impl Keymap {
    pub fn get(&self, ev: &KeyDownEvent) -> Option<Binding> {
        if self.map.is_empty() { return None }
        let m = &ev.keystroke.modifiers;
        self.map.get(&canonical(m.control, m.alt, m.shift, m.platform, &ev.keystroke.key)).cloned()
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }
}

/// One spelling for a chord: modifiers in a fixed order, then the key, all lower case.
fn canonical(ctrl: bool, alt: bool, shift: bool, sup: bool, key: &str) -> String {
    let mut s = String::new();
    for (on, name) in [(ctrl, "ctrl"), (alt, "alt"), (shift, "shift"), (sup, "super")] {
        if on { s.push_str(name); s.push('+'); }
    }
    s + &key.to_lowercase()
}

/// "Ctrl+Alt+T" -> "ctrl+alt+t". None: no key, or more than one.
fn normalize(binding: &str) -> Option<String> {
    let (mut ctrl, mut alt, mut shift, mut sup) = (false, false, false, false);
    let mut key = None;
    for part in binding.split('+').map(|p| p.trim().to_lowercase()).filter(|p| !p.is_empty()) {
        match part.as_str() {
            "ctrl" | "control" => ctrl = true,
            "alt" | "option" => alt = true,
            "shift" => shift = true,
            "super" | "cmd" | "win" | "meta" => sup = true,
            k => {
                let k = match k { "esc" => "escape", "return" => "enter", "pgup" => "pageup", "pgdn" => "pagedown", "del" => "delete", k => k };
                if key.replace(k.to_string()).is_some() { return None }
            }
        }
    }
    key.map(|k| canonical(ctrl, alt, shift, sup, &k))
}

/// The `[desktop_keys]` table of keys.toml text: the chords, and a line for each thing that could not be used.
pub fn parse(text: &str) -> (Keymap, Vec<String>) {
    let (mut keymap, mut problems, mut inside) = (Keymap::default(), vec![], false);
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        if let Some(section) = line.strip_prefix('[') {
            inside = section.trim_end_matches(']').trim() == "desktop_keys";
            continue;
        }
        let (true, Some((id, value))) = (inside, line.split_once('=')) else { continue };
        let id = id.trim().trim_matches('"');
        // A name with a dot is a plugin action (it may be one that is not installed yet).
        let what = match IDS.iter().find(|(name, _)| *name == id) {
            Some(&(_, cmd)) => Binding::Cmd(cmd),
            None if id.contains('.') => Binding::Plugin(id.to_string()),
            None => {
                problems.push(format!("keys.toml: no command called \"{id}\""));
                continue;
            }
        };
        // Every quoted string on the line is a chord ("a" or ["a", "b"]).
        for binding in value.split('"').skip(1).step_by(2) {
            match normalize(binding) {
                Some(chord) => { keymap.map.insert(chord, what.clone()); }
                None => problems.push(format!("keys.toml: \"{binding}\" is not a shortcut (like \"ctrl+alt+t\")")),
            }
        }
    }
    (keymap, problems)
}

impl DroverApp {
    /// Read keys.toml from the daemon and apply `[desktop_keys]`. `announce`: say how many shortcuts it found.
    pub fn load_keymap(&mut self, announce: bool) {
        self.rpc_reply("settings.files", json!({}), move |app, r| {
            let text = r.ok().map(|d| crate::views::settings::file_text(&d, "keys.toml")).unwrap_or_default();
            let (keymap, mut problems) = parse(&text);
            let (ui, ui_problems) = crate::ui_config::parse(&text);
            problems.extend(ui_problems);
            app.ui = ui;
            let count = keymap.len();
            app.keymap = keymap;
            if !problems.is_empty() {
                app.toasts.error(problems.join("\n"));
            } else if announce {
                app.toasts.push("Config reloaded".into(), format!("{count} custom shortcut(s)"), String::new(), Tone::Done, "", Target::Nothing);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_chords_and_reports_problems() {
        let text = "[keys]\nnew_agent = \"c\"\n\n[desktop_keys]\n# mine\nnew_tab = \"Ctrl+Alt+T\"\ncopy_mode = [\"f6\", \"ctrl+shift+y\"]\n\"example.notes.save\" = \"ctrl+alt+n\"\nnope = \"f9\"\nzoom = \"ctrl+a+b\"\n";
        let (map, problems) = parse(text);
        assert_eq!(map.len(), 4);
        assert_eq!(map.map.get("ctrl+alt+t"), Some(&Binding::Cmd(Cmd::NewTab)));
        assert_eq!(map.map.get("ctrl+shift+y"), Some(&Binding::Cmd(Cmd::CopyMode)));
        assert_eq!(map.map.get("ctrl+alt+n"), Some(&Binding::Plugin("example.notes.save".into())));
        assert_eq!(problems.len(), 2);
    }

    #[test]
    fn normalizes_spelling() {
        assert_eq!(normalize("Shift+Ctrl+Esc"), Some("ctrl+shift+escape".into()));
        assert_eq!(normalize("ctrl+"), None);
    }
}
