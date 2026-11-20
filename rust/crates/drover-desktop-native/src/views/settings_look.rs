//! Settings → Appearance (Herdr's "Look"): pick the theme. The choice is saved as `theme = "…"` at the top of
//! settings.toml (comments and other lines stay) and applied at once; the daemon reports it to every window.

use gpui::{Context, Div, Entity, FontWeight, MouseButton, div, prelude::*, rgb};
use crate::ui_scale::px;
use serde_json::json;

use crate::font_prefs::{self, OwnFont};
use crate::theme::{self, Palette};
use crate::views::text_input::TextInput;
use crate::views::ui::{self, Kind};
use crate::workspace_app::DroverApp;

/// Terminal font fields: family (any installed monospace font, or typed) and size.
pub struct FontForm {
    pub family: Entity<TextInput>,
    pub size: Entity<TextInput>,
    /// `fonts.list`: monospace families installed on the machine running the daemon.
    pub fonts: Option<Result<Vec<String>, String>>,
    asked: bool,
}

impl FontForm {
    pub fn new(cx: &mut Context<DroverApp>) -> Self {
        FontForm { family: cx.new(|cx| TextInput::new("Same as your system terminal", cx)), size: cx.new(|cx| TextInput::new("size", cx)),
                   fonts: None, asked: false }
    }
}

/// Load the font list the first time Appearance is open.
pub fn refresh(app: &mut DroverApp) {
    if app.font_form.asked { return }
    app.font_form.asked = true;
    app.rpc_reply("fonts.list", json!({}), |app, r| {
        app.font_form.fonts = Some(r.map(|v| v.as_array().into_iter().flatten().filter_map(|f| f.as_str()).map(String::from).collect()));
    });
}

impl DroverApp {
    /// Use `family` / `size` text from the form (empty = follow the daemon), keep it on this device and apply it now.
    pub fn apply_own_font(&mut self, family: &str, size: &str, cx: &mut Context<Self>) {
        let font = OwnFont { family: family.trim().to_string(), size: size.trim().parse::<f32>().ok().filter(|s| (8.0..=40.0).contains(s)).unwrap_or(0.0) };
        match font_prefs::save_to(&font_prefs::path(), &font) {
            Ok(()) => { self.font_own = font; self.refresh_font(); }
            Err(e) => self.toasts.error(format!("Could not save the terminal font: {e}")),
        }
        cx.notify();
    }
}

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
    let head = |t: &str| div().text_size(px(11.)).text_color(rgb(theme::dim())).font_weight(FontWeight::SEMIBOLD).child(t.to_string());
    let f = &app.font_form;
    let apply = cx.listener(|app, _, _, cx| {
        let (family, size) = (app.font_form.family.read(cx).value(), app.font_form.size.read(cx).value());
        app.apply_own_font(&family, &size, cx);
    });
    let reset = cx.listener(|app, _, _, cx| {
        app.font_form.family.update(cx, |i, cx| i.set_text("", cx));
        app.font_form.size.update(cx, |i, cx| i.set_text("", cx));
        app.apply_own_font("", "", cx);
    });
    let mut list = div().flex().flex_wrap().gap(px(6.));
    match &f.fonts {
        Some(Ok(fonts)) => for name in fonts.iter().take(60) {
            let n = name.clone();
            list = list.child(ui::tab(name, app.font_own.family == *name).on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| {
                app.font_form.family.update(cx, |i, cx| i.set_text(&n, cx));
                let size = app.font_form.size.read(cx).value();
                app.apply_own_font(&n, &size, cx);
            })));
        },
        Some(Err(e)) => list = list.child(div().text_size(px(12.)).text_color(rgb(theme::error())).child(e.clone())),
        None => list = list.child(div().text_size(px(12.)).text_color(rgb(theme::dim())).child("Loading fonts…")),
    }
    let sample = div().p(px(10.)).rounded(px(8.)).border_1().border_color(theme::border()).font_family(app.term_font.font.family.clone())
        .text_size(px(app.term_font.size)).child("$ claude  ❯ 1. Yes  0O 1lI {}[] => != ✓");
    let inner = div().id("settings-look").flex().flex_col().gap(px(12.)).p(px(18.)).max_w(px(760.)).size_full().overflow_y_scroll()
        .child(head("THEME"))
        .child(row)
        .child(div().text_size(px(12.)).text_color(rgb(theme::dim()))
            .child("Saved as theme = \"…\" in settings.toml. Terminal text size: Ctrl+= / Ctrl+- / Ctrl+0."))
        .child(head("INTERFACE SIZE (THIS DEVICE)"))
        .child(div().flex().items_center().gap(px(8.))
            .child(ui::button("−", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.zoom_app(-1, cx))))
            .child(div().w(px(48.)).flex().justify_center().text_color(rgb(theme::fg())).child(format!("{}%", crate::ui_scale::percent())))
            .child(ui::button("+", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.zoom_app(1, cx))))
            .child(ui::button("100%", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.zoom_app(0, cx))))
            .child(div().text_size(px(12.)).text_color(rgb(theme::dim())).child("Ctrl+Alt+= / - / 0. Terminal text has its own size (Ctrl+= / - / 0).")))
        .child(head("TERMINAL FONT (THIS DEVICE)"))
        .child(div().flex().items_center().gap(px(8.))
            .child(div().flex_1().child(f.family.clone())).child(div().w(px(80.)).child(f.size.clone()))
            .child(ui::button("Apply", Kind::Primary).on_mouse_down(MouseButton::Left, apply))
            .child(ui::button("Reset", Kind::Plain).on_mouse_down(MouseButton::Left, reset)))
        .child(sample)
        .child(list)
        .child(div().text_size(px(12.)).text_color(rgb(theme::dim()))
            .child("Pick a font or type its name, size 8–40. Empty = the same font and size as your system terminal."));
    div().flex().flex_col().flex_1().min_h_0().child(inner)
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
