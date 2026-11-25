//! What the daemon sends for chats (the item contract of drover-web `chat-types.ts`), read from JSON by hand so the
//! crate needs no serde derive. One flat `Item` for every kind keeps the transcript code simple.

use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct Chat {
    pub id: String,
    pub name: String,
    pub cwd: String,
    /// idle | working | blocked
    pub state: String,
    pub permission_mode: String,
    pub model: Option<String>,
}

impl Chat {
    pub fn from_json(v: &Value) -> Option<Chat> {
        Some(Chat {
            id: v["id"].as_str()?.to_string(),
            name: v["name"].as_str().unwrap_or("chat").to_string(),
            cwd: v["cwd"].as_str().unwrap_or("").to_string(),
            state: v["state"].as_str().unwrap_or("idle").to_string(),
            permission_mode: v["permission_mode"].as_str().unwrap_or("default").to_string(),
            model: v["model"].as_str().filter(|m| !m.is_empty()).map(String::from),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { User, Text, Thinking, Tool, Permission, Result, Notice }

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub id: String,
    pub kind: Kind,
    /// user / text / thinking text, or a notice's text.
    pub text: String,
    /// Number of images attached to a user message.
    pub images: usize,
    /// Tool or permission: the tool's name and its input.
    pub tool: String,
    pub input: Value,
    /// tool: running | done | error | stopped; permission: pending | allowed | denied.
    pub status: String,
    pub result: Option<String>,
    pub is_error: bool,
    pub title: Option<String>,
    pub detail: Option<String>,
    /// result item: ok, duration in ms, cost, turns, error.
    pub ok: bool,
    pub ms: f64,
    pub cost: Option<f64>,
    pub turns: i64,
    pub error: Option<String>,
    /// notice: info | error.
    pub level: String,
}

impl Item {
    pub fn from_json(v: &Value) -> Option<Item> {
        let s = |k: &str| v[k].as_str().map(String::from);
        let kind = match v["kind"].as_str()? {
            "user" => Kind::User,
            "text" => Kind::Text,
            "thinking" => Kind::Thinking,
            "tool" => Kind::Tool,
            "permission" => Kind::Permission,
            "result" => Kind::Result,
            "notice" => Kind::Notice,
            _ => return None,
        };
        Some(Item {
            id: s("id")?,
            kind,
            text: s("text").unwrap_or_default(),
            images: v["images"].as_array().map_or(0, |a| a.len()),
            tool: s("name").or_else(|| s("tool")).unwrap_or_default(),
            input: v["input"].clone(),
            status: s("status").unwrap_or_default(),
            result: s("result"),
            is_error: v["is_error"].as_bool().unwrap_or(false),
            title: s("title"),
            detail: s("detail"),
            ok: v["ok"].as_bool().unwrap_or(false),
            ms: v["ms"].as_f64().unwrap_or(0.0),
            cost: v["cost"].as_f64(),
            turns: v["turns"].as_i64().unwrap_or(0),
            error: s("error"),
            level: s("level").unwrap_or_else(|| "info".into()),
        })
    }
}
