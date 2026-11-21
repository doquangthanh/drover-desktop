//! What the buttons do: the same RPCs as the web UI's actions (`frontend/src/actions/*.ts`).

use std::sync::mpsc::{Receiver, channel};

use drover_proto::client::Command;
use gpui::{Context, Window};
use serde_json::{Value, json};

use crate::workspace_app::DroverApp;

/// DROVER_NATIVE_TRACE=1: log what the buttons do.
pub fn trace(msg: &str) {
    if std::env::var_os("DROVER_NATIVE_TRACE").is_some() {
        eprintln!("trace: {msg}");
    }
}

/// A question to answer before something destructive happens (the web UI's `confirmBox`).
pub struct Confirm {
    pub title: String,
    pub detail: String,
    pub ok: &'static str,
    pub then: Then,
}

pub enum Then {
    ClosePane(String),
    CloseTab(String),
    CloseSpace(String),
    StopSubtree(String),
    RemoveMachine(String),
    UpdateApply(String),
    StopServer,
}

/// What to do with an rpc reply when it arrives (checked by the drain loop).
pub type OnReply = Box<dyn FnOnce(&mut DroverApp, Result<Value, String>)>;
pub type Pending = (Receiver<Result<Value, String>>, OnReply);

/// Views the native app does not have yet: opened in the browser through the web UI (`#view` in main.tsx).
pub const WEB_VIEWS: [&str; 7] = ["inbox", "changes", "files", "machines", "settings", "server", "tree"];

impl DroverApp {
    pub fn rpc(&self, method: &str, params: Value) {
        trace(&format!("rpc {method} {params}"));
        // Fire and forget, except that a failure is shown as a toast (take_replies).
        let (tx, rx) = channel();
        let _ = self.client.commands.send(Command::Rpc { method: method.into(), params, reply: Some(tx) });
        self.fired.lock().unwrap().push(rx);
    }

    fn rpc_then(&mut self, method: &str, params: Value, then: fn(&mut DroverApp, Value)) {
        self.rpc_reply(method, params, move |app, r| match r {
            Ok(v) => then(app, v),
            Err(e) => eprintln!("drover: {e}"),
        });
    }

    /// Send an rpc and hand its reply, value or error, to `then` (views show the error themselves).
    pub fn rpc_reply(&mut self, method: &str, params: Value, then: impl FnOnce(&mut DroverApp, Result<Value, String>) + 'static) {
        trace(&format!("rpc {method} {params} (waits for the reply)"));
        let (tx, rx) = channel();
        let _ = self.client.commands.send(Command::Rpc { method: method.into(), params, reply: Some(tx) });
        self.pending.push((rx, Box::new(then)));
    }

    /// GET `url` with the system `curl` (like the daemon does), off the UI thread; `then` gets the text or the error.
    pub fn http_get(&mut self, url: &str, then: impl FnOnce(&mut DroverApp, Result<String, String>) + 'static) {
        let (tx, rx) = channel();
        let url = url.to_string();
        std::thread::spawn(move || { let _ = tx.send(curl_text(&url).map(Value::String)); });
        self.pending.push((rx, Box::new(move |app, r| then(app, r.and_then(|v| v.as_str().map(String::from).ok_or_else(|| "bad reply".to_string()))))));
    }

    /// Run the callbacks of replies that arrived. True when one ran.
    pub fn take_replies(&mut self) -> bool {
        let mut ready = vec![];
        for (rx, then) in std::mem::take(&mut self.pending) {
            match rx.try_recv() {
                Ok(r) => {
                    if std::env::var_os("DROVER_NATIVE_TRACE").is_some() {
                        trace(&format!("reply {r:?}"));
                    }
                    ready.push((then, r));
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => self.pending.push((rx, then)),
                Err(_) => ready.push((then, Err("drover-server went away".into()))),
            }
        }
        let errors: Vec<String> = {
            let mut fired = self.fired.lock().unwrap();
            let mut errors = vec![];
            fired.retain(|rx| match rx.try_recv() {
                Ok(r) => { errors.extend(r.err()); false }
                Err(std::sync::mpsc::TryRecvError::Empty) => true,
                Err(_) => false,
            });
            errors
        };
        let ran = !ready.is_empty() || !errors.is_empty();
        for e in errors {
            self.toasts.error(e);
        }
        for (then, r) in ready {
            then(self, r);
        }
        ran
    }

    /// Show a pane: its space and tab, keys go to it, the daemon marks it read.
    pub fn show_pane(&mut self, pane: &str) {
        let Some(p) = self.store.pane(pane) else { return };
        self.view = crate::views::View::Terminals;
        self.want_term_focus = true;
        self.space = Some(p.space_id.clone());
        self.tab = Some(p.tab_id.clone());
        self.focused = Some(pane.to_string());
        self.sizes.lock().unwrap().remove(pane);
        self.rpc("pane.focus", json!({"pane": pane}));
    }

    fn focus_created(app: &mut DroverApp, v: Value) {
        // The state event with the new pane may come after the reply: remember it, the drain loop shows it.
        let id = v["id"].as_str().or_else(|| v["pane"]["id"].as_str()).unwrap_or("").to_string();
        app.show_when_known = Some(id);
    }

    /// A shell in a new tab, in `cwd` (the Changes view's "Terminal here"); it is shown once it exists.
    pub fn open_shell_in(&mut self, cwd: &str) {
        self.rpc_then("pane.create", json!({"command": "", "cwd": cwd}), Self::focus_created);
    }

    pub fn new_tab(&mut self, cx: &mut Context<Self>) {
        trace("button: new tab");
        let cwd = self.space.as_ref().and_then(|s| self.store.spaces.iter().find(|x| &x.id == s)).map(|s| s.cwd.clone());
        match cwd {
            Some(cwd) => self.rpc_then("pane.create", json!({"command": "", "cwd": cwd}), Self::focus_created),
            None => self.new_space(cx),
        }
    }

    /// Like Herdr: a new space with a shell in the folder the focused pane is in.
    pub fn new_space(&mut self, _cx: &mut Context<Self>) {
        let params = match self.focused.as_ref() {
            Some(from) => json!({"new": true, "from": from}),
            None => json!({"new": true}),
        };
        self.rpc_then("space.create", params, Self::focus_created);
    }

    pub fn split(&mut self, pane: &str, direction: &str) {
        let Some(p) = self.store.pane(pane) else { return };
        let params = json!({"command": "", "cwd": p.cwd, "tab_id": p.tab_id, "split_from": p.id, "direction": direction});
        self.rpc_then("pane.create", params, Self::focus_created);
    }

    pub fn toggle_zoom(&mut self, pane: &str) {
        let Some(p) = self.store.pane(pane).cloned() else { return };
        let zoomed = self.store.tab(&p.tab_id).is_some_and(|t| t.zoomed);
        self.focused = Some(p.id.clone());
        self.rpc("pane.focus", json!({"pane": p.id}));
        self.rpc("tab.zoom", json!({"tab": p.tab_id, "zoomed": !zoomed}));
    }

    pub fn ask_close_pane(&mut self, pane: &str, cx: &mut Context<Self>) {
        let Some(p) = self.store.pane(pane) else { return };
        if p.state == "working" {
            self.confirm = Some(Confirm { title: format!("Stop {}?", p.name), detail: "It is still working.".into(),
                                          ok: "Stop", then: Then::ClosePane(p.id.clone()) });
        } else {
            self.rpc("pane.close", json!({"pane": pane}));
        }
        cx.notify();
    }

    pub fn ask_close_tab(&mut self, tab: &str, cx: &mut Context<Self>) {
        let Some(t) = self.store.tab(tab) else { return };
        self.confirm = Some(Confirm { title: format!("Close tab {}?", t.name), detail: "Every agent in this tab stops.".into(),
                                      ok: "Close", then: Then::CloseTab(t.id.clone()) });
        cx.notify();
    }

    pub fn ask_close_space(&mut self, space: &str, cx: &mut Context<Self>) {
        let Some(s) = self.store.spaces.iter().find(|s| s.id == space) else { return };
        let running = self.store.panes_of_space(space).filter(|p| p.state != "exited").count();
        let detail = match running {
            0 => "The folder itself is not touched.".to_string(),
            n => format!("{n} agent{} running in it will stop. The folder itself is not touched.", if n > 1 { "s" } else { "" }),
        };
        self.confirm = Some(Confirm { title: format!("Close {}?", s.name), detail, ok: "Close", then: Then::CloseSpace(s.id.clone()) });
        cx.notify();
    }

    /// The Agents view's Stop: the agent and every helper it started.
    pub fn ask_stop_subtree(&mut self, pane: &str, cx: &mut Context<Self>) {
        let Some(p) = self.store.pane(pane) else { return };
        let helpers = drover_proto::agent_tree::subtree_ids(pane, &drover_proto::agent_tree::agents(&self.store.panes)).len() - 1;
        let detail = match helpers {
            0 => "It stops. Its terminal closes.".to_string(),
            n => format!("It and the {n} helper{} it started stop.", if n > 1 { "s" } else { "" }),
        };
        self.confirm = Some(Confirm { title: format!("Stop {}?", p.name), detail, ok: "Stop", then: Then::StopSubtree(p.id.clone()) });
        cx.notify();
    }

    pub fn answer_confirm(&mut self, yes: bool, window: &mut Window, cx: &mut Context<Self>) {
        if let (true, Some(c)) = (yes, self.confirm.take()) {
            match c.then {
                Then::ClosePane(id) => self.rpc("pane.close", json!({"pane": id})),
                Then::CloseTab(id) => self.rpc("tab.close", json!({"tab": id})),
                Then::CloseSpace(id) => self.rpc("space.remove", json!({"space": id})),
                Then::StopSubtree(id) => self.rpc("pane.close", json!({"pane": id, "subtree": true})),
                Then::RemoveMachine(alias) => self.rpc("machine.remove", json!({"alias": alias})),
                Then::StopServer => self.rpc("server.stop", json!({})),
                Then::UpdateApply(tag) => {
                    self.settings.update = Some(Err("Updating: the server restarts…".into()));
                    self.rpc_reply("update.apply", json!({"tag": tag}), |app, r| {
                        if let Err(e) = r { app.settings.update = Some(Err(format!("Update failed: {e}"))); }
                    });
                }
            }
        }
        self.confirm = None;
        window.focus(&self.term_focus);
        cx.notify();
    }

    /// Open a view the native app does not draw yet in the browser, through the web UI.
    pub fn open_web(&self, view: &str) {
        let Ok((port, token)) = drover_proto::client::daemon_address() else { return };
        let hash = if WEB_VIEWS.contains(&view) { format!("#{view}") } else { String::new() };
        open_in_browser(&format!("http://127.0.0.1:{port}/?token={token}{hash}"));
    }
}

fn curl_text(url: &str) -> Result<String, String> {
    let mut cmd = std::process::Command::new("curl");
    cmd.args(["-sSLf", "--max-time", "15", "-A", "drover", url]);
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000); // no console window
    let out = cmd.output().map_err(|e| format!("could not run curl: {e}"))?;
    if !out.status.success() {
        return Err(format!("{}", String::from_utf8_lossy(&out.stderr).trim()).chars().take(200).collect::<String>().replace("curl: ", ""));
    }
    String::from_utf8(out.stdout).map_err(|_| "the reply is not text".to_string())
}

/// Open `url` in the default browser (links in terminals, web-only views).
pub fn open_in_browser(url: &str) {
    let opener = if cfg!(target_os = "macos") { "open" } else if cfg!(windows) { "explorer" } else { "xdg-open" };
    if let Err(e) = std::process::Command::new(opener).arg(url).spawn() {
        eprintln!("drover: cannot open the browser ({opener}): {e}");
    }
}
