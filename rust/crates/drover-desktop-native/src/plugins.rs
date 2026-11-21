//! Plugins (Herdr's plugins, small): a folder in `<state dir>/plugins/` with a `drover-plugin.toml` and the commands it
//! runs. There is no SDK: a plugin drives Drover through the `dr` command (`dr ls`, `dr send`, `dr read`, `dr run`…).
//!
//! ```toml
//! id = "example.notes"            # letters, digits . _ - :
//! name = "Notes"
//!
//! [[actions]]                     # in the command palette, and bindable in keys.toml as "example.notes.save"
//! id = "save"                     # no dots
//! title = "Save the screen to notes"
//! command = ["bash", "save.sh"]   # an argv list, run in the plugin's folder, not through a shell
//!
//! [[events]]                      # runs by itself when an agent changes state
//! on = "agent.done"               # agent.working | blocked | done | idle | exited
//! command = ["node", "notify.js"]
//! ```
//!
//! A command gets `DROVER_PLUGIN_ID`, `DROVER_PLUGIN_DIR`, `DROVER_EVENT` ("action" or the event), and when there is
//! one `DROVER_PANE`, `DROVER_PANE_NAME`, `DROVER_STATE`, `DROVER_TAB`, `DROVER_SPACE`, `DROVER_CWD`.
//! A plugin is code that runs as you: Drover does not sandbox it.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, channel};

use serde_json::Value;

pub const MANIFEST: &str = "drover-plugin.toml";

#[derive(Clone, Debug, PartialEq)]
pub struct Action {
    pub id: String,
    pub title: String,
    pub command: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Hook {
    pub on: String,
    pub command: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Plugin {
    pub id: String,
    pub name: String,
    pub dir: PathBuf,
    pub actions: Vec<Action>,
    pub hooks: Vec<Hook>,
}

pub fn dir() -> PathBuf {
    drover_proto::client::state_dir().join("plugins")
}

fn argv(v: Option<&toml::Value>) -> Option<Vec<String>> {
    let list: Vec<String> = v?.as_array()?.iter().filter_map(|x| x.as_str().map(String::from)).collect();
    (!list.is_empty() && list.iter().all(|s| !s.is_empty())).then_some(list)
}

fn text<'a>(t: &'a toml::Table, key: &str) -> Option<&'a str> {
    t.get(key).and_then(toml::Value::as_str).filter(|s| !s.trim().is_empty())
}

fn id_ok(id: &str, dots: bool) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ':') || (dots && c == '.'))
}

/// One manifest; the error says what to fix.
pub fn parse(dir: &Path, manifest: &str) -> Result<Plugin, String> {
    let t = manifest.parse::<toml::Table>().map_err(|e| format!("{MANIFEST}: {}", e.to_string().lines().last().unwrap_or("not valid TOML")))?;
    let id = text(&t, "id").filter(|i| id_ok(i, true)).ok_or_else(|| "needs an id (letters, digits and . _ - :)".to_string())?.to_string();
    let name = text(&t, "name").unwrap_or(&id).to_string();
    let mut plugin = Plugin { id, name, dir: dir.to_path_buf(), actions: vec![], hooks: vec![] };
    for (i, a) in t.get("actions").and_then(toml::Value::as_array).into_iter().flatten().filter_map(toml::Value::as_table).enumerate() {
        let aid = text(a, "id").filter(|i| id_ok(i, false)).ok_or_else(|| format!("action {} needs an id (no dots)", i + 1))?;
        let command = argv(a.get("command")).ok_or_else(|| format!("action {aid} needs command = [\"program\", \"arg\"]"))?;
        if plugin.actions.iter().any(|x| x.id == aid) { return Err(format!("two actions are called {aid}")) }
        plugin.actions.push(Action { id: aid.to_string(), title: text(a, "title").unwrap_or(aid).to_string(), command });
    }
    for (i, e) in t.get("events").and_then(toml::Value::as_array).into_iter().flatten().filter_map(toml::Value::as_table).enumerate() {
        let on = text(e, "on").ok_or_else(|| format!("event {} needs on = \"agent.done\"", i + 1))?;
        let command = argv(e.get("command")).ok_or_else(|| format!("event {on} needs command = [\"program\", \"arg\"]"))?;
        plugin.hooks.push(Hook { on: on.to_string(), command });
    }
    Ok(plugin)
}

/// Every plugin folder under `root`, by id, and a line for each folder that could not be used.
pub fn load_from(root: &Path) -> (Vec<Plugin>, Vec<String>) {
    let (mut plugins, mut problems): (Vec<Plugin>, Vec<String>) = (vec![], vec![]);
    let mut folders: Vec<PathBuf> = std::fs::read_dir(root).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    folders.sort();
    for folder in folders {
        let name = folder.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let Ok(manifest) = std::fs::read_to_string(folder.join(MANIFEST)) else { continue };
        match parse(&folder, &manifest) {
            Ok(p) if plugins.iter().any(|x| x.id == p.id) => problems.push(format!("plugin {name}: id {} is already used by another folder", p.id)),
            Ok(p) => plugins.push(p),
            Err(e) => problems.push(format!("plugin {name}: {e}")),
        }
    }
    (plugins, problems)
}

fn tail(s: &str, n: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    chars[chars.len().saturating_sub(n)..].iter().collect()
}

/// Run `argv` in `dir` and wait: its output on success, what went wrong otherwise. Call it off the UI thread.
pub fn run(argv: &[String], dir: &Path, env: &[(String, String)]) -> Result<String, String> {
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..]).current_dir(dir).envs(env.iter().cloned()).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000); // no console window
    let out = cmd.output().map_err(|e| format!("could not start {}: {e}", argv[0]))?;
    let text = |b: &[u8]| String::from_utf8_lossy(b).trim().to_string();
    if out.status.success() {
        return Ok(text(&out.stdout));
    }
    let (code, err) = (out.status.code().map(|c| c.to_string()).unwrap_or_else(|| "a signal".into()), text(&out.stderr));
    Err(if err.is_empty() { format!("{} stopped with {code}", argv[0]) } else { format!("{} stopped with {code}: {}", argv[0], tail(&err, 300)) })
}

/// `work` on its own thread; the reply channel is what `DroverApp.pending` waits on.
pub fn in_background(work: impl FnOnce() -> Result<String, String> + Send + 'static) -> Receiver<Result<Value, String>> {
    let (tx, rx) = channel();
    std::thread::spawn(move || { let _ = tx.send(work().map(Value::String)); });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "id = \"example.notes\"\nname = \"Notes\"\n[[actions]]\nid = \"save\"\ntitle = \"Save\"\ncommand = [\"bash\", \"save.sh\"]\n[[events]]\non = \"agent.done\"\ncommand = [\"node\", \"n.js\"]\n";

    #[test]
    fn reads_actions_and_events() {
        let p = parse(Path::new("x"), GOOD).unwrap();
        assert_eq!((p.id.as_str(), p.name.as_str()), ("example.notes", "Notes"));
        assert_eq!(p.actions, vec![Action { id: "save".into(), title: "Save".into(), command: vec!["bash".into(), "save.sh".into()] }]);
        assert_eq!(p.hooks, vec![Hook { on: "agent.done".into(), command: vec!["node".into(), "n.js".into()] }]);
    }

    #[test]
    fn says_what_to_fix() {
        assert!(parse(Path::new("x"), "name = \"a\"").unwrap_err().contains("needs an id"));
        assert!(parse(Path::new("x"), "id = \"a\"\n[[actions]]\nid = \"a.b\"\ncommand = [\"x\"]").unwrap_err().contains("no dots"));
        assert!(parse(Path::new("x"), "id = \"a\"\n[[actions]]\nid = \"go\"").unwrap_err().contains("needs command"));
        assert!(parse(Path::new("x"), "id = ").unwrap_err().starts_with(MANIFEST));
    }

    #[test]
    fn loads_folders_and_skips_bad_ones() {
        let root = std::env::temp_dir().join(format!("drover-plugins-test-{}", std::process::id()));
        for (folder, manifest) in [("a", GOOD), ("b", GOOD), ("c", "id = ")] {
            std::fs::create_dir_all(root.join(folder)).unwrap();
            std::fs::write(root.join(folder).join(MANIFEST), manifest).unwrap();
        }
        std::fs::create_dir_all(root.join("no-manifest")).unwrap();
        let (plugins, problems) = load_from(&root);
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(plugins.len(), 1);
        assert_eq!(problems.len(), 2);
        assert!(problems[0].contains("already used") && problems[1].starts_with("plugin c"));
    }

    #[test]
    fn a_missing_program_is_an_error_not_a_panic() {
        assert!(run(&["drover-no-such-program-xyz".into()], Path::new("."), &[]).unwrap_err().contains("could not start"));
    }
}
