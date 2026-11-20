//! Settings → Appearance (Herdr's "Look"): pick the theme. The choice is saved as `theme = "…"` at the top of
//! settings.toml (comments and other lines stay) and applied at once; the daemon reports it to every window.

use gpui::{Context, Div, FontWeight, MouseButton, div, prelude::*, px, rgb};
use serde_json::json;

use crate::theme::{self, Palette};
use crate::workspace_app::DroverApp;

const THEMES: [(&str, &str, &str); 4] = [
    ("dark", "Dark", "default"), ("light", "Light", ""), ("midnight", "Midnight", ""), ("auto", "Auto", "follows the system"),
];

/// Set a top-level `key = rhs` (before the first `[section]`), keeping every other line.
pub fn edit_top(text: &str, key: &str, rhs: &str) -> String {
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    let line = format!("{key} = {rhs}");
    let first_section = lines.iter().position(|l| l.trim_start().starts_with('[')).unwrap_or(lines.len());
    let is_key = |l: &String| { let t = l.trim_start(); t.strip_prefix(key).is_some_and(|r| r.trim_start().starts_with('=')) };
    match lines[..first_section].iter().position(is_key) {
        Some(i) => lines[i] = line,
        None => lines.insert(first_section, line),
    }
    lines.join("\n") + "\n"
}

impl DroverApp {
    pub fn choose_theme(&mut self, name: &'static str) {
        let Some(Ok(data)) = &self.settings.files else { return };
        let Some(text) = data["files"].as_array().and_then(|f| f.iter().find(|f| f["name"] == "settings.toml")).and_then(|f| f["text"].as_str()) else { return };
        let new_text = edit_top(text, "theme", &format!("\"{name}\""));
        theme::set_theme(name);
        self.rpc_reply("settings.write", json!({"name": "settings.toml", "text": new_text}), |app, r| match r {
            Ok(_) => app.settings.files = None,
            Err(e) => app.toasts.error(e),
        });
    }
}

fn swatch(p: &Palette, label: &str, note: &str, on: bool) -> Div {
    let preview = div().w(px(150.)).h(px(70.)).rounded(px(8.)).bg(rgb(p.canvas)).border_1().border_color(theme::border2()).p(px(8.))
        .flex().flex_col().gap(px(5.))
        .child(div().h(px(8.)).w(px(60.)).rounded(px(3.)).bg(rgb(p.accent)))
        .child(div().h(px(6.)).w(px(110.)).rounded(px(3.)).bg(rgb(p.fg)))
        .child(div().h(px(6.)).w(px(80.)).rounded(px(3.)).bg(rgb(p.muted)));
    div().flex().flex_col().gap(px(6.)).p(px(8.)).rounded(px(10.)).border_1()
        .border_color(if on { rgb(theme::accent()).into() } else { theme::border() })
        .hover(|d| d.bg(theme::hover())).child(preview)
        .child(div().text_size(px(12.5)).text_color(rgb(theme::fg())).child(if note.is_empty() { label.to_string() } else { format!("{label} · {note}") }))
}

pub fn panel(app: &DroverApp, cx: &mut Context<DroverApp>) -> Div {
    let current = app.theme_choice.as_str();
    let mut row = div().flex().flex_wrap().gap(px(12.));
    for (id, label, note) in THEMES {
        let p = match id { "light" => &theme::LIGHT, "midnight" => &theme::MIDNIGHT, _ => &theme::DARK };
        row = row.child(swatch(p, label, note, current == id)
            .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| { app.theme_choice = id.to_string(); app.choose_theme(id); cx.notify(); })));
    }
    div().flex().flex_col().gap(px(12.)).p(px(18.)).max_w(px(760.))
        .child(div().text_size(px(11.)).text_color(rgb(theme::dim())).font_weight(FontWeight::SEMIBOLD).child("THEME"))
        .child(row)
        .child(div().text_size(px(12.)).text_color(rgb(theme::dim()))
            .child("Saved as theme = \"…\" in settings.toml. Terminal text size: Ctrl+= / Ctrl+- / Ctrl+0."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_the_top_level_key_only() {
        let t = "theme = \"dark\"\nscrollback = 5000\n\n[notify]\ntheme = \"x\"\n";
        assert_eq!(edit_top(t, "theme", "\"light\""), "theme = \"light\"\nscrollback = 5000\n\n[notify]\ntheme = \"x\"\n");
    }

    #[test]
    fn adds_the_key_before_the_first_section() {
        assert_eq!(edit_top("[notify]\nblocked = []\n", "theme", "\"light\""), "theme = \"light\"\n[notify]\nblocked = []\n");
        assert_eq!(edit_top("", "theme", "\"midnight\""), "theme = \"midnight\"\n");
    }
}
