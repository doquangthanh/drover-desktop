//! Files panel (port of `frontend/src/files/*`), as Herdr's: the focused pane's folder tree (lazy; an agent's
//! worktree outside the space too), git status per file, the open files as tabs in the code editor (docs.rs,
//! code_editor/) and, for a changed file, its diff vs HEAD. While open it polls `files.status` every 3 s and reloads
//! what changed.

mod content;
pub mod dock;
pub mod docs;
mod tree;


use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use drover_proto::changes::{self, DiffRow};
use gpui::{Context, Div, div, prelude::*};
use serde_json::{Value, json};

use crate::views::{View, ui};
use crate::workspace_app::DroverApp;

pub use drover_proto::files::{Entry, FileContent};

#[derive(Default)]
pub struct FilesState {
    /// The scope it shows (views/panel.rs): reset when the focused pane or its folder changes.
    space: String,
    space_id: String,
    pane: Option<String>,
    cwd: String,
    pub dirs: HashMap<String, Vec<Entry>>,
    pub open: HashSet<String>,
    /// git status letter per path ("M", "A", "D", "R", "?"); untracked folders end with "/".
    pub status: HashMap<String, String>,
    pub is_repo: bool,
    pub file: Option<String>,
    pub show_diff: bool,
    pub only_changed: bool,
    pub content: Option<Result<Loaded, String>>,
    pub diff: Option<Result<Arc<Vec<DiffRow>>, String>>,
    /// The shown file's diff vs HEAD as git printed it (the editor's gutter marks).
    pub raw_diff: Option<String>,
    /// files.read said the file can be saved (not cut at 1 MB, valid UTF-8).
    pub editable: bool,
    pub error: Option<String>,
    polled: Option<Instant>,
}

/// A read file and its lines (tabs expanded), ready to draw.
pub type Loaded = (FileContent, Arc<Vec<String>>);

pub const CHANGED: [&str; 4] = ["M", "A", "D", "R"];

fn space_params(app: &DroverApp, extra: Value) -> Value {
    // Both: a daemon that knows `pane` lists that pane's folder, an older one the space's.
    let mut p = json!({"space": app.files.space_id});
    if let Some(pane) = &app.files.pane {
        p["pane"] = json!(pane);
    }
    if let (Some(o), Some(e)) = (p.as_object_mut(), extra.as_object()) {
        o.extend(e.clone());
    }
    p
}

impl DroverApp {
    pub fn files_load_dir(&mut self, path: &str) {
        let (sp, path) = (self.files.space.clone(), path.to_string());
        self.rpc_reply("files.list", space_params(self, json!({"path": path})), move |app, r| {
            if app.files.space != sp { return; }
            match r.and_then(|v| serde_json::from_value::<Vec<Entry>>(v["entries"].clone()).map_err(|e| e.to_string())) {
                Ok(entries) => { app.files.dirs.insert(path, entries); }
                Err(e) if path.is_empty() => app.files.error = Some(e),
                Err(_) => { app.files.dirs.remove(&path); }
            }
        });
    }

    /// Read the picked file and, when git says it changed, its diff vs HEAD.
    pub fn files_load_file(&mut self, keep_shown: bool) {
        let Some(file) = self.files.file.clone() else { return };
        if !keep_shown {
            self.files.content = None;
            self.files.diff = None;
        }
        let sp = self.files.space.clone();
        let f = file.clone();
        self.rpc_reply("files.read", space_params(self, json!({"path": file})), move |app, r| {
            if app.files.space != sp || app.files.file.as_ref() != Some(&f) { return; }
            app.files.editable = r.as_ref().map(|v| v["editable"].as_bool().unwrap_or(true)).unwrap_or(false);
            app.files.content = Some(r.and_then(|v| serde_json::from_value::<FileContent>(v).map_err(|e| e.to_string())).map(|c| {
                let lines = Arc::new(c.text.as_deref().unwrap_or("").split('\n').map(|l| l.replace('\t', "    ")).collect());
                (c, lines)
            }));
        });
        let changed = self.files.status.get(&file).is_some_and(|s| CHANGED.contains(&s.as_str()));
        let cwd = self.files.cwd.clone();
        if changed {
            let (sp, f) = (self.files.space.clone(), file.clone());
            self.rpc_reply("worktree.diff", json!({"path": cwd, "base": "HEAD", "file": file}), move |app, r| {
                if app.files.space != sp || app.files.file.as_ref() != Some(&f) { return; }
                app.files.raw_diff = r.as_ref().ok().and_then(|v| v["diff"].as_str().map(String::from));
                app.files.diff = Some(r.map(|v| Arc::new(changes::parse_diff(v["diff"].as_str().unwrap_or("")))));
            });
        } else {
            self.files.raw_diff = None;
            self.files.diff = None;
            self.files.show_diff = false;
        }
    }

    pub fn files_pick(&mut self, path: String, dir: bool, cx: &mut Context<Self>) {
        if dir {
            if !self.files.open.remove(&path) {
                self.files.open.insert(path.clone());
                if !self.files.dirs.contains_key(&path) {
                    self.files_load_dir(&path);
                }
            }
        } else {
            self.files_show(path); // opens it as a tab (docs.rs)
        }
        cx.notify();
    }

    /// Make an empty file (files.write with create), open the folders down to it and show it.
    pub fn files_create(&mut self, path: String) {
        let sp = self.files.space.clone();
        self.rpc_reply("files.write", space_params(self, json!({"path": path, "text": "", "create": true})), move |app, r| {
            if app.files.space != sp { return; }
            match r {
                Ok(_) => {
                    app.files_reveal(&path);
                    app.files_poll();
                    app.files_show(path.clone());
                }
                Err(e) => app.toasts.error(e),
            }
        });
    }

    /// Open every folder above `path` (loading the ones not read yet) and leave the Changed list for the full tree.
    pub fn files_reveal(&mut self, path: &str) {
        self.files.only_changed = false;
        let mut chain = String::new();
        let parts: Vec<&str> = path.split('/').collect();
        for part in &parts[..parts.len().saturating_sub(1)] {
            if !chain.is_empty() { chain.push('/'); }
            chain.push_str(part);
            self.files.open.insert(chain.clone());
            if !self.files.dirs.contains_key(&chain) { self.files_load_dir(&chain); }
        }
    }

    /// The ↻ button: read the open folders, the status and the shown file again.
    pub fn files_reload(&mut self) {
        let dirs: Vec<String> = self.files.dirs.keys().cloned().collect();
        for d in dirs { self.files_load_dir(&d); }
        self.files_poll();
        self.files_load_file(true);
    }

    /// Back to the tree from the opened file (its tab stays open).
    pub fn files_close_file(&mut self) {
        self.docs.shown.remove(&self.files.space);
        self.files.file = None;
        self.files.content = None;
        self.files.diff = None;
        self.files.show_diff = false;
    }

    /// Poll git status; on a change reload the open folders and the shown file.
    fn files_poll(&mut self) {
        self.files.polled = Some(Instant::now());
        let sp = self.files.space.clone();
        self.rpc_reply("files.status", space_params(self, json!({})), move |app, r| {
            let Ok(v) = r else { return };
            if app.files.space != sp { return; }
            let status: HashMap<String, String> = serde_json::from_value(v["files"].clone()).unwrap_or_default();
            app.files.is_repo = v["is_repo"].as_bool().unwrap_or(false);
            if status != app.files.status {
                app.files.status = status;
                let dirs: Vec<String> = app.files.dirs.keys().cloned().collect();
                for d in dirs {
                    app.files_load_dir(&d);
                }
                app.files_load_file(true);
            }
        });
    }
}

/// Called from the app's drain loop: start on a new scope, poll every 3 s while the panel is open.
pub fn tick(app: &mut DroverApp) -> bool {
    if app.view != View::Terminals || app.panel() != Some(View::Files) {
        return false;
    }
    let Some(sc) = crate::views::panel::scope(app) else { return false };
    if sc.key != app.files.space {
        // Keep what the folder left showed; one seen before comes back as it was and is refreshed by the poll.
        let fresh = FilesState { space: sc.key.clone(), space_id: sc.space, pane: sc.pane, cwd: sc.cwd, ..Default::default() };
        let left = std::mem::replace(&mut app.files, fresh);
        if !left.space.is_empty() { app.files_cache.insert(left.space.clone(), left); }
        match app.files_cache.remove(&sc.key) {
            Some(mut back) => {
                back.pane = app.files.pane.take();
                app.files = back;
                // Replies that came while it was away were dropped: read the root (if it never arrived), the open
                // folders, the status and the shown file again.
                if !app.files.dirs.contains_key("") { app.files_load_dir(""); }
                app.files_reload();
            }
            None => { app.files_load_dir(""); app.files_poll(); }
        }
        return true;
    }
    if app.files.polled.is_none_or(|t| t.elapsed() > Duration::from_secs(3)) {
        app.files_poll();
    }
    false
}

/// The panel: the whole tree, or the opened file on its own ("← Files" goes back to the tree).
pub fn panel_body(app: &DroverApp, cx: &mut Context<DroverApp>) -> Div {
    if let Some(e) = &app.files.error {
        return ui::note(e.clone(), true);
    }
    if !app.files.dirs.contains_key("") {
        return ui::note("Loading files…", false);
    }
    let name = crate::widgets::short_path(&app.files.cwd);
    if crate::views::panel::is_wide(app, View::Files) {
        // As the web: the tree (320 px) and the open file, or a hint, side by side.
        let right = if app.files.file.is_some() { content::panel(app, cx) }
            else { div().flex().flex_1().min_w_0().min_h_0().child(ui::note("Pick a file on the left to read or edit it.", false)) };
        // Each part is its own rounded card with a gap between, the editor's border in the accent colour.
        let card = |d: Div| d.flex().flex_col().min_h_0().min_w_0().rounded(crate::ui_scale::px(10.)).overflow_hidden().border_1().bg(gpui::rgb(crate::theme::pane()));
        return div().flex().flex_row().flex_1().min_h_0().min_w_0().gap(crate::ui_scale::px(8.)).p(crate::ui_scale::px(8.)).pr(crate::ui_scale::px(0.))
            .child(card(div().w(crate::ui_scale::px(320.)).flex_none()).border_color(crate::theme::border()).child(tree::panel(app, &name, cx)))
            .child(card(div().flex_1()).border_color(gpui::rgb(crate::theme::accent())).child(right));
    }
    if app.files.file.is_some() {
        return content::panel(app, cx);
    }
    div().flex().flex_col().flex_1().min_h_0().child(tree::panel(app, &name, cx))
}
