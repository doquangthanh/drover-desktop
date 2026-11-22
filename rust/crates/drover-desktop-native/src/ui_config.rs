//! How the sidebar rows and the window title look, set in keys.toml's `[desktop_ui]` table:
//!
//! ```toml
//! [desktop_ui]
//! space_row = "avatar name branch dirty state"   # any of these, in any order
//! agent_row = "icon name age state"
//! window_title = "{agent} · {space} — Drover"     # {space} {tab} {agent} {state}
//! scroll_lines = 3                              # lines per mouse wheel notch (default 3)
//! ```
//!
//! What is left out is not shown. Saving keys.toml, or the "Reload config" command, applies it.

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpaceCol { Avatar, Name, Branch, Dirty, State }

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AgentCol { Icon, Name, Age, State }

#[derive(Clone, Debug, PartialEq)]
pub struct UiConfig {
    pub space_row: Vec<SpaceCol>,
    pub agent_row: Vec<AgentCol>,
    pub window_title: String,
    /// Lines one notch of the mouse wheel scrolls (fast turning goes further).
    pub scroll_lines: f32,
}

impl Default for UiConfig {
    fn default() -> Self {
        use {AgentCol as A, SpaceCol as S};
        Self { space_row: vec![S::Avatar, S::Name, S::Branch, S::Dirty, S::State], agent_row: vec![A::Icon, A::Name, A::Age, A::State], window_title: "Drover".into(), scroll_lines: 3.0 }
    }
}

/// The words of `value`, each looked up in `names`; the ones that match nothing go to `problems`.
fn words<T: Copy>(key: &str, value: &str, names: &[(&str, T)], problems: &mut Vec<String>) -> Vec<T> {
    let mut out = vec![];
    for w in value.split_whitespace() {
        match names.iter().find(|(n, _)| *n == w.to_lowercase()) {
            Some(&(_, v)) => out.push(v),
            None => problems.push(format!("keys.toml: {key} has no \"{w}\" (use {})", names.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", "))),
        }
    }
    out
}

/// The `[desktop_ui]` table of keys.toml text, and a line for each thing that could not be used.
pub fn parse(text: &str) -> (UiConfig, Vec<String>) {
    use {AgentCol as A, SpaceCol as S};
    let (mut ui, mut problems, mut inside) = (UiConfig::default(), vec![], true); // lines before the first [section] count too: a bare `scroll_lines = 2` works
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        if let Some(section) = line.strip_prefix('[') {
            inside = section.trim_end_matches(']').trim() == "desktop_ui";
            continue;
        }
        let (true, Some((key, value))) = (inside, line.split_once('=')) else { continue };
        let raw = value.trim().trim_matches('"').to_string();
        let value = value.split('"').nth(1).unwrap_or("");
        match key.trim() {
            "space_row" => ui.space_row = words("space_row", value, &[("avatar", S::Avatar), ("name", S::Name), ("branch", S::Branch), ("dirty", S::Dirty), ("state", S::State)], &mut problems),
            "agent_row" => ui.agent_row = words("agent_row", value, &[("icon", A::Icon), ("name", A::Name), ("age", A::Age), ("state", A::State)], &mut problems),
            "scroll_lines" => match raw.parse::<f32>() {
                Ok(n) if (0.5..=50.0).contains(&n) => ui.scroll_lines = n,
                _ => problems.push("keys.toml: scroll_lines is a number from 0.5 to 50".into()),
            },
            "window_title" if !value.trim().is_empty() => ui.window_title = value.to_string(),
            other => problems.push(format!("keys.toml: [desktop_ui] has no \"{other}\" (use space_row, agent_row, window_title, scroll_lines)")),
        }
    }
    (ui, problems)
}

/// The title with `{space}`, `{tab}`, `{agent}` and `{state}` filled in; separators left dangling by an empty one are trimmed.
pub fn title(format: &str, space: &str, tab: &str, agent: &str, state: &str) -> String {
    let t = format.replace("{space}", space).replace("{tab}", tab).replace("{agent}", agent).replace("{state}", state);
    let t = t.trim_matches(|c: char| c.is_whitespace() || matches!(c, '-' | '·' | '|' | '—' | ':'));
    if t.is_empty() { "Drover".into() } else { t.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_rows_and_title() {
        let text = "[keys]\nx = \"y\"\n[desktop_ui]\nspace_row = \"Name state\"\nagent_row = \"name\"\nwindow_title = \"{agent} · {space}\"\n";
        let (ui, problems) = parse(text);
        assert!(problems.is_empty());
        assert_eq!(ui.space_row, vec![SpaceCol::Name, SpaceCol::State]);
        assert_eq!(ui.agent_row, vec![AgentCol::Name]);
        assert_eq!(ui.window_title, "{agent} · {space}");
    }

    #[test]
    fn missing_table_keeps_the_defaults_and_typos_are_reported() {
        assert_eq!(parse("[keys]\na = \"b\"\n").0, UiConfig::default());
        let (_, problems) = parse("[desktop_ui]\nspace_row = \"name colour\"\nwhat = \"x\"\n");
        assert_eq!(problems.len(), 2);
    }

    #[test]
    fn title_fills_in_and_trims_empty_parts() {
        assert_eq!(title("{agent} · {space}", "api", "main", "claude", "working"), "claude · api");
        assert_eq!(title("{agent} · {space}", "api", "main", "", "idle"), "api");
        assert_eq!(title("{agent}", "", "", "", ""), "Drover");
    }
}

#[cfg(test)]
mod bare_tests {
    use super::*;

    #[test]
    fn a_line_outside_any_table_still_counts() {
        assert_eq!(parse("scroll_lines = 1").0.scroll_lines, 1.0);
        assert_eq!(parse("scroll_lines = 2\n[desktop_keys]\nnew_tab = \"f5\"\n").0.scroll_lines, 2.0);
        // ...but not lines that belong to another table.
        assert_eq!(parse("[keys]\nscroll_lines = 9\n").0.scroll_lines, 3.0);
    }
}
