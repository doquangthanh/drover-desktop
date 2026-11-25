//! The "New agent" dialog (the web UI's new-agent-dialog.tsx): which agent, a name, the folder, arguments, a first
//! prompt (sent once the agent is ready) and an optional task worktree. It starts in a new tab of the current space's
//! machine. Escape or a click outside closes it; Enter in a one-line field starts.

use gpui::{Context, Div, Entity, Focusable, FontWeight, MouseButton, Window, div, prelude::*, rgb, rgba};
use serde_json::json;

use crate::theme::{self, dim, fg, muted, UI_FONT};
use crate::ui_scale::px;
use crate::views::text_input::{InputEvent, TextInput};
use crate::views::ui::{self, Kind};
use crate::workspace_app::DroverApp;

/// (kind, label, command): `agent.kinds` of the machine, a plain terminal first.
pub type AgentKind = (String, String, String);

pub struct NewAgent {
    pub kinds: Vec<AgentKind>,
    pub kind: usize,
    pub machine: Option<String>,
    pub name: Entity<TextInput>,
    pub folder: Entity<TextInput>,
    pub args: Entity<TextInput>,
    pub prompt: Entity<TextInput>,
    pub worktree: bool,
}

/// The command line: the agent's command and the extra arguments.
pub fn command_line(base: &str, args: &str) -> String {
    [base.trim(), args.trim()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" ")
}

impl DroverApp {
    pub fn open_new_agent(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (cwd, machine) = self.space_target().unwrap_or_default();
        let field = |cx: &mut Context<Self>, hint: &'static str, text: &str| {
            let t = cx.new(|cx| { let mut i = TextInput::new(hint, cx); i.set_text(text, cx); i });
            cx.subscribe(&t, |app, _, ev, cx| if matches!(ev, InputEvent::Submit) { app.start_new_agent(cx) }).detach();
            t
        };
        let name = field(cx, "e.g. discount codes", "");
        let folder = field(cx, "Folder", &cwd);
        let args = field(cx, "extra arguments (optional)", "");
        let prompt = cx.new(TextInput::new_multiline);
        window.focus(&name.read(cx).focus_handle(cx));
        self.new_agent = Some(NewAgent { kinds: vec![("shell".into(), "Terminal".into(), String::new())], kind: 0, machine: machine.clone(), name, folder, args, prompt, worktree: false });
        let params = machine.map(|m| json!({"machine": m})).unwrap_or_else(|| json!({}));
        self.rpc_reply("agent.kinds", params, |app, r| {
            let (Some(d), Ok(v)) = (app.new_agent.as_mut(), r) else { return };
            for k in v.as_array().into_iter().flatten().filter(|k| k["installed"] != false) {
                let kind = k["kind"].as_str().unwrap_or("");
                d.kinds.push((kind.to_string(), k["label"].as_str().unwrap_or(kind).to_string(), k["command"].as_str().unwrap_or(kind).to_string()));
            }
            if d.kinds.len() > 1 { d.kind = 1; } // the first agent, as the web dialog starts on Claude
        });
        cx.notify();
    }

    pub fn close_new_agent(&mut self, cx: &mut Context<Self>) {
        self.new_agent = None;
        self.want_term_focus = true;
        cx.notify();
    }

    pub fn start_new_agent(&mut self, cx: &mut Context<Self>) {
        let Some(d) = &self.new_agent else { return };
        let (_, _, base) = d.kinds.get(d.kind).cloned().unwrap_or_default();
        let (name, cwd) = (d.name.read(cx).value().trim().to_string(), d.folder.read(cx).value().trim().to_string());
        let command = command_line(&base, &d.args.read(cx).value());
        let prompt = d.prompt.read(cx).value().trim().to_string();
        let mut params = json!({"command": command, "cwd": cwd, "name": name});
        if let Some(m) = &d.machine { params["machine"] = json!(m); }
        if d.worktree {
            let task = if name.is_empty() { d.kinds[d.kind].0.clone() } else { name };
            params["worktree"] = json!({"task": task});
        }
        self.close_new_agent(cx);
        self.rpc_reply("pane.create", params, move |app, r| match r {
            Err(e) => app.toasts.error(e),
            Ok(v) => {
                let id = v["id"].as_str().or_else(|| v["pane"]["id"].as_str()).unwrap_or("").to_string();
                Self::focus_created(app, v);
                if !prompt.is_empty() && !id.is_empty() {
                    // Once the agent is ready (idle or waiting), as the web dialog does.
                    app.rpc_reply("pane.wait", json!({"pane": id, "states": ["idle", "blocked"], "timeout": 25}), move |app, _| {
                        app.rpc("pane.send", json!({"pane": id, "text": prompt}));
                    });
                }
            }
        });
    }
}

fn label(text: &str) -> Div { div().text_size(px(12.)).text_color(rgb(muted())).child(text.to_string()) }

fn boxed(input: &Entity<TextInput>, h: f32) -> Div { div().h(px(h)).child(input.clone()) }

pub fn overlay(app: &DroverApp, cx: &mut Context<DroverApp>) -> Option<Div> {
    let d = app.new_agent.as_ref()?;
    let chips = d.kinds.iter().enumerate().map(|(i, (_, name, _))| {
        ui::tab(name, i == d.kind).on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| {
            if let Some(d) = &mut app.new_agent { d.kind = i; }
            cx.notify();
        }))
    }).collect::<Vec<_>>();
    let place = d.machine.clone().unwrap_or_else(|| "This computer".into());
    let card = div().w(px(560.)).p(px(22.)).flex().flex_col().gap(px(10.)).rounded(px(12.)).bg(rgb(theme::card()))
        .border_1().border_color(theme::border2()).shadow_lg().font_family(UI_FONT)
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(div().flex().items_center()
            .child(div().flex_1().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).text_color(rgb(fg())).child("New agent"))
            .child(div().text_size(px(12.)).text_color(rgb(dim())).child(place)))
        .child(label("Agent"))
        .child(div().flex().flex_wrap().gap(px(4.)).children(chips))
        .child(label("Name")).child(boxed(&d.name, 32.))
        .child(label("Folder")).child(boxed(&d.folder, 32.))
        .child(label("Arguments")).child(boxed(&d.args, 32.))
        .child(label("First prompt (sent once the agent is ready, optional)")).child(boxed(&d.prompt, 70.))
        .child(div().flex().items_center().gap(px(10.)).child(ui::toggle(d.worktree))
            .child(div().flex_1().text_size(px(13.)).text_color(rgb(fg())).child("Work in a new task worktree (branch dr/<name>)"))
            .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| {
                if let Some(d) = &mut app.new_agent { d.worktree = !d.worktree; }
                cx.notify();
            })))
        .child(div().flex().items_center().gap(px(8.)).justify_end()
            .child(ui::button("Cancel", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.close_new_agent(cx))))
            .child(ui::button("Start", Kind::Primary).on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.start_new_agent(cx)))));
    Some(div().absolute().top_0().left_0().size_full().flex().items_center().justify_center().bg(rgba(0x00000073))
        .occlude().child(card)
        .on_mouse_down(MouseButton::Left, cx.listener(|app, _, _, cx| app.close_new_agent(cx))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_line_joins_what_is_set() {
        assert_eq!(command_line("claude", " --resume "), "claude --resume");
        assert_eq!(command_line("", "x"), "x");
        assert_eq!(command_line("", ""), "");
    }
}
