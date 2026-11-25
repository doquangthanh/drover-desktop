//! Pure helpers of the chat screen (drover-web `chat-logic.ts`): transcript updates, tool summaries, edit diffs.

use serde_json::Value;

use super::types::{Item, Kind};

/// Insert or replace by id, keeping the place of an existing item.
pub fn upsert(list: &mut Vec<Item>, item: Item) {
    match list.iter().position(|x| x.id == item.id) {
        Some(i) => list[i] = item,
        None => list.push(item),
    }
}

/// Append streamed text to a text / thinking item. An unknown id is ignored: the item arrives with its own text.
pub fn append_delta(list: &mut [Item], id: &str, text: &str) {
    if let Some(i) = list.iter_mut().find(|x| x.id == id && matches!(x.kind, Kind::Text | Kind::Thinking)) {
        i.text.push_str(text);
    }
}

/// History from the server underneath, anything that arrived live meanwhile on top.
pub fn merge_history(history: Vec<Item>, live: Vec<Item>) -> Vec<Item> {
    live.into_iter().fold(history, |mut all, item| { upsert(&mut all, item); all })
}

fn str_of<'a>(input: &'a Value, key: &str) -> &'a str { input[key].as_str().unwrap_or("") }

fn base(p: &str) -> &str { p.rsplit(['/', '\\']).find(|s| !s.is_empty()).unwrap_or(p) }

/// One line saying what a tool call does ("Read pricing.tsx", "Run npm test").
pub fn tool_summary(name: &str, input: &Value) -> String {
    let path = [str_of(input, "file_path"), str_of(input, "path"), str_of(input, "notebook_path")].into_iter().find(|p| !p.is_empty()).unwrap_or("");
    match name {
        "Read" | "Write" | "Edit" | "MultiEdit" | "NotebookEdit" => if path.is_empty() { String::new() } else { base(path).to_string() },
        "Bash" => str_of(input, "command").lines().next().unwrap_or("").chars().take(120).collect(),
        "Grep" | "Glob" => str_of(input, "pattern").to_string(),
        "WebFetch" => str_of(input, "url").to_string(),
        "WebSearch" => str_of(input, "query").to_string(),
        "Task" | "Agent" => str_of(input, "description").to_string(),
        _ => if !path.is_empty() { path.to_string() } else { str_of(input, "description").to_string() },
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DiffLine { pub sign: char, pub text: String }

fn lines(s: &str) -> impl Iterator<Item = &str> { s.lines().map(|l| l.trim_end_matches('\r')) }

/// Edit / MultiEdit / Write input as removed and added lines (the agent gives old and new text, so not a true diff).
pub fn edit_diff(name: &str, input: &Value) -> Option<Vec<DiffLine>> {
    let line = |sign, t: &str| DiffLine { sign, text: t.to_string() };
    match name {
        "Edit" => Some(lines(str_of(input, "old_string")).map(|t| line('-', t)).chain(lines(str_of(input, "new_string")).map(|t| line('+', t))).collect()),
        "MultiEdit" => Some(input["edits"].as_array()?.iter().flat_map(|e| edit_diff("Edit", e).unwrap_or_default()).collect()),
        "Write" => Some(lines(str_of(input, "content")).map(|t| line('+', t)).collect()),
        _ => None,
    }
}

pub fn diff_counts(d: &[DiffLine]) -> (usize, usize) {
    (d.iter().filter(|l| l.sign == '+').count(), d.iter().filter(|l| l.sign == '-').count())
}

/// "Run this command?" style question of a permission card.
pub fn permission_question(item: &Item) -> String {
    let what = tool_summary(&item.tool, &item.input);
    match item.tool.as_str() {
        "Bash" => "Run this command?".into(),
        "Edit" | "Write" | "MultiEdit" => format!("Change {}?", if what.is_empty() { "a file" } else { &what }),
        t if what.is_empty() => format!("Allow {t}?"),
        t => format!("Allow {t} ({what})?"),
    }
}

/// "12.3 s" / "340 ms" / "2 min 5 s".
pub fn duration(ms: f64) -> String {
    if ms < 1000.0 { format!("{} ms", ms.round() as i64) }
    else if ms < 60_000.0 { format!("{:.1} s", ms / 1000.0) }
    else { format!("{} min {} s", (ms / 60_000.0) as i64, ((ms % 60_000.0) / 1000.0).round() as i64) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn item(id: &str, kind: &str, text: &str) -> Item { Item::from_json(&json!({"id": id, "kind": kind, "text": text})).unwrap() }

    #[test]
    fn upsert_keeps_the_place_of_an_existing_item() {
        let mut l = vec![item("a", "text", "1"), item("b", "text", "2")];
        upsert(&mut l, item("a", "text", "new"));
        upsert(&mut l, item("c", "text", "3"));
        assert_eq!(l.iter().map(|i| (i.id.as_str(), i.text.as_str())).collect::<Vec<_>>(), [("a", "new"), ("b", "2"), ("c", "3")]);
    }

    #[test]
    fn delta_appends_to_text_and_thinking_only() {
        let mut l = vec![item("a", "text", "he"), item("u", "user", "x")];
        append_delta(&mut l, "a", "llo");
        append_delta(&mut l, "u", "!");
        append_delta(&mut l, "zzz", "!");
        assert_eq!((l[0].text.as_str(), l[1].text.as_str()), ("hello", "x"));
    }

    #[test]
    fn history_goes_underneath_live_items() {
        let m = merge_history(vec![item("a", "text", "old"), item("b", "text", "b")], vec![item("a", "text", "live"), item("c", "text", "c")]);
        assert_eq!(m.iter().map(|i| i.text.as_str()).collect::<Vec<_>>(), ["live", "b", "c"]);
    }

    #[test]
    fn summaries_and_questions() {
        assert_eq!(tool_summary("Read", &json!({"file_path": "/a/b/pricing.tsx"})), "pricing.tsx");
        assert_eq!(tool_summary("Bash", &json!({"command": "npm test\nmore"})), "npm test");
        assert_eq!(tool_summary("Grep", &json!({"pattern": "foo"})), "foo");
        let mut p = item("p", "permission", "");
        p.tool = "Bash".into();
        assert_eq!(permission_question(&p), "Run this command?");
        p.tool = "Edit".into();
        p.input = json!({"file_path": "/x/y.rs"});
        assert_eq!(permission_question(&p), "Change y.rs?");
        p.tool = "WebFetch".into();
        p.input = json!({});
        assert_eq!(permission_question(&p), "Allow WebFetch?");
    }

    #[test]
    fn diff_of_edit_multiedit_and_write() {
        let e = json!({"old_string": "a\nb", "new_string": "c"});
        let d = edit_diff("Edit", &e).unwrap();
        assert_eq!(diff_counts(&d), (1, 2));
        assert_eq!(edit_diff("MultiEdit", &json!({"edits": [e.clone(), e]})).unwrap().len(), 6);
        assert_eq!(edit_diff("Write", &json!({"content": "x\ny"})).unwrap().len(), 2);
        assert!(edit_diff("Bash", &json!({})).is_none());
    }

    #[test]
    fn durations() {
        assert_eq!(duration(340.0), "340 ms");
        assert_eq!(duration(12_300.0), "12.3 s");
        assert_eq!(duration(125_000.0), "2 min 5 s");
    }

    #[test]
    fn unknown_kinds_and_missing_ids_are_skipped() {
        assert!(Item::from_json(&json!({"id": "a", "kind": "weird"})).is_none());
        assert!(Item::from_json(&json!({"kind": "text"})).is_none());
    }
}
