//! File icons of the Files tree and tabs: the web UI's default "Material" set (core/file-icons.ts `materialName`), the
//! Material Icon Theme drawn in colour per file type and folder name. The icons and their lookup table are embedded
//! (assets/file-icons, MIT, see LICENSE-material-icon-theme.txt). The web's Badges and Line sets and the choice in
//! Settings are not ported.

use std::collections::HashMap;
use std::sync::OnceLock;

use gpui::{Img, img, prelude::*};
use serde_json::Value;

use crate::ui_scale::px;

struct Table {
    file: String,
    folder: String,
    folder_open: String,
    folders: HashMap<String, String>,
    open_of: HashMap<String, String>,
    names: HashMap<String, String>,
    ext: HashMap<String, String>,
}

fn strings(v: &Value) -> HashMap<String, String> {
    v.as_object().into_iter().flatten().filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string()))).collect()
}

fn table() -> &'static Table {
    static T: OnceLock<Table> = OnceLock::new();
    T.get_or_init(|| {
        let v: Value = serde_json::from_str(include_str!("../assets/file-icons/map.json")).expect("assets/file-icons/map.json");
        let one = |k: &str| v[k].as_str().unwrap_or("file").to_string();
        Table { file: one("file"), folder: one("folder"), folder_open: one("folderOpen"), folders: strings(&v["folders"]),
            open_of: strings(&v["openOf"]), names: strings(&v["names"]), ext: strings(&v["ext"]) }
    })
}

/// The icon (a name in assets/file-icons) for a file or folder name: by full name first, then by the longest extension
/// ("a.test.ts" -> "test.ts" -> "ts"); folders by name, open or closed.
pub fn material_name(name: &str, dir: bool, open: bool) -> String {
    let (t, lower) = (table(), name.to_lowercase());
    if dir {
        let closed = t.folders.get(&lower).unwrap_or(&t.folder);
        return if open { t.open_of.get(closed).unwrap_or(&t.folder_open) } else { closed }.clone();
    }
    if let Some(n) = t.names.get(&lower) { return n.clone() }
    let parts: Vec<&str> = lower.split('.').collect();
    (1..parts.len()).find_map(|i| t.ext.get(&parts[i..].join("."))).unwrap_or(&t.file).clone()
}

/// The icon, 14 px, drawn in its own colours.
pub fn file_icon(name: &str, dir: bool, open: bool) -> Img {
    img(format!("file-icons/{}.svg", material_name(name, dir, open))).size(px(14.)).flex_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material_icons::MATERIAL;

    fn exists(n: &str) -> bool { MATERIAL.binary_search_by_key(&n, |(k, _)| k).is_ok() }

    #[test]
    fn files_match_by_name_then_by_longest_extension() {
        assert_eq!(material_name("Dockerfile", false, false), "docker");
        assert_eq!(material_name("main.rs", false, false), "rust");
        assert_eq!(material_name("app.component.spec.ts", false, false), material_name("x.spec.ts", false, false));
        assert_eq!(material_name("noextension_zzz", false, false), "file");
    }

    #[test]
    fn folders_have_a_name_and_an_open_look() {
        let (closed, open) = (material_name("src", true, false), material_name("src", true, true));
        assert_ne!(closed, material_name("zzz-unknown", true, false));
        assert_ne!(closed, open);
        assert_eq!(material_name("zzz-unknown", true, false), "folder");
    }

    #[test]
    fn every_icon_the_table_names_is_embedded() {
        let t = table();
        let all = t.names.values().chain(t.ext.values()).chain(t.folders.values()).chain(t.open_of.values()).chain([&t.file, &t.folder, &t.folder_open]);
        let missing: Vec<&String> = all.filter(|n| !exists(n)).collect();
        assert!(missing.is_empty(), "icons named in map.json but not embedded: {missing:?}");
    }
}
