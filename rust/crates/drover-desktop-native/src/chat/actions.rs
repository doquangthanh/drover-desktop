//! What you can do with a chat: send, stop, answer a permission question, start a new one.

use gpui::{Context, Focusable, Window, prelude::*};
use serde_json::json;

use crate::views::View;
use crate::views::text_input::{InputEvent, TextInput};
use crate::workspace_app::DroverApp;

impl DroverApp {
    /// Show the Chat view with the message box focused; the first time, pick the newest chat that exists.
    pub fn open_chat_view(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.chat.composer.is_none() {
            let input = cx.new(|cx| TextInput::new_chat_box("Message Claude…  (Enter sends, Shift+Enter breaks the line)", cx));
            cx.subscribe(&input, |app, _, ev, cx| match ev {
                InputEvent::Submit => app.send_chat(cx),
                InputEvent::Changed => app.chat_text_changed(cx),
                InputEvent::Cancel => {}
            }).detach();
            self.chat.composer = Some(input);
        }
        if self.chat.current.is_none() && let Some(id) = self.chat.chats.first().map(|c| c.id.clone()) { self.open_chat(&id); }
        self.set_view(View::Chat, window, cx);
        if let Some(c) = &self.chat.composer { window.focus(&c.read(cx).focus_handle(cx)); }
    }

    /// Send what is typed to the open chat and empty the box.
    pub fn send_chat(&mut self, cx: &mut Context<Self>) {
        let (Some(id), Some(input)) = (self.chat.current.clone(), self.chat.composer.clone()) else { return };
        let text = input.read(cx).value().trim().to_string();
        if text.is_empty() { return; }
        self.rpc("chat.send", json!({"chat": id, "text": text}));
        input.update(cx, |i, cx| i.set_text("", cx));
        self.chat.suggest.clear();
        self.chat.stick.set(true);
        cx.notify();
    }

    pub fn stop_chat(&mut self) {
        if let Some(id) = self.chat.current.clone() { self.rpc("chat.interrupt", json!({"chat": id})); }
    }

    /// Allow / Always allow / Deny on a permission card (`request` is the card's item id).
    pub fn answer_permission(&mut self, request: &str, allow: bool, always: bool) {
        if let Some(id) = self.chat.current.clone() {
            self.rpc("chat.answer", json!({"chat": id, "request": request, "allow": allow, "always": always}));
        }
    }

    /// A new chat in the folder of the focused pane (else the focused space), opened when the daemon has made it.
    pub fn new_chat(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let pane = self.focused.as_deref().and_then(|p| self.store.pane(p));
        let cwd = pane.map(|p| p.cwd.clone()).filter(|c| !c.is_empty())
            .or_else(|| self.space.as_deref().and_then(|s| self.store.spaces.iter().find(|x| x.id == s)).map(|s| s.cwd.clone()));
        let Some(cwd) = cwd else { return self.toasts.error("Open a space first: a chat works in a folder.".into()) };
        self.rpc_reply("chat.create", json!({"cwd": cwd}), |app, r| match r {
            Ok(v) => if let Some(id) = v["id"].as_str() { app.open_chat(id); app.chat.stick.set(true); },
            Err(e) => app.toasts.error(format!("new chat: {e}")),
        });
        self.open_chat_view(window, cx);
    }
}
