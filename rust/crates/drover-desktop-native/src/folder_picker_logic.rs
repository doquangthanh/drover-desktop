//! Pure helpers of the folder picker (folder_picker.rs), the same rules as the web UI's folder-picker-logic.ts.

use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct DirEntry {
    pub name: String,
    pub path: String,
    pub is_git: bool,
    /// Modified time in seconds since the epoch; 0 when unknown.
    pub mtime: f64,
}

/// One answer of `fs.list_dirs`.
#[derive(Clone, Debug, PartialEq)]
pub struct DirListing {
    pub path: String,
    pub parent: Option<String>,
    pub roots: Vec<String>,
    pub entries: Vec<DirEntry>,
}

impl DirListing {
    pub fn from_json(v: &Value) -> Option<DirListing> {
        let text = |x: &Value| x.as_str().map(String::from);
        Some(DirListing {
            path: text(&v["path"])?,
            parent: text(&v["parent"]),
            roots: v["roots"].as_array().into_iter().flatten().filter_map(text).collect(),
            entries: v["entries"].as_array().into_iter().flatten().filter_map(|e| Some(DirEntry {
                name: text(&e["name"])?,
                path: text(&e["path"])?,
                is_git: e["is_git"].as_bool().unwrap_or(false),
                mtime: e["mtime"].as_f64().unwrap_or(0.0),
            })).collect(),
        })
    }
}

/// Last path segment ("/a/b/" -> "b"); the whole path when there is none.
pub fn base_name(p: &str) -> &str {
    p.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next().filter(|s| !s.is_empty()).unwrap_or(p)
}

/// Folder age as the picker shows it: today / yesterday / N days ago / N weeks ago / N months ago.
pub fn folder_age(mtime: f64, now: f64) -> String {
    let d = ((now - mtime) / 86400.0).floor() as i64;
    match d {
        i64::MIN..=0 => "today".into(),
        1 => "yesterday".into(),
        2..=6 => format!("{d} days ago"),
        7..=29 => format!("{} week{} ago", d / 7, if d >= 14 { "s" } else { "" }),
        _ => format!("{} month{} ago", d / 30, if d >= 60 { "s" } else { "" }),
    }
}

/// The entries whose name contains `filter`, case ignored; all of them for an empty filter.
pub fn filter_entries<'a>(entries: &'a [DirEntry], filter: &str) -> Vec<&'a DirEntry> {
    let f = filter.trim().to_lowercase();
    entries.iter().filter(|e| f.is_empty() || e.name.to_lowercase().contains(&f)).collect()
}

/// Distinct space folders on one machine (`None` = this computer), in the order given, at most `limit`.
pub fn recent_folders(spaces: &[(Option<String>, String)], machine: Option<&str>, limit: usize) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for (m, cwd) in spaces {
        let here = m.as_deref().filter(|m| !m.is_empty() && *m != "local");
        if here == machine && !cwd.is_empty() && !out.contains(cwd) {
            out.push(cwd.clone());
        }
    }
    out.truncate(limit);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn e(name: &str) -> DirEntry { DirEntry { name: name.into(), path: format!("/x/{name}"), is_git: false, mtime: 0.0 } }

    #[test]
    fn base_name_takes_the_last_segment() {
        assert_eq!(base_name("/a/b/"), "b");
        assert_eq!(base_name("C:\\Users\\me"), "me");
        assert_eq!(base_name("/"), "/");
    }

    #[test]
    fn age_words() {
        let day = 86400.0;
        assert_eq!(folder_age(1000.0, 1000.0 + 3600.0), "today");
        assert_eq!(folder_age(0.0, day), "yesterday");
        assert_eq!(folder_age(0.0, 3.0 * day), "3 days ago");
        assert_eq!(folder_age(0.0, 8.0 * day), "1 week ago");
        assert_eq!(folder_age(0.0, 15.0 * day), "2 weeks ago");
        assert_eq!(folder_age(0.0, 45.0 * day), "1 month ago");
        assert_eq!(folder_age(0.0, 70.0 * day), "2 months ago");
    }

    #[test]
    fn filter_ignores_case_and_empty_keeps_all() {
        let list = [e("Alpha"), e("beta")];
        assert_eq!(filter_entries(&list, "").len(), 2);
        assert_eq!(filter_entries(&list, " ALP ")[0].name, "Alpha");
        assert!(filter_entries(&list, "zzz").is_empty());
    }

    #[test]
    fn recent_is_per_machine_and_distinct() {
        let spaces = vec![
            (None, "/a".to_string()), (Some("local".into()), "/a".to_string()), (Some("box".into()), "/r".to_string()),
            (None, "/b".to_string()), (None, String::new()),
        ];
        assert_eq!(recent_folders(&spaces, None, 6), vec!["/a", "/b"]);
        assert_eq!(recent_folders(&spaces, Some("box"), 6), vec!["/r"]);
        assert_eq!(recent_folders(&spaces, None, 1), vec!["/a"]);
    }

    #[test]
    fn listing_parses_the_daemon_reply() {
        let v = json!({"path": "/h", "parent": null, "roots": ["/h"],
            "entries": [{"name": "src", "path": "/h/src", "is_git": true, "mtime": 5.0}, {"path": "/h/bad"}]});
        let l = DirListing::from_json(&v).unwrap();
        assert_eq!((l.path.as_str(), l.parent, l.roots.len(), l.entries.len()), ("/h", None, 1, 1));
        assert!(l.entries[0].is_git);
        assert!(DirListing::from_json(&json!({})).is_none());
    }
}
