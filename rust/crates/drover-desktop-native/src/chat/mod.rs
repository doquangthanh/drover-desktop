//! Chat view: Claude conversations shown as a chat (port of drover-web `chat/`). This file is the state: the chat
//! list, each chat's transcript and what the daemon's `chats` / `chat` events change in them. The drawing is in the
//! other files of this folder.

mod actions;
pub mod options;
pub mod transcript;
pub mod logic;
pub mod markdown;
pub mod types;
pub mod view;

use std::collections::{HashMap, HashSet};

use serde_json::{Value, json};

use crate::workspace_app::DroverApp;
use types::{Chat, Item};

#[derive(Default)]
pub struct ChatState {
    pub chats: Vec<Chat>,
    pub items: HashMap<String, Vec<Item>>,
    /// Chats whose history was fetched.
    pub loaded: HashSet<String>,
    /// The chat shown on the right.
    pub current: Option<String>,
    /// The message box (made when the view is first opened).
    pub composer: Option<gpui::Entity<crate::views::text_input::TextInput>>,
    /// The transcript scrolls to its end on the next frame: set when something arrives or you send.
    pub stick: std::cell::Cell<bool>,
    pub scroll: gpui::ScrollHandle,
    /// Models and commands per chat, and the chats they were asked for.
    pub options: HashMap<String, options::Options>,
    pub options_asked: HashSet<String>,
    /// Suggestions under the message box: (what is inserted, a note), and the `@` query they answer.
    pub suggest: Vec<(String, String)>,
    pub suggest_for: String,
}

impl ChatState {
    /// Follow one daemon event; true when something a chat shows changed.
    pub fn apply_event(&mut self, name: &str, data: &Value) -> bool {
        match name {
            "state" | "resync" => data["chats"].as_array().map(|l| self.set_chats(l)).is_some(),
            "chats" => data.as_array().map(|l| self.set_chats(l)).is_some(),
            "chat" => {
                let Some(id) = data["chat"].as_str() else { return false };
                let list = self.items.entry(id.to_string()).or_default();
                match data["op"].as_str() {
                    Some("item") => if let Some(item) = Item::from_json(&data["item"]) { logic::upsert(list, item); },
                    Some("delta") => logic::append_delta(list, data["id"].as_str().unwrap_or(""), data["text"].as_str().unwrap_or("")),
                    _ => return false,
                }
                self.stick.set(true);
                true
            }
            _ => false,
        }
    }

    fn set_chats(&mut self, list: &[Value]) {
        self.chats = list.iter().filter_map(Chat::from_json).collect();
        let alive: HashSet<&str> = self.chats.iter().map(|c| c.id.as_str()).collect();
        self.items.retain(|id, _| alive.contains(id.as_str())); // forget transcripts of deleted chats
        self.loaded.retain(|id| alive.contains(id.as_str()));
        if self.current.as_deref().is_some_and(|c| !alive.contains(c)) {
            self.current = None;
        }
    }

    pub fn current_chat(&self) -> Option<&Chat> {
        self.current.as_deref().and_then(|id| self.chats.iter().find(|c| c.id == id))
    }

    pub fn items_of(&self, id: &str) -> &[Item] { self.items.get(id).map_or(&[], |v| v.as_slice()) }

    /// A fetched history goes underneath whatever arrived live meanwhile.
    pub fn set_history(&mut self, id: &str, history: &Value) {
        let history: Vec<Item> = history.as_array().into_iter().flatten().filter_map(Item::from_json).collect();
        let live = self.items.remove(id).unwrap_or_default();
        self.items.insert(id.to_string(), logic::merge_history(history, live));
        self.loaded.insert(id.to_string());
    }
}

impl DroverApp {
    /// Show chat `id` and fetch its history the first time.
    pub fn open_chat(&mut self, id: &str) {
        self.chat.current = Some(id.to_string());
        self.chat.suggest.clear();
        self.load_chat_options(id);
        if !self.chat.loaded.contains(id) { self.load_chat_history(id); }
    }

    pub fn load_chat_history(&mut self, id: &str) {
        let chat = id.to_string();
        self.rpc_reply("chat.history", json!({"chat": id}), move |app, r| match r {
            Ok(v) => app.chat.set_history(&chat, if v["items"].is_array() { &v["items"] } else { &v }),
            Err(e) => app.toasts.error(format!("chat history: {e}")),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chats() -> Value { json!([{"id": "c1", "name": "one", "cwd": "/x", "state": "working", "permission_mode": "default"}]) }

    #[test]
    fn chats_event_replaces_the_list_and_drops_dead_transcripts() {
        let mut s = ChatState::default();
        assert!(s.apply_event("chats", &chats()));
        s.apply_event("chat", &json!({"chat": "c1", "op": "item", "item": {"id": "i", "kind": "text", "text": "hi"}}));
        s.apply_event("chat", &json!({"chat": "gone", "op": "item", "item": {"id": "i", "kind": "text", "text": "x"}}));
        s.current = Some("c1".into());
        s.apply_event("chats", &chats());
        assert_eq!(s.items_of("c1").len(), 1);
        s.apply_event("chats", &json!([]));
        assert!(s.items.is_empty() && s.current.is_none() && s.chats.is_empty());
    }

    #[test]
    fn state_event_carries_chats_and_ops_stream_text() {
        let mut s = ChatState::default();
        assert!(s.apply_event("state", &json!({"chats": chats()})));
        assert!(!s.apply_event("state", &json!({"panes": []})), "no chats key: nothing to follow");
        s.apply_event("chat", &json!({"chat": "c1", "op": "item", "item": {"id": "t", "kind": "text", "text": "he", "live": true}}));
        s.apply_event("chat", &json!({"chat": "c1", "op": "delta", "id": "t", "text": "llo"}));
        assert_eq!(s.items_of("c1")[0].text, "hello");
        assert!(!s.apply_event("chat", &json!({"chat": "c1", "op": "other"})));
    }

    #[test]
    fn history_merges_under_live_items() {
        let mut s = ChatState::default();
        s.apply_event("chat", &json!({"chat": "c1", "op": "item", "item": {"id": "b", "kind": "text", "text": "live"}}));
        s.set_history("c1", &json!([{"id": "a", "kind": "user", "text": "q"}, {"id": "b", "kind": "text", "text": "old"}]));
        assert_eq!(s.items_of("c1").iter().map(|i| i.text.as_str()).collect::<Vec<_>>(), ["q", "live"]);
        assert!(s.loaded.contains("c1"));
    }
}
