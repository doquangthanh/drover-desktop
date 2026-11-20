//! Settings → Notifications, the controls (Herdr's "Alerts"): per event which channels notify you, and quiet hours.
//! They edit `[notify]` in settings.toml in place (comments and other lines stay) and save it with `settings.write`.

use gpui::{Context, Div, FontWeight, MouseButton, div, prelude::*, px, rgb};
use serde_json::{Value, json};

use crate::theme::{self, dim, fg, muted};
use crate::views::ui::{self, Kind};
use crate::workspace_app::DroverApp;

const EVENTS: [(&str, &str, &[&str]); 3] = [
    ("blocked", "An agent needs me", &["toast", "desktop", "sound"]),
    ("done", "An agent finishes", &["toast", "desktop"]),
    ("exited", "An agent crashes or stops", &["toast", "desktop"]),
];
const CHANNELS: [(&str, &str); 3] = [("toast", "Toast"), ("desktop", "Desktop"), ("sound", "Sound")];
const QUIET: [(&str, &str); 4] = [("", "Off"), ("22:00-07:00", "22:00 – 07:00"), ("23:00-07:00", "23:00 – 07:00"), ("00:00-08:00", "00:00 – 08:00")];

/// Set `key = rhs` inside `[notify]`, keeping every other line. Adds the key (or the whole section) when missing.
pub fn edit_notify(text: &str, key: &str, rhs: &str) -> String {
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    let line = format!("{key} = {rhs}");
    let Some(head) = lines.iter().position(|l| l.trim() == "[notify]") else {
        if !text.is_empty() && !text.ends_with('\n') { lines.push(String::new()); }
        lines.extend(["".into(), "[notify]".into(), line]);
        return lines.join("\n") + "\n";
    };
    let end = lines.iter().skip(head + 1).position(|l| l.trim_start().starts_with('[')).map(|i| head + 1 + i).unwrap_or(lines.len());
    let is_key = |l: &String| { let t = l.trim_start(); t.strip_prefix(key).is_some_and(|r| r.trim_start().starts_with('=')) };
    match lines[head + 1..end].iter().position(is_key) {
        Some(i) => lines[head + 1 + i] = line,
        None => {
            // After the section's last non-blank line, so a blank gap before the next section stays.
            let at = (head + 1..end).rev().find(|&i| !lines[i].trim().is_empty()).map(|i| i + 1).unwrap_or(head + 1);
            lines.insert(at, line);
        }
    }
    lines.join("\n") + "\n"
}

fn channels_of(app: &DroverApp, event: &str, default: &[&str]) -> Vec<String> {
    match app.store.notify.get(event).and_then(Value::as_array) {
        Some(a) => a.iter().filter_map(Value::as_str).map(String::from).collect(),
        None => default.iter().map(|s| s.to_string()).collect(),
    }
}

fn toml_list(v: &[String]) -> String {
    format!("[{}]", v.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(", "))
}

impl DroverApp {
    /// Save one `[notify]` key. `value` is what the daemon will report (the store follows at once), `rhs` the TOML.
    fn save_notify(&mut self, key: &str, value: Value, rhs: String) {
        let Some(Ok(data)) = &self.settings.files else { return };
        let Some(text) = data["files"].as_array().and_then(|f| f.iter().find(|f| f["name"] == "settings.toml")).and_then(|f| f["text"].as_str()) else { return };
        let new_text = edit_notify(text, key, &rhs);
        let key = key.to_string();
        self.rpc_reply("settings.write", json!({"name": "settings.toml", "text": new_text}), move |app, r| match r {
            Ok(_) => {
                if !app.store.notify.is_object() { app.store.notify = json!({}); }
                app.store.notify[&key] = value;
                app.settings.files = None; // reload the file view
            }
            Err(e) => app.toasts.error(e),
        });
    }

    fn toggle_channel(&mut self, event: &str, channel: &str, default: &[&str]) {
        let mut v = channels_of(self, event, default);
        match v.iter().position(|c| c == channel) { Some(i) => { v.remove(i); } None => v.push(channel.into()) }
        // Keep the order toast, desktop, sound.
        v.sort_by_key(|c| CHANNELS.iter().position(|(k, _)| k == c));
        let rhs = toml_list(&v);
        self.save_notify(event, json!(v), rhs);
    }
}

pub fn controls(app: &DroverApp, cx: &mut Context<DroverApp>) -> Div {
    let mut card = div().flex().flex_col().gap(px(4.)).p(px(14.)).border_b_1().border_color(theme::border())
        .child(div().text_size(px(11.)).text_color(rgb(dim())).font_weight(FontWeight::SEMIBOLD).child("NOTIFY ME WHEN"));
    for (event, label, default) in EVENTS {
        let on = channels_of(app, event, default);
        let mut row = div().flex().items_center().gap(px(14.)).h(px(30.)).child(div().flex_1().text_color(rgb(fg())).child(label));
        for (ch, name) in CHANNELS {
            let is_on = on.iter().any(|c| c == ch);
            row = row.child(div().flex().items_center().gap(px(6.)).text_size(px(12.)).text_color(rgb(muted()))
                .child(name).child(ui::toggle(is_on))
                .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| { app.toggle_channel(event, ch, default); cx.notify(); })));
        }
        card = card.child(row);
    }
    let quiet = app.store.notify["quiet_hours"].as_str().unwrap_or("").to_string();
    let mut q = div().flex().items_center().gap(px(6.)).h(px(34.)).child(div().w(px(120.)).text_color(rgb(fg())).child("Quiet hours"));
    for (value, label) in QUIET {
        q = q.child(ui::tab(label, quiet == value).on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| {
            app.save_notify("quiet_hours", json!(value), format!("\"{value}\""));
            cx.notify();
        })));
    }
    card.child(q)
        .child(div().text_size(px(12.)).text_color(rgb(dim())).child("Desktop and Sound are sent by the daemon; Toast is the card in this window. Quiet hours mute Desktop and Sound."))
        .child(div().flex().pt(px(4.)).child(ui::button("Send a test notification", Kind::Plain)
            .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| { app.test_notification(); cx.notify(); }))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_a_key_and_keeps_everything_else() {
        let t = "theme = \"dark\"\n\n[notify]\n# why\nblocked = [\"toast\"]\ndone = [\"toast\"]\n\n[orchestration]\nmax_depth = 3\n";
        let out = edit_notify(t, "blocked", "[\"desktop\"]");
        assert_eq!(out, t.replace("blocked = [\"toast\"]", "blocked = [\"desktop\"]"));
    }

    #[test]
    fn adds_a_missing_key_at_the_end_of_the_section() {
        let t = "[notify]\nblocked = []\n\n[orchestration]\nmax_depth = 3\n";
        assert_eq!(edit_notify(t, "quiet_hours", "\"22:00-07:00\""),
            "[notify]\nblocked = []\nquiet_hours = \"22:00-07:00\"\n\n[orchestration]\nmax_depth = 3\n");
    }

    #[test]
    fn adds_the_section_when_missing_and_does_not_confuse_prefixes() {
        assert_eq!(edit_notify("theme = \"dark\"\n", "done", "[]"), "theme = \"dark\"\n\n[notify]\ndone = []\n");
        // `done_extra` is another key: `done` must be added, not replace it.
        let out = edit_notify("[notify]\ndone_extra = 1\n", "done", "[]");
        assert!(out.contains("done_extra = 1") && out.contains("done = []"));
    }

    #[test]
    fn toml_list_is_valid_toml() {
        let s = format!("x = {}", toml_list(&["toast".into(), "sound".into()]));
        assert!(s.contains("[\"toast\", \"sound\"]"));
    }
}
