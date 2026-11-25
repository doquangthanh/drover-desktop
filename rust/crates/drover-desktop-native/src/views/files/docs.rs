//! The Files panel's open documents: one code editor per open tab, kept across focus changes (the panel's state is
//! reset when the focused pane changes; unsaved edits are not). Loading, saving (files.write, refusing to overwrite a
//! file an agent changed unless asked), reloading a clean tab when the file changes on disk, and opening a file at a
//! line (Ctrl+Click in a terminal, quickfix).
//!
//! RPC replies only get the app, not a GPUI context, so what they learn is queued here and applied in `sync`, which
//! the window calls on every render.

use std::collections::HashMap;

use gpui::{AppContext, Context, Entity, Window};
use serde_json::json;

use crate::code_editor::{CodeEditor, EditorEvent, marks};
use crate::workspace_app::DroverApp;

pub struct OpenDoc {
    pub editor: Entity<CodeEditor>,
    /// mtime of the text the editor started from or last saved.
    pub mtime: i64,
    /// The file changed on disk while the tab had edits: shown as a banner (Reload / keep mine).
    pub stale: bool,
    /// The last save was refused because the file changed on disk: Overwrite or Reload.
    pub conflict: bool,
    /// The diff the gutter marks were made from (so they are only parsed again when it changes).
    marks_from: Option<String>,
    /// Saved text whose reply came back (applied in sync).
    saved: Option<String>,
    goto: Option<(usize, Option<usize>)>,
}

#[derive(Default)]
pub struct Docs {
    /// By "scope key|path" (views/panel.rs Scope::key): a path means the same file only inside the same folder.
    pub open: HashMap<String, OpenDoc>,
    /// The tabs and the shown file of each scope.
    pub tabs: HashMap<String, Vec<String>>,
    pub shown: HashMap<String, String>,
    /// A file to open when its scope is next shown: (scope, path, line, col).
    pub want: Option<(String, String, Option<usize>, Option<usize>)>,
    /// A goto for a file whose editor is made once its text is read: (doc key, line, col).
    pending_goto: Option<(String, usize, Option<usize>)>,
    /// A tab with edits whose × was clicked: the banner asks before it goes.
    pub closing: Option<String>,
    /// Ctrl+J: the editor takes the width, the agent's terminal stays as a narrow column beside it.
    pub wide: bool,
    /// The agent beside the editor: shown (the web's default) or folded to its "Agent ▸" tab, and which pane it shows
    /// (None: the focused agent, else the first one).
    pub dock: bool,
    pub dock_pane: Option<String>,
}

impl Docs {
    /// `wide` starts on: Files opens as the web's tree | editor | agent (Ctrl+J narrows it).
    pub fn new() -> Self { Docs { wide: true, dock: true, ..Default::default() } }
}

pub fn key(scope: &str, path: &str) -> String { format!("{scope}|{path}") }

impl DroverApp {
    /// The open document of the shown file, if it is one.
    pub fn shown_doc(&self) -> Option<&OpenDoc> {
        let file = self.files.file.as_ref()?;
        self.docs.open.get(&key(&self.files.space, file))
    }

    /// Show `path` (relative to the panel's folder) in the Files panel of the focused pane, at `line`.
    pub fn open_in_files(&mut self, path: &str, line: Option<usize>, col: Option<usize>, cx: &mut Context<Self>) {
        self.set_panel(Some(crate::views::View::Files));
        self.view = crate::views::View::Terminals;
        let Some(sc) = crate::views::panel::scope(self) else { return };
        self.docs.want = Some((sc.key, path.to_string(), line, col));
        cx.notify();
    }

    /// Ctrl+S: write the tab's text; `force` overwrites a file that changed on disk since it was read.
    pub fn files_save(&mut self, path: &str, force: bool, cx: &mut Context<Self>) {
        let k = key(&self.files.space, path);
        let Some(doc) = self.docs.open.get(&k) else { return };
        let ed = doc.editor.read(cx);
        if !ed.editable { return self.toasts.error(format!("{path} is read-only here")) }
        let text = ed.buf.text.clone();
        let params = super::space_params(self, json!({"path": path, "text": text, "base_mtime_us": doc.mtime, "force": force}));
        let p = path.to_string();
        self.rpc_reply("files.write", params, move |app, r| {
            let Some(doc) = app.docs.open.get_mut(&k) else { return };
            match r {
                Ok(v) => {
                    doc.mtime = v["mtime_us"].as_i64().unwrap_or(doc.mtime);
                    (doc.stale, doc.conflict, doc.saved) = (false, false, Some(text));
                    app.toasts.push("Saved".into(), p, String::new(), crate::toasts::Tone::Done, "", crate::toasts::Target::Nothing);
                    app.files_load_file(true); // new git marks
                }
                Err(e) if e.contains("changed on disk") => doc.conflict = true,
                Err(e) => app.toasts.error(format!("Could not save {p}: {e}")),
            }
        });
    }

    /// Drop local edits and show what is on disk (the stale / conflict banners' Reload).
    pub fn files_reload_doc(&mut self, path: &str, cx: &mut Context<Self>) {
        if let Some(doc) = self.docs.open.remove(&key(&self.files.space, path)) { drop(doc); }
        self.files_load_file(true);
        cx.notify();
    }

    /// Close a tab (the editor and its edits go too; the caller asked first when it had edits).
    pub fn files_close_tab(&mut self, path: &str, cx: &mut Context<Self>) {
        let scope = self.files.space.clone();
        self.docs.open.remove(&key(&scope, path));
        let tabs = self.docs.tabs.entry(scope.clone()).or_default();
        let at = tabs.iter().position(|t| t == path);
        tabs.retain(|t| t != path);
        if self.files.file.as_deref() == Some(path) {
            match at.and_then(|i| tabs.get(i.min(tabs.len().saturating_sub(1))).cloned()) {
                Some(next) => self.files_show(next),
                None => { self.docs.shown.remove(&scope); self.files_close_file(); }
            }
        }
        cx.notify();
    }

    /// Make `path` the shown file (a tab click, a tree click, an open request).
    pub fn files_show(&mut self, path: String) {
        let scope = self.files.space.clone();
        let tabs = self.docs.tabs.entry(scope.clone()).or_default();
        if !tabs.contains(&path) { tabs.push(path.clone()); }
        self.docs.shown.insert(scope, path.clone());
        if self.files.file.as_ref() != Some(&path) {
            self.files.file = Some(path);
            self.files.show_diff = false;
            self.files_load_file(false);
        }
    }

    /// Find box of the open file: select the next (or previous) match of what was typed; says when there is none.
    pub fn files_find(&mut self, forward: bool, restart: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(query) = self.picker.as_ref().map(|p| p.query.clone()) else { return };
        let Some(editor) = self.shown_doc().map(|d| d.editor.clone()) else { return };
        let found = !query.is_empty() && editor.update(cx, |e, _| e.find_next(&query, forward, restart));
        if let Some(p) = self.picker.as_mut() { p.note = (!query.is_empty() && !found).then(|| "no match".to_string()); }
        let _ = window;
        cx.notify();
    }

    /// Go to line: put the caret on the number typed, back to the editor.
    pub fn files_goto_line(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let n = self.picker.as_ref().and_then(|p| p.query.trim().parse::<usize>().ok());
        self.close_picker(cx);
        if let (Some(n), Some(editor)) = (n, self.shown_doc().map(|d| d.editor.clone())) {
            editor.update(cx, |e, _| e.goto(n.max(1), None));
        }
        self.want_term_focus = false;
        if let Some(editor) = self.shown_doc().map(|d| d.editor.clone()) { window.focus(&gpui::Focusable::focus_handle(editor.read(cx), cx)); }
    }

    /// Alt+S: paste the selection (path:lines and the code) into an agent of this space, without Enter.
    pub fn files_send_selection(&mut self, cx: &mut Context<Self>) {
        let Some(doc) = self.shown_doc() else { return self.toasts.error("Open a file in the editor first: nothing to send.".into()) };
        let (a, b, text) = doc.editor.read(cx).selection_lines();
        let path = self.files.file.clone().unwrap_or_default();
        let space = self.files.space_id.clone();
        let agents: Vec<&drover_proto::wire::Pane> = self.store.panes.iter()
            .filter(|p| p.space_id == space && p.kind != "shell" && p.state != "exited").collect();
        // The agent shown beside the editor first, else the focused one, else the first.
        let docked = self.docs.dock_pane.as_ref();
        let pane = agents.iter().find(|p| Some(&p.id) == docked).or_else(|| agents.iter().find(|p| Some(&p.id) == self.focused.as_ref()))
            .or(agents.first()).map(|p| (p.id.clone(), p.name.clone()));
        let Some((pane, name)) = pane else { return self.toasts.error("No agent in this space: run claude, codex… in a terminal first.".into()) };
        let data = marks::as_paste(&marks::format_selection(&path, a, b, &text));
        let _ = self.client.commands.send(drover_proto::client::Command::Input { pane, data });
        let at = if b > a { format!("{path}:{a}-{b}") } else { format!("{path}:{a}") };
        self.toasts.push(format!("Sent to {name}"), at, String::new(), crate::toasts::Tone::Done, "", crate::toasts::Target::Nothing);
    }
}

/// Called on every render: open a wanted file, make the shown file's editor, follow the disk, apply queued replies.
pub fn sync(app: &mut DroverApp, window: &mut Window, cx: &mut Context<DroverApp>) {
    let scope = app.files.space.clone();
    if scope.is_empty() { return }
    match app.docs.want.take() {
        Some(w) if w.0 != scope => app.docs.want = Some(w), // for a folder not shown yet
        Some((_, path, line, col)) => {
            app.files_show(path.clone());
            let k = key(&scope, &path);
            match (app.docs.open.get_mut(&k), line) {
                (Some(doc), Some(l)) => doc.goto = Some((l, col)),
                (None, Some(l)) => app.docs.pending_goto = Some((k, l, col)), // once its text is read
                _ => {}
            }
        }
        None => {}
    }
    if app.files.file.is_none() {
        // A fresh panel state (focus changed): show the tab it had.
        if let Some(path) = app.docs.shown.get(&scope).cloned() { app.files_show(path); }
        return;
    }
    let Some(file) = app.files.file.clone() else { return };
    let k = key(&scope, &file);
    let loaded = match &app.files.content { Some(Ok((c, _))) if c.error.is_none() && !c.binary => Some(c.clone()), _ => None };
    let family = app.code_family.clone();
    let diff = app.files.raw_diff.clone();
    if let (Some(c), false) = (&loaded, app.docs.open.contains_key(&k)) {
        let (text, editable) = (c.text.clone().unwrap_or_default(), app.files.editable);
        let editor = cx.new(|cx| CodeEditor::new(&file, &text, editable, cx));
        let p = file.clone();
        cx.subscribe_in(&editor, window, move |app, _, ev, window, cx| match ev {
            EditorEvent::Save => app.files_save(&p, false, cx),
            EditorEvent::Send => app.files_send_selection(cx),
            EditorEvent::ToggleDock => { app.docs.wide = !app.docs.wide; cx.notify(); }
            EditorEvent::Changed => cx.notify(),
            EditorEvent::Find => {
                let word = app.shown_doc().map(|d| d.editor.read(cx).selected_word()).unwrap_or_default();
                app.open_picker_with(crate::picker::Kind::FileFind, ("Find in this file (Enter: next)", word), None, window, cx);
            }
            EditorEvent::GotoLine => app.open_picker_with(crate::picker::Kind::GotoLine, ("Go to line…", String::new()), None, window, cx),
        }).detach();
        let goto = app.docs.pending_goto.take().filter(|(kk, _, _)| *kk == k).map(|(_, l, c)| (l, c));
        app.docs.open.insert(k.clone(), OpenDoc { editor, mtime: c.mtime_us, stale: false, conflict: false, marks_from: None, saved: None, goto });
    }
    let Some(doc) = app.docs.open.get_mut(&k) else { return };
    let editable = app.files.editable;
    doc.editor.update(cx, |ed, _| {
        ed.family = family;
        if let Some(text) = doc.saved.take() { ed.mark_saved(&text); }
        if let Some(c) = loaded.as_ref().filter(|c| c.mtime_us != doc.mtime) {
            // On disk it changed (an agent?): follow it when there are no local edits, else say so.
            if ed.dirty() { doc.stale = true; } else { ed.reload(c.text.as_deref().unwrap_or(""), editable); doc.mtime = c.mtime_us; }
        }
        if doc.marks_from != diff {
            ed.marks = diff.as_deref().map(marks::diff_marks).unwrap_or_default();
            doc.marks_from = diff.clone();
        }
        if let Some((line, col)) = doc.goto.take() { ed.goto(line, col); }
    });
}
