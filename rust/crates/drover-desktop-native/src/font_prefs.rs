//! This device's own terminal font (Settings → Appearance), as the web UI keeps it per browser: a family and a size
//! that replace what the daemon reports for the system terminal. Empty family / size 0 = follow the daemon.
//! Saved in `native-terminal-font.json` in the state folder (not in settings.toml: it belongs to this computer).

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OwnFont {
    pub family: String,
    pub size: f32,
}

pub fn path() -> PathBuf {
    drover_proto::client::state_dir().join("native-terminal-font.json")
}

pub fn parse(text: &str) -> OwnFont {
    let v: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    OwnFont {
        family: v["family"].as_str().unwrap_or("").trim().to_string(),
        size: v["size"].as_f64().filter(|s| (8.0..=40.0).contains(s)).unwrap_or(0.0) as f32,
    }
}

pub fn load_from(path: &Path) -> OwnFont {
    std::fs::read_to_string(path).map(|t| parse(&t)).unwrap_or_default()
}

/// Save, or remove the file when nothing is overridden. Errors are reported to the caller (shown as a toast).
pub fn save_to(path: &Path, font: &OwnFont) -> Result<(), String> {
    if font.family.is_empty() && font.size == 0.0 {
        return match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        };
    }
    std::fs::write(path, json!({"family": font.family, "size": font.size}).to_string()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_family_and_size_and_ignores_nonsense() {
        assert_eq!(parse(r#"{"family":" Fira Code ","size":14}"#), OwnFont { family: "Fira Code".into(), size: 14.0 });
        assert_eq!(parse(r#"{"family":"X","size":99}"#), OwnFont { family: "X".into(), size: 0.0 });
        assert_eq!(parse("not json"), OwnFont::default());
    }

    #[test]
    fn saves_loads_and_clears() {
        let dir = std::env::temp_dir().join(format!("drover-font-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("f.json");
        let font = OwnFont { family: "JetBrains Mono".into(), size: 15.0 };
        save_to(&file, &font).unwrap();
        assert_eq!(load_from(&file), font);
        save_to(&file, &OwnFont::default()).unwrap();
        assert!(!file.exists());
        save_to(&file, &OwnFont::default()).unwrap(); // already gone: not an error
        let _ = std::fs::remove_dir_all(dir);
    }
}
