//! Changes panel (port of `frontend/src/changes/*`), as Herdr's: the focused pane's worktree, files changed vs a
//! base, the selected file's diff, commit / rebase / push & PR; a Worktrees tab lists the repo's worktrees (pick,
//! new, remove, prune). Data is fetched again when the scope, the dirty count, the worktree, the base, the file or an
//! action (`reload`) changes.

mod main_panel;
mod review;
mod review_view;
mod worktrees;

pub use main_panel::diff_view;

use std::sync::Arc;

use drover_proto::changes::{self, ChangedFile, DiffRow, GitInfo, Worktree};
use drover_proto::wire::Space;
use gpui::{Context, Div, Entity, prelude::*};
use serde_json::{Value, json};

use crate::views::text_input::{InputEvent, TextInput};
use crate::views::ui;
use crate::workspace_app::DroverApp;

type Fetched<T> = Option<Result<T, String>>;

#[derive(Default)]
pub struct ChangesState {
    pub picked_path: Option<String>,
    pub picked_file: Option<String>,
    pub info: Fetched<GitInfo>,
    pub trees: Fetched<Vec<Worktree>>,
    pub files: Fetched<Vec<ChangedFile>>,
    pub diff: Fetched<Arc<Vec<DiffRow>>>,
    pub confirm_remove: bool,
    pub show_trees: bool,
    /// The last action's output (false) or error (true).
    pub output: Option<(String, bool)>,
    /// Review loop and checks of the shown worktree (review.rs).
    pub review: review::ReviewState,
    pub reload: u64,
    keys: [String; 3],
}

/// Fields of the view: the base to compare against, a commit message, a new task worktree's name.
pub struct ChangesInputs {
    pub base: Entity<TextInput>,
    pub commit: Entity<TextInput>,
    pub task: Entity<TextInput>,
    /// The review comment being written (review_view.rs), and the review diff's keyboard focus (F7, A).
    pub comment: Entity<TextInput>,
    pub review_focus: gpui::FocusHandle,
}

impl ChangesInputs {
    pub fn new(cx: &mut Context<DroverApp>) -> Self {
        let base = cx.new(|cx| TextInput::new("base branch", cx));
        let commit = cx.new(|cx| TextInput::new("Commit message", cx));
        let task = cx.new(|cx| TextInput::new("New task worktree", cx));
        let comment = cx.new(TextInput::new_multiline);
        cx.subscribe(&comment, |app, _, ev, cx| if let InputEvent::Submit = ev { app.review_submit_draft(cx) }).detach();
        cx.subscribe(&base, |_, _, ev, cx| if let InputEvent::Submit = ev { cx.notify() }).detach();
        cx.subscribe(&commit, |app, _, ev, cx| if let InputEvent::Submit = ev { app.changes_commit(cx) }).detach();
        cx.subscribe(&task, |app, _, ev, cx| if let InputEvent::Submit = ev { app.changes_new_worktree(cx) }).detach();
        ChangesInputs { base, commit, task, comment, review_focus: cx.focus_handle() }
    }
}

pub fn space(app: &DroverApp) -> Option<&Space> {
    app.space.as_deref().and_then(|id| app.store.spaces.iter().find(|s| s.id == id))
}

/// RPC params with the space's machine, as the web UI sends them for a remote space.
pub fn with_machine(app: &DroverApp, mut params: Value) -> Value {
    if let Some(m) = space(app).and_then(|s| s.machine.clone()).filter(|m| !m.is_empty() && m != "local") {
        params["machine"] = json!(m);
    }
    params
}

/// The shown worktree, base and file, from what was fetched and picked.
pub fn shown(app: &DroverApp, cx: &gpui::App) -> (Option<String>, String, Option<String>) {
    let trees: &[Worktree] = match &app.changes.trees { Some(Ok(t)) => t, _ => &[] };
    // The focused pane's worktree (set from git.info), or one picked in the Worktrees tab.
    let path = app.changes.picked_path.clone().or_else(|| changes::shown_path(None, trees));
    let base = changes::base(&app.changes_in.base.read(cx).text, trees);
    // Only a file the user opened (the list shows until then), and only while it still has changes.
    let file = match &app.changes.files {
        Some(Ok(files)) => app.changes.picked_file.clone().filter(|f| files.iter().any(|x| &x.path == f)),
        _ => None,
    };
    (path, base, file)
}

/// Fetch what changed: repo + worktrees, then the changed files, then the shown file's diff.
pub fn refresh(app: &mut DroverApp, cx: &gpui::App) {
    let Some(sc) = crate::views::panel::scope(app) else { return };
    let dirty = space(app).and_then(|s| s.dirty).unwrap_or(0);
    let tick = format!("{dirty}|{}", app.changes.reload);
    let repo_key = format!("{}|{tick}", sc.key);
    if repo_key != app.changes.keys[0] {
        if !app.changes.keys[0].starts_with(&format!("{}|", sc.key)) {
            // Another pane or folder: nothing of the old one, show its own worktree.
            app.changes = ChangesState { reload: app.changes.reload, show_trees: app.changes.show_trees, ..Default::default() };
        }
        app.changes.keys[0] = repo_key.clone();
        let (k1, k2) = (repo_key.clone(), repo_key);
        app.rpc_reply("git.info", with_machine(app, json!({"path": sc.cwd})), move |app, r| {
            if app.changes.keys[0] != k1 { return; }
            let info: Result<GitInfo, String> = r.and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()));
            if let Some(i) = info.as_ref().ok().filter(|i| i.is_repo && app.changes.picked_path.is_none()) {
                app.changes.picked_path = Some(i.root.clone());
            }
            app.changes.info = Some(info);
        });
        app.rpc_reply("worktree.list", with_machine(app, json!({"repo": sc.cwd})), move |app, r| {
            if app.changes.keys[0] == k2 { app.changes.trees = Some(r.and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))); }
        });
    }
    review::refresh(app, &tick, cx);
    let (path, base, file) = shown(app, cx);
    let Some(path) = path else { return };
    let files_key = format!("{path}|{base}|{tick}");
    if files_key != app.changes.keys[1] {
        app.changes.keys[1] = files_key.clone();
        app.rpc_reply("worktree.changes", with_machine(app, json!({"path": path, "base": base})), move |app, r| {
            // As the web UI: a failed listing shows as no changes.
            if app.changes.keys[1] == files_key { app.changes.files = Some(Ok(r.ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default())); }
        });
    }
    let Some(file) = file else { return };
    let diff_key = format!("{path}|{base}|{file}|{tick}");
    if diff_key != app.changes.keys[2] {
        app.changes.keys[2] = diff_key.clone();
        app.changes.diff = None;
        app.rpc_reply("worktree.diff", with_machine(app, json!({"path": path, "base": base, "file": file})), move |app, r| {
            if app.changes.keys[2] == diff_key {
                app.changes.diff = Some(r.map(|v| Arc::new(changes::parse_diff(v["diff"].as_str().unwrap_or("")))));
            }
        });
    }
}

impl DroverApp {
    fn changes_done(&mut self, label: &str, r: Result<Value, String>) {
        self.changes.output = Some(match r {
            Ok(v) => (format!("{label}: {}", v["output"].as_str().unwrap_or("done").trim()), false),
            Err(e) => (format!("{label}: {e}"), true),
        });
        self.changes.reload += 1;
    }

    /// Run a git action on the shown worktree and show its output.
    pub fn changes_run(&mut self, label: &'static str, method: &str, params: Value) {
        let params = with_machine(self, params);
        self.rpc_reply(method, params, move |app, r| app.changes_done(label, r));
    }

    pub fn changes_commit(&mut self, cx: &mut Context<Self>) {
        let message = self.changes_in.commit.read(cx).text.trim().to_string();
        let (Some(path), _, _) = shown(self, cx) else { return };
        if message.is_empty() {
            self.changes.output = Some(("Type a commit message first.".into(), true));
        } else {
            self.changes_run("Commit", "git.commit", json!({"path": path, "message": message}));
            self.changes_in.commit.update(cx, |i, cx| i.set_text("", cx));
        }
        cx.notify();
    }

    pub fn changes_push_pr(&mut self, cx: &mut Context<Self>) {
        let (Some(path), _, _) = shown(self, cx) else { return };
        let pr = with_machine(self, json!({"path": path}));
        self.rpc_reply("git.push", with_machine(self, json!({"path": path})), move |app, r| match r {
            Ok(_) => app.rpc_reply("git.pr", pr, |app, r| app.changes_done("Push & open PR", r)),
            Err(e) => app.changes_done("Push", Err(e)),
        });
    }

    pub fn changes_new_worktree(&mut self, cx: &mut Context<Self>) {
        let task = self.changes_in.task.read(cx).text.trim().to_string();
        let Some(Ok(info)) = self.changes.info.clone() else { return };
        if task.is_empty() {
            return;
        }
        let (_, base, _) = shown(self, cx);
        self.changes_in.task.update(cx, |i, cx| i.set_text("", cx));
        let params = with_machine(self, json!({"repo": info.root, "task": task, "base": base}));
        self.rpc_reply("worktree.create", params, |app, r| {
            if let Ok(v) = &r {
                app.changes.picked_path = v["path"].as_str().map(String::from);
            }
            let r = r.map(|v| json!({"output": format!("created {}", v["branch"].as_str().unwrap_or("?"))}));
            app.changes_done("New task worktree", r);
        });
    }
}

/// The panel: Changes of the shown worktree, or the Worktrees tab.
pub fn panel_body(app: &DroverApp, cx: &mut Context<DroverApp>) -> Div {
    let cwd = crate::views::panel::scope(app).map(|s| s.cwd).unwrap_or_default();
    let content = match (&app.changes.info, &app.changes.trees) {
        (Some(Err(e)), _) => ui::note(e.clone(), true),
        (Some(Ok(info)), _) if !info.is_repo => ui::note(format!("{cwd} is not a git repository."), false),
        (Some(Ok(_)), Some(Err(e))) => ui::note(e.clone(), true),
        (Some(Ok(_)), Some(Ok(trees))) => {
            let (path, base, file) = shown(app, cx);
            let current = || trees.iter().find(|t| Some(&t.path) == path.as_ref()).cloned()
                .or_else(|| path.clone().map(|p| Worktree { path: p, ..Default::default() }));
            if crate::views::panel::is_wide(app, crate::views::View::Changes) {
                // As the web: the worktrees (300 px) beside the branch, checks, files and diff.
                return gpui::div().flex().flex_row().flex_1().min_h_0().min_w_0()
                    .child(gpui::div().w(crate::ui_scale::px(300.)).flex_none().flex().flex_col().min_h_0()
                        .border_r_1().border_color(crate::theme::border()).child(worktrees::panel(app, trees, path.as_deref(), cx)))
                    .child(gpui::div().flex().flex_col().flex_1().min_w_0().min_h_0().child(main_panel::panel(app, current().as_ref(), &base, file.as_deref(), cx)));
            }
            if app.changes.show_trees {
                worktrees::panel(app, trees, path.as_deref(), cx)
            } else {
                let current = trees.iter().find(|t| Some(&t.path) == path.as_ref()).cloned()
                    .or_else(|| path.clone().map(|p| Worktree { path: p, ..Default::default() }));
                main_panel::panel(app, current.as_ref(), &base, file.as_deref(), cx)
            }
        }
        _ => ui::note("Loading git info…", false),
    };
    let tab = |label: &str, trees: bool, cx: &mut Context<DroverApp>| ui::tab(label, app.changes.show_trees == trees)
        .on_mouse_down(gpui::MouseButton::Left, cx.listener(move |app, _, _, cx| { app.changes.show_trees = trees; cx.notify(); }));
    gpui::div().flex().flex_col().flex_1().min_h_0()
        .child(gpui::div().h(crate::ui_scale::px(36.)).flex_none().flex().items_center().gap(crate::ui_scale::px(4.)).px(crate::ui_scale::px(10.))
            .border_b_1().border_color(crate::theme::border())
            .child(tab("Changes", false, cx)).child(tab("Worktrees", true, cx)))
        .child(content)
}
