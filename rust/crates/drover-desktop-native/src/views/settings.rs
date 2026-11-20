//! Settings (port of `settings-view.tsx`): every setting is a file in the config folder. The native window shows
//! each file read-only with its validation errors; "Open in editor" opens it in the system editor (the daemon
//! reloads it on save) and the web UI still has the in-place editor. Updates are checked and applied here.

use gpui::{Context, Div, FontWeight, MouseButton, div, prelude::*, px, rgb};
use serde_json::{Value, json};

use crate::actions::{Confirm, Then, open_in_browser};
use crate::theme::{self, dim, done, error, muted};
use crate::views::files::text_view;
use crate::views::ui::{self, Kind};
use crate::workspace_app::DroverApp;

/// Label, file, section marker (shown as a hint).
const SECTIONS: [(&str, &str, &str); 9] = [
    ("General", "settings.toml", ""), ("Appearance", "settings.toml", "theme"), ("Notifications", "settings.toml", "[notify]"), ("Keyboard", "keys.toml", ""),
    ("Machines", "machines.toml", ""), ("Agent policy", "settings.toml", "[orchestration]"),
    ("Notes for agents", "AGENTS.md", ""), ("Updates", "", ""), ("Server", "", ""),
];

#[derive(Default)]
pub struct SettingsState {
    pub section: usize,
    /// `settings.files`: {dir, files: [{name, path, exists, text}], errors}.
    pub files: Option<Result<Value, String>>,
    pub update: Option<Result<Value, String>>,
    /// `server.info`, loaded when the Server section is open.
    pub server: Option<Result<Value, String>>,
    loading: bool,
}

pub fn refresh(app: &mut DroverApp) {
    if app.settings.files.is_none() && !app.settings.loading {
        app.settings.loading = true;
        app.rpc_reply("settings.files", json!({}), |app, r| {
            app.settings.loading = false;
            app.settings.files = Some(r);
        });
    }
    if SECTIONS[app.settings.section].0 == "Updates" && app.settings.update.is_none() {
        app.settings.update = Some(Err("Checking…".into()));
        app.rpc_reply("update.check", json!({}), |app, r| app.settings.update = Some(r));
    }
    if SECTIONS[app.settings.section].0 == "Server" {
        crate::views::settings_server::refresh(app);
    }
}

impl DroverApp {
    pub fn ask_update(&mut self, tag: String, latest: String, cx: &mut Context<Self>) {
        self.confirm = Some(Confirm { title: format!("Update Drover to v{latest}?"),
            detail: "The server restarts, so running agents stop (the session can be restored).".into(), ok: "Update", then: Then::UpdateApply(tag) });
        cx.notify();
    }
}

fn file_section(app: &DroverApp, data: &Value, name: &str, marker: &str, cx: &mut Context<DroverApp>) -> Div {
    let file = data["files"].as_array().and_then(|f| f.iter().find(|f| f["name"] == name)).cloned().unwrap_or(json!({}));
    let path = file["path"].as_str().map(String::from).unwrap_or_else(|| format!("{}/{name}", data["dir"].as_str().unwrap_or("")));
    let text = file["text"].as_str().unwrap_or("").to_string();
    let errors: Vec<String> = data["errors"].as_array().into_iter().flatten().filter_map(|e| e.as_str())
        .filter(|e| e.starts_with(name)).map(String::from).collect();
    let mut head = div().h(px(42.)).flex_none().flex().items_center().gap(px(8.)).px(px(12.)).border_b_1().border_color(theme::border())
        .child(div().flex_1().min_w_0().truncate().font_family(app.code_family.clone()).child(name.to_string()))
        .when(!marker.is_empty(), |d| d.child(div().text_size(px(12.)).text_color(rgb(dim())).child(format!("see {marker}"))))
        .when(file["exists"] == false, |d| d.child(div().text_size(px(12.)).text_color(rgb(dim())).child("new file")));
    head = head
        .child(ui::button("Open in editor", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(move |_, _, _, _| open_in_browser(&path))))
        .child(ui::button("Edit in browser", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, _| app.open_web("settings"))));
    let lines: Vec<String> = text.split('\n').map(String::from).collect();
    div().flex().flex_col().flex_1().min_h_0().min_w_0()
        .when(marker == "[notify]", |d| d.child(crate::views::settings_alerts::controls(app, cx)))
        .child(head)
        .children(errors.into_iter().map(|e| div().px(px(12.)).py(px(6.)).text_size(px(12.)).text_color(rgb(error())).child(e)))
        .child(text_view(std::sync::Arc::new(lines), app.code_family.clone()))
}

fn updates(app: &DroverApp, cx: &mut Context<DroverApp>) -> Div {
    let mut d = div().flex().flex_col().gap(px(10.)).p(px(18.)).max_w(px(700.))
        .child(div().text_size(px(11.)).text_color(rgb(dim())).font_weight(FontWeight::SEMIBOLD).child("UPDATES"));
    match &app.settings.update {
        Some(Ok(v)) => {
            d = d.child(div().child(format!("Installed: v{}", v["current"].as_str().unwrap_or("?"))));
            let latest = v["latest"].as_str().unwrap_or("").to_string();
            if v["available"].as_bool() != Some(true) {
                d = d.child(div().text_color(rgb(done())).child(format!("✓ Drover is up to date{}",
                    if latest.is_empty() { String::new() } else { format!(" (latest: v{latest})") })));
            } else {
                d = d.child(div().child(format!("New version v{latest} is available.")))
                    .when_some(v["notes"].as_str().filter(|n| !n.is_empty()), |d, n| d.child(div().text_size(px(12.)).text_color(rgb(muted())).child(n.to_string())));
                if v["installable"].as_bool() == Some(true) {
                    let tag = v["tag"].as_str().unwrap_or("").to_string();
                    d = d.child(div().flex().child(ui::button(&format!("Update to v{latest}"), Kind::Primary)
                        .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| app.ask_update(tag.clone(), latest.clone(), cx)))));
                } else {
                    d = d.child(div().text_size(px(12.)).text_color(rgb(dim())).child("Installed from source: update with git pull (or uv tool upgrade)."));
                }
            }
        }
        Some(Err(e)) => d = d.child(div().text_color(rgb(if e == "Checking…" { dim() } else { error() })).child(e.clone())),
        None => {}
    }
    d.child(div().flex().child(ui::button("Check for updates", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| {
        app.settings.update = None;
        cx.notify();
    }))))
}

pub fn body(app: &DroverApp, cx: &mut Context<DroverApp>) -> Div {
    let data = match &app.settings.files {
        None => return ui::note("Loading settings…", false),
        Some(Err(e)) => return ui::note(e.clone(), true),
        Some(Ok(v)) => v.clone(),
    };
    let problems = data["errors"].as_array().map(|e| e.len()).unwrap_or(0);
    let dir = data["dir"].as_str().unwrap_or("").to_string();
    let mut nav = div().flex().flex_col().gap(px(2.)).p(px(8.)).flex_1();
    for (i, (label, _, _)) in SECTIONS.iter().enumerate() {
        nav = nav.child(ui::row(app.settings.section == i).child(label.to_string())
            .on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| { app.settings.section = i; cx.notify(); })));
    }
    let open_dir = dir.clone();
    let left = div().flex().flex_col().flex_1()
        .child(nav)
        .child(div().flex().flex_col().gap(px(8.)).p(px(12.)).border_t_1().border_color(theme::border()).text_size(px(12.))
            .child(div().text_color(rgb(if problems > 0 { error() } else { done() }))
                .child(if problems > 0 { format!("{problems} problem{}", if problems > 1 { "s" } else { "" }) } else { "✓ All files valid".into() }))
            .child(div().text_color(rgb(dim())).child(format!("{dir} · plain files, reloads on save")))
            .child(div().flex().gap(px(6.))
                .child(ui::button("Open folder", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(move |_, _, _, _| open_in_browser(&open_dir))))
                .child(ui::button("Reload", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| { app.settings.files = None; app.settings.server = None; cx.notify(); })))));
    let (label, file, marker) = SECTIONS[app.settings.section];
    let right = if label == "Updates" { updates(app, cx) } else if label == "Server" { crate::views::settings_server::panel(app, cx) } else if label == "Appearance" { crate::views::settings_look::panel(app, cx) } else { file_section(app, &data, file, marker, cx) };
    ui::split(220., left, right)
}
