//! Settings (port of `settings-view.tsx`): every setting is a file in the config folder. The native window shows
//! each file read-only with its validation errors; "Open in editor" opens it in the system editor (the daemon
//! reloads it on save) and the web UI still has the in-place editor. Updates are checked and applied here.

use gpui::{Context, Div, FontWeight, MouseButton, div, prelude::*, rgb};
use crate::ui_scale::px;
use serde_json::{Value, json};

use crate::actions::{Confirm, Then, open_in_browser};
use crate::theme::{self, dim, done, error, muted};
use crate::views::ui::{self, Kind};
use crate::workspace_app::DroverApp;

/// Label, file, section marker (shown as a hint).
const SECTIONS: [(&str, &str, &str); 10] = [
    ("General", "settings.toml", ""), ("Appearance", "settings.toml", "theme"), ("Notifications", "settings.toml", "[notify]"), ("Keyboard", "keys.toml", ""),
    ("Machines", "machines.toml", ""), ("Agent policy", "settings.toml", "[orchestration]"),
    ("Notes for agents", "AGENTS.md", ""), ("Updates", "", ""), ("Server", "", ""), ("Rules tester", "", ""),
];

#[derive(Default)]
pub struct SettingsState {
    pub section: usize,
    /// `settings.files`: {dir, files: [{name, path, exists, text}], errors}.
    pub files: Option<Result<Value, String>>,
    pub update: Option<Result<Value, String>>,
    /// `server.info`, loaded when the Server section is open.
    pub server: Option<Result<Value, String>>,
    /// The file in the editor and the text it was loaded with (the editor is "modified" when it differs).
    pub editing: Option<(String, String)>,
    /// "Saved …" after a save, cleared when another file opens.
    pub note: Option<String>,
    loading: bool,
}

/// Text of config file `name` from `settings.files`.
pub fn file_text(data: &Value, name: &str) -> String {
    data["files"].as_array().and_then(|f| f.iter().find(|f| f["name"] == name)).and_then(|f| f["text"].as_str()).unwrap_or("").to_string()
}

/// Put the open file into the editor when another file was opened, or when it was reloaded and has no unsaved edits.
fn sync_editor(app: &mut DroverApp, cx: &mut Context<DroverApp>) {
    let (_, name, _) = SECTIONS[app.settings.section];
    let Some(Ok(data)) = &app.settings.files else { return };
    if name.is_empty() { return }
    let text = file_text(data, name);
    let current = app.settings_editor.read(cx).text.clone();
    let reload = match &app.settings.editing {
        Some((n, base)) if n == name => *base != text && current == *base,
        _ => true,
    };
    if reload {
        if app.settings.editing.as_ref().is_none_or(|(n, _)| n != name) { app.settings.note = None; }
        app.settings_editor.update(cx, |e, cx| e.set_text(&text, cx));
        app.settings.editing = Some((name.to_string(), text));
    }
}

impl DroverApp {
    /// Save the editor to its file (`settings.write`, validated by the daemon); the daemon reloads the file itself.
    pub fn save_setting_file(&mut self, cx: &mut Context<Self>) {
        let Some((name, _)) = self.settings.editing.clone() else { return };
        let text = self.settings_editor.read(cx).text.clone();
        self.settings.note = Some("Saving…".into());
        let saved = text.clone();
        let file = name.clone();
        self.rpc_reply("settings.write", json!({"name": name, "text": text}), move |app, r| match r {
            Ok(_) => {
                if file == "keys.toml" { app.load_keymap(true); }
                app.settings.note = Some("✓ Saved. Drover reloads it within 2 seconds.".into());
                app.settings.editing = app.settings.editing.take().map(|(n, _)| (n, saved));
                app.settings.files = None;
            }
            Err(e) => { app.settings.note = None; app.toasts.error(e); }
        });
        cx.notify();
    }
}

pub fn refresh(app: &mut DroverApp, cx: &mut Context<DroverApp>) {
    let family = app.code_family.clone();
    app.settings_editor.update(cx, |e, _| e.family = Some(family));
    sync_editor(app, cx);
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
    if SECTIONS[app.settings.section].0 == "Appearance" {
        crate::views::settings_look::refresh(app);
    }
    if SECTIONS[app.settings.section].0 == "Rules tester" {
        crate::views::settings_rules::refresh(app);
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
    let errors: Vec<String> = data["errors"].as_array().into_iter().flatten().filter_map(|e| e.as_str())
        .filter(|e| e.starts_with(name)).map(String::from).collect();
    let mut head = div().h(px(42.)).flex_none().flex().items_center().gap(px(8.)).px(px(12.)).border_b_1().border_color(theme::border())
        .child(div().flex_1().min_w_0().truncate().font_family(app.code_family.clone()).child(name.to_string()))
        .when(!marker.is_empty(), |d| d.child(div().text_size(px(12.)).text_color(rgb(dim())).child(format!("see {marker}"))))
        .when(file["exists"] == false, |d| d.child(div().text_size(px(12.)).text_color(rgb(dim())).child("new file")));
    let modified = app.settings.editing.as_ref().is_some_and(|(n, base)| n == name && *base != app.settings_editor.read(cx).text);
    head = head
        .when(modified, |d| d.child(div().text_size(px(12.)).text_color(rgb(dim())).child("modified")))
        .when_some(app.settings.note.clone().filter(|_| !modified), |d, n| d.child(div().text_size(px(12.)).text_color(rgb(done())).child(n)))
        .child(ui::button("Open in editor", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(move |_, _, _, _| open_in_browser(&path))))
        .child(ui::button("Revert", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| {
            let base = app.settings.editing.as_ref().map(|(_, b)| b.clone()).unwrap_or_default();
            app.settings_editor.update(cx, |e, cx| e.set_text(&base, cx));
        })))
        .child(ui::button("Save", Kind::Primary).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.save_setting_file(cx))));
    div().flex().flex_col().flex_1().min_h_0().min_w_0()
        .when(marker == "[notify]", |d| d.child(crate::views::settings_alerts::controls(app, cx)))
        .child(head)
        .children(errors.into_iter().map(|e| div().px(px(12.)).py(px(6.)).text_size(px(12.)).text_color(rgb(error())).child(e)))
        .child(div().flex().flex_col().flex_1().min_h_0().p(px(10.)).child(app.settings_editor.clone()))
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
    d.child(div().flex().gap(px(8.))
        .child(ui::button("Check for updates", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| {
            app.settings.update = None;
            cx.notify();
        })))
        .child(ui::button("What's new", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(|app, _, window, cx| app.set_view(crate::views::View::WhatsNew, window, cx))))
        .child(ui::button("Send feedback", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(|app, _, window, cx| app.set_view(crate::views::View::Feedback, window, cx)))))
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
    let right = if label == "Updates" { updates(app, cx) } else if label == "Rules tester" { crate::views::settings_rules::panel(app, cx) } else if label == "Server" { crate::views::settings_server::panel(app, cx) } else if label == "Appearance" { crate::views::settings_look::panel(app, cx) } else { file_section(app, &data, file, marker, cx) };
    ui::split(220., left, right)
}
