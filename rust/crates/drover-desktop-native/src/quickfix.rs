//! "Check errors" (the web UI's quickfix): the file:line references in the output of the failed post-run checks
//! (`check.list`) of the focused pane's folder, in the picker; Enter opens the file there in the editor.

use gpui::{Context, Window};
use serde_json::{Value, json};

use crate::picker::Kind;
use crate::workspace_app::DroverApp;

#[derive(Clone, Debug, PartialEq)]
pub struct FixItem {
    pub step: String,
    pub path: String,
    pub line: Option<usize>,
    pub col: Option<usize>,
    pub text: String,
}

const MAX_ITEMS: usize = 200;

/// One item per distinct file:line:col a step that did not pass printed; paths outside `cwd` are left out.
pub fn items(results: &[Value], cwd: &str) -> Vec<FixItem> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for r in results {
        let (status, output) = (r["status"].as_str().unwrap_or(""), r["output"].as_str().unwrap_or(""));
        if status == "passed" || output.is_empty() { continue }
        for line in output.lines() {
            let chars: Vec<char> = line.chars().collect();
            for f in crate::path_refs::find_path_refs(&chars) {
                let Some(path) = crate::path_refs::to_space_path(&f.path, cwd, cwd) else { continue };
                if !seen.insert(format!("{path}:{:?}:{:?}", f.line, f.col)) { continue }
                out.push(FixItem { step: r["name"].as_str().unwrap_or("").into(), path, line: f.line.map(|l| l as usize),
                                   col: f.col.map(|c| c as usize), text: line.trim().to_string() });
                if out.len() >= MAX_ITEMS { return out }
            }
        }
    }
    out
}

impl DroverApp {
    /// Open the picker on the check errors of the focused pane's folder (filled when check.list answers).
    pub fn open_quickfix(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(sc) = crate::views::panel::scope(self) else { return self.toasts.error("Open a space first".into()) };
        self.open_picker(Kind::Quickfix, window, cx);
        let cwd = sc.cwd.clone();
        let params = crate::views::changes::with_machine(self, json!({"cwd": sc.cwd}));
        self.rpc_reply("check.list", params, move |app, r| {
            let found = match r {
                Ok(v) => items(v["results"].as_array().map(Vec::as_slice).unwrap_or(&[]), &cwd),
                Err(e) => { app.toasts.error(format!("Checks: {e}")); vec![] }
            };
            if let Some(p) = app.picker.as_mut().filter(|p| p.kind == Kind::Quickfix) {
                p.note = found.is_empty().then(|| "No file:line in failed checks. Run the checks in Changes first.".to_string());
                p.fixes = found;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_lines_of_failed_steps_only_once_and_inside_the_folder() {
        let results = vec![
            json!({"name": "test", "status": "failed", "output": "error at src/a.rs:4:2\n  --> src/a.rs:4:2\nsee /etc/x.rs:1"}),
            json!({"name": "lint", "status": "passed", "output": "src/b.rs:9: fine"}),
            json!({"name": "tsc", "status": "failed", "output": "src/c.ts(7,3): error TS1"}),
        ];
        let got = items(&results, "/w/p");
        assert_eq!(got.iter().map(|i| (i.path.as_str(), i.line)).collect::<Vec<_>>(), vec![("src/a.rs", Some(4)), ("src/c.ts", Some(7))]);
        assert_eq!(got[0].step, "test");
        assert_eq!(got[0].col, Some(2));
    }
}
