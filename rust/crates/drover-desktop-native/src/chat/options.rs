//! What the message box offers: the agent's models and slash commands (`chat.options`), `/` and `@` suggestions
//! under the box, and the model / permission-mode menus.

use gpui::{Context, Pixels, Point};
use serde_json::{Value, json};

use crate::context_menu::{Act, Entry, item};
use crate::workspace_app::DroverApp;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Options {
    /// (name, description)
    pub commands: Vec<(String, String)>,
    /// (value, label)
    pub models: Vec<(String, String)>,
}

impl Options {
    pub fn from_json(v: &Value) -> Options {
        let pair = |a: &Value, k1: &str, k2: &str| Some((a[k1].as_str()?.to_string(), a[k2].as_str().unwrap_or("").to_string()));
        Options {
            commands: v["commands"].as_array().into_iter().flatten().filter_map(|c| pair(c, "name", "description")).collect(),
            models: v["models"].as_array().into_iter().flatten().filter_map(|m| pair(m, "value", "label")).collect(),
        }
    }
}

/// The word being typed at the end of `text` when it starts with `/` or `@`: (sigil, what follows it, byte offset).
/// A `/` only counts at the very start of the message, as slash commands do.
pub fn token_at_end(text: &str) -> Option<(char, &str, usize)> {
    let start = text.rfind(char::is_whitespace).map_or(0, |i| i + text[i..].chars().next().map_or(1, char::len_utf8));
    let word = &text[start..];
    let sigil = word.chars().next()?;
    match sigil {
        '@' => Some(('@', &word[1..], start)),
        '/' if start == 0 => Some(('/', &word[1..], start)),
        _ => None,
    }
}

/// `text` with the word that starts at `start` replaced by `insert`.
pub fn replace_token(text: &str, start: usize, insert: &str) -> String { format!("{}{}", &text[..start], insert) }

pub const MODES: [(&str, &str); 4] = [("default", "Ask before changes"), ("acceptEdits", "Edit automatically"), ("plan", "Plan only"), ("bypassPermissions", "Full access")];

impl DroverApp {
    /// Fetch the agent's models and commands once per chat (the first call starts the agent, about a second).
    pub fn load_chat_options(&mut self, id: &str) {
        if self.chat.options.contains_key(id) || !self.chat.options_asked.insert(id.to_string()) { return; }
        let chat = id.to_string();
        self.rpc_reply("chat.options", json!({"chat": id}), move |app, r| if let Ok(v) = r { app.chat.options.insert(chat, Options::from_json(&v)); });
    }

    /// The box's text changed: offer commands (`/`) or project files (`@`) for the word at its end.
    pub fn chat_text_changed(&mut self, cx: &mut Context<Self>) {
        let (Some(id), Some(input)) = (self.chat.current.clone(), self.chat.composer.clone()) else { return };
        let text = input.read(cx).value();
        let token = token_at_end(&text).map(|(s, q, _)| (s, q.to_string()));
        self.chat.suggest.clear();
        match token {
            Some(('/', q)) => {
                let q = q.to_lowercase();
                self.chat.suggest = self.chat.options.get(&id).into_iter().flat_map(|o| o.commands.iter())
                    .filter(|(n, _)| n.to_lowercase().contains(&q)).take(8).map(|(n, d)| (format!("/{n}"), d.clone())).collect();
            }
            Some(('@', q)) => {
                self.chat.suggest_for = q.clone();
                self.rpc_reply("chat.files", json!({"chat": id, "query": q, "limit": 8}), move |app, r| {
                    // An older answer never overwrites a newer one.
                    if app.chat.suggest_for != q { return; }
                    app.chat.suggest = r.ok().iter().flat_map(|v| v["files"].as_array().into_iter().flatten()).filter_map(|f| f.as_str())
                        .map(|f| (format!("@{f}"), String::new())).collect();
                });
            }
            _ => {}
        }
        cx.notify();
    }

    /// Put suggestion `i` in place of the word being typed.
    pub fn apply_suggestion(&mut self, i: usize, cx: &mut Context<Self>) {
        let (Some(input), Some((label, _))) = (self.chat.composer.clone(), self.chat.suggest.get(i).cloned()) else { return };
        let text = input.read(cx).value();
        if let Some((_, _, start)) = token_at_end(&text) { input.update(cx, |t, cx| t.set_text(&replace_token(&text, start, &format!("{label} ")), cx)); }
        self.chat.suggest.clear();
        cx.notify();
    }

    pub fn open_chat_mode_menu(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        let now = self.chat.current_chat().map(|c| c.permission_mode.clone()).unwrap_or_default();
        let entries = MODES.iter().map(|(m, l)| item(&format!("{}{l}", if *m == now { "✓ " } else { "" }), Act::ChatMode((*m).into()))).collect();
        self.open_menu(at, entries, cx);
    }

    pub fn open_chat_model_menu(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(chat) = self.chat.current_chat() else { return };
        let now = chat.model.clone().unwrap_or_default();
        let mut entries = vec![item(&format!("{}Agent's default", if now.is_empty() { "✓ " } else { "" }), Act::ChatModel(String::new()))];
        match self.chat.options.get(&chat.id) {
            Some(o) => entries.extend(o.models.iter().map(|(v, l)| item(&format!("{}{l}", if *v == now { "✓ " } else { "" }), Act::ChatModel(v.clone())))),
            None => entries.push(Entry::Head("Loading models…".into())),
        }
        self.open_menu(at, entries, cx);
    }

    pub fn set_chat_mode(&mut self, mode: &str) {
        if let Some(c) = self.chat.current.clone() { self.rpc("chat.set_mode", json!({"chat": c, "permission_mode": mode})); }
    }

    pub fn set_chat_model(&mut self, model: &str) {
        if let Some(c) = self.chat.current.clone() { self.rpc("chat.set_model", json!({"chat": c, "model": model})); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_at_the_end_of_the_text() {
        assert_eq!(token_at_end("/com"), Some(('/', "com", 0)));
        assert_eq!(token_at_end("look at @src/ma"), Some(('@', "src/ma", 8)));
        assert_eq!(token_at_end("@"), Some(('@', "", 0)));
        assert_eq!(token_at_end("a /not-a-command"), None, "a slash only counts at the start");
        assert_eq!(token_at_end("plain words "), None);
        assert_eq!(token_at_end(""), None);
        assert_eq!(token_at_end("é @x"), Some(('@', "x", 3)));
    }

    #[test]
    fn replacing_the_token() {
        assert_eq!(replace_token("look at @sr", 8, "@src/main.rs "), "look at @src/main.rs ");
    }

    #[test]
    fn options_parse() {
        let o = Options::from_json(&json!({"commands": [{"name": "compact", "description": "d"}, {}], "models": [{"value": "m", "label": "M"}]}));
        assert_eq!((o.commands.len(), o.models), (1, vec![("m".to_string(), "M".to_string())]));
    }
}
