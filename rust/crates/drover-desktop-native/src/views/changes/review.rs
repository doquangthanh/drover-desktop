//! Changes panel, the review loop and post-run checks (the web UI's review-*.tsx and checks-strip.tsx), data side:
//! `review.summary` / `review.diff` / `review.list` of the shown worktree and file, `check.list` of the worktree
//! (polled while a run is going), and the actions (comment, resolve, mark a hunk reviewed, reject it, send the
//! comments to the agent, run / stop the checks). Every action reloads the panel. Drawing is in review_view.rs.

use std::time::{Duration, Instant};

use gpui::Context;
use serde_json::{Value, json};

use super::{shown, with_machine};
use crate::workspace_app::DroverApp;

#[derive(Default)]
pub struct ReviewState {
    keys: [String; 3],
    /// review.diff of the shown file and its comments; None until loaded.
    pub diff: Option<Result<Value, String>>,
    pub comments: Vec<Value>,
    /// The daemon has no review.* (an older one): the plain diff is shown.
    pub unsupported: bool,
    pub summary: Option<Value>,
    pub checks: Option<Value>,
    polled: Option<Instant>,
    /// The check step whose output is shown.
    pub check_open: Option<String>,
    /// Hunk the keyboard works on (F7 / Shift+F7, A).
    pub cur: usize,
    /// The comment being written: side, line and the comment it edits.
    pub draft: Option<(String, u64, Option<String>)>,
    /// "Reject hunk" was clicked once: the hunk index waiting for the second click.
    pub reject_ask: Option<u64>,
}

pub fn busy(run: &Value) -> bool { matches!(run["status"].as_str(), Some("queued" | "running")) }

/// Fetch what changed since the last frame (called from changes::refresh).
pub fn refresh(app: &mut DroverApp, tick: &str, cx: &gpui::App) {
    let (Some(path), base, file) = shown(app, cx) else { return };
    let k0 = format!("{path}|{base}|{tick}");
    if app.changes.review.keys[0] != k0 {
        app.changes.review.keys[0] = k0.clone();
        let k = k0.clone();
        app.rpc_reply("review.summary", with_machine(app, json!({"cwd": path, "base": base})), move |app, r| {
            if app.changes.review.keys[0] != k { return }
            app.changes.review.unsupported = r.as_ref().err().is_some_and(|e| e.to_lowercase().contains("unknown method"));
            app.changes.review.summary = r.ok();
        });
    }
    let running = app.changes.review.checks.as_ref().is_some_and(busy);
    let k1 = format!("{path}|{tick}");
    if app.changes.review.keys[1] != k1 || (running && app.changes.review.polled.is_none_or(|t| t.elapsed() > Duration::from_millis(1500))) {
        app.changes.review.keys[1] = k1.clone();
        app.changes.review.polled = Some(Instant::now());
        app.rpc_reply("check.list", with_machine(app, json!({"cwd": path})), move |app, r| {
            if app.changes.review.keys[1] == k1 { app.changes.review.checks = r.ok(); }
        });
    }
    let Some(file) = file else { return };
    let k2 = format!("{path}|{base}|{file}|{tick}");
    if app.changes.review.keys[2] != k2 && !app.changes.review.unsupported {
        if !app.changes.review.keys[2].starts_with(&format!("{path}|{base}|{file}|")) {
            (app.changes.review.diff, app.changes.review.cur, app.changes.review.draft) = (None, 0, None);
        }
        app.changes.review.keys[2] = k2.clone();
        let k = k2.clone();
        app.rpc_reply("review.diff", with_machine(app, json!({"cwd": path, "base": base, "file": file})), move |app, r| {
            if app.changes.review.keys[2] != k { return }
            if r.as_ref().err().is_some_and(|e| e.to_lowercase().contains("unknown method")) { app.changes.review.unsupported = true; }
            app.changes.review.diff = Some(r);
        });
        app.rpc_reply("review.list", with_machine(app, json!({"cwd": path, "file": file})), move |app, r| {
            if app.changes.review.keys[2] != k2 { return }
            app.changes.review.comments = r.ok().and_then(|v| v["comments"].as_array().cloned()).unwrap_or_default();
        });
    }
}

impl DroverApp {
    /// Run a review / check method on the shown worktree (and file), then reload. `extra` adds to cwd/base/file.
    pub fn review_call(&mut self, method: &str, extra: Value, cx: &gpui::App) {
        let (Some(path), base, file) = shown(self, cx) else { return };
        let mut p = json!({"cwd": path, "base": base});
        if let Some(f) = file { p["file"] = json!(f); }
        if let (Some(o), Some(e)) = (p.as_object_mut(), extra.as_object()) { o.extend(e.clone()); }
        let label = method.to_string();
        self.rpc_reply(method, with_machine(self, p), move |app, r| {
            match r {
                Ok(v) if label == "review.hunk.reject" => if let Some(u) = v["undo_ref"].as_str() {
                    app.toasts.push("Hunk rejected".into(), format!("Get it back: git checkout {u} -- <file>"), String::new(), crate::toasts::Tone::Done, "", crate::toasts::Target::Nothing);
                },
                Ok(v) if label == "review.send" => {
                    let n = v["sent"].as_u64().unwrap_or(0);
                    app.toasts.push(format!("Sent {n} comment{}", if n == 1 { "" } else { "s" }), format!("to {}", v["pane"].as_str().unwrap_or("the agent")), String::new(), crate::toasts::Tone::Done, "", crate::toasts::Target::Nothing);
                }
                Ok(_) => {}
                // Several agents work in this folder: send to the focused one.
                Err(e) if label == "review.send" && e.contains("say which one") => match app.focused.clone() {
                    Some(pane) => return app.review_send_to(pane),
                    None => app.toasts.error(e),
                },
                Err(e) => app.toasts.error(format!("{label}: {e}")),
            }
            app.changes.reload += 1;
        });
    }

    fn review_send_to(&mut self, pane: String) {
        let Some(path) = self.changes.picked_path.clone() else { return };
        self.rpc_reply("review.send", with_machine(self, json!({"cwd": path, "pane": pane})), |app, r| {
            match r {
                Ok(v) => app.toasts.push(format!("Sent {} comment(s)", v["sent"].as_u64().unwrap_or(0)), String::new(), String::new(), crate::toasts::Tone::Done, "", crate::toasts::Target::Nothing),
                Err(e) => app.toasts.error(e),
            }
            app.changes.reload += 1;
        });
    }

    /// Add the drafted comment (or save the edited one) from the comment field.
    pub fn review_submit_draft(&mut self, cx: &mut Context<Self>) {
        let text = self.changes_in.comment.read(cx).text.trim().to_string();
        let Some((side, line, edit)) = self.changes.review.draft.take() else { return };
        if text.is_empty() { self.changes.review.draft = Some((side, line, edit)); return }
        match edit {
            Some(id) => self.review_call("review.comment.edit", json!({"id": id, "text": text}), cx),
            None => self.review_call("review.comment.add", json!({"side": side, "line": line, "text": text}), cx),
        }
        self.changes_in.comment.update(cx, |i, cx| i.set_text("", cx));
        cx.notify();
    }

    /// F7 / Shift+F7: next / previous hunk; A: mark the current one reviewed and go on.
    pub fn review_key(&mut self, key: &str, shift: bool, cx: &mut Context<Self>) -> bool {
        let Some(Ok(diff)) = &self.changes.review.diff else { return false };
        let hunks = diff["hunks"].as_array().cloned().unwrap_or_default();
        if hunks.is_empty() { return false }
        let cur = self.changes.review.cur.min(hunks.len() - 1);
        match (key, shift) {
            ("f7", false) => self.changes.review.cur = (cur + 1).min(hunks.len() - 1),
            ("f7", true) => self.changes.review.cur = cur.saturating_sub(1),
            ("a", false) => {
                if !hunks[cur]["reviewed"].as_bool().unwrap_or(false) {
                    self.review_call("review.hunk.accept", json!({"index": hunks[cur]["index"], "accepted": true}), cx);
                }
                self.changes.review.cur = (cur + 1).min(hunks.len() - 1);
            }
            _ => return false,
        }
        cx.notify();
        true
    }
}
