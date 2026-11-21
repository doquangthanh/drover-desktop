//! What's new (Herdr's "What's new" frame): the changelog of drover-core, with the entry of the version you run first.
//! Fetched from GitHub when the screen opens (a public file; nothing about you is sent), so it also lists what is newer.

use gpui::{Context, Div, FontWeight, MouseButton, div, prelude::*, rgb};
use serde_json::json;

use crate::actions::open_in_browser;
use crate::theme::{self, dim, fg, muted};
use crate::ui_scale::px;
use crate::views::ui::{self, Kind};
use crate::workspace_app::DroverApp;

const CHANGELOG_RAW: &str = "https://raw.githubusercontent.com/doquangthanh/drover-core/main/docs/project-changelog.md";
const CHANGELOG_PAGE: &str = "https://github.com/doquangthanh/drover-core/blob/main/docs/project-changelog.md";
/// How many entries get a tab.
const TABS: usize = 8;

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Heading(String),
    Bullet(String),
    /// An indented `- ` line under a bullet.
    Sub(String),
    Text(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub title: String,
    pub items: Vec<Item>,
}

/// Split `docs/project-changelog.md` into its `## ` entries ("Unreleased", "0.1.48"...); a wrapped bullet is joined.
pub fn parse_changelog(text: &str) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    for line in text.lines() {
        if let Some(title) = line.strip_prefix("## ") {
            sections.push(Section { title: title.trim().to_string(), items: vec![] });
            continue;
        }
        let Some(section) = sections.last_mut() else { continue };
        if let Some(t) = line.strip_prefix("### ") {
            section.items.push(Item::Heading(t.trim().to_string()));
        } else if let Some(t) = line.strip_prefix("- ") {
            section.items.push(Item::Bullet(t.trim().to_string()));
        } else if line.trim().is_empty() {
        } else if line.starts_with(' ') && line.trim_start().starts_with("- ") {
            section.items.push(Item::Sub(line.trim_start()[2..].trim().to_string()));
        } else if line.starts_with(' ') {
            match section.items.last_mut() {
                Some(Item::Bullet(t) | Item::Sub(t) | Item::Text(t)) => { t.push(' '); t.push_str(line.trim()); }
                _ => section.items.push(Item::Text(line.trim().to_string())),
            }
        } else {
            section.items.push(Item::Text(line.trim().to_string()));
        }
    }
    sections
}

/// The entry of `version` ("0.1.48" or "v0.1.48"), else the first one.
pub fn pick(sections: &[Section], version: &str) -> usize {
    let v = version.trim_start_matches('v');
    sections.iter().position(|s| s.title == v).unwrap_or(0)
}

#[derive(Default)]
pub struct WhatsNewState {
    pub sections: Option<Result<Vec<Section>, String>>,
    /// The daemon's version (`server.info`), to open on the right entry.
    pub version: Option<String>,
    /// The tab picked by hand; None = the entry of the running version.
    pub selected: Option<usize>,
    asked: bool,
}

/// Load the changelog and the running version the first time the screen is shown.
pub fn refresh(app: &mut DroverApp) {
    if app.whats_new.asked { return }
    app.whats_new.asked = true;
    app.rpc_reply("server.info", json!({}), |app, r| {
        app.whats_new.version = r.ok().and_then(|v| v["version"].as_str().map(String::from));
    });
    app.http_get(CHANGELOG_RAW, |app, r| app.whats_new.sections = Some(r.map(|t| parse_changelog(&t))));
}

pub fn body(app: &DroverApp, cx: &mut Context<DroverApp>) -> Div {
    let sections = match &app.whats_new.sections {
        None => return ui::note("Loading the changelog…", false),
        Some(Err(e)) => return error_view(e, cx),
        Some(Ok(s)) if s.is_empty() => return error_view("The changelog is empty.", cx),
        Some(Ok(s)) => s,
    };
    let version = app.whats_new.version.clone().unwrap_or_default();
    let current = pick(sections, &version);
    let shown = app.whats_new.selected.filter(|i| *i < sections.len()).unwrap_or(current);
    let mut tabs = div().flex().flex_wrap().gap(px(4.));
    for (i, s) in sections.iter().enumerate().take(TABS.max(current + 1)) {
        let label = if i == current && !version.is_empty() { format!("{} · you", s.title) } else { s.title.clone() };
        tabs = tabs.child(ui::tab(&label, i == shown).on_mouse_down(MouseButton::Left, cx.listener(move |app, _, _, cx| {
            app.whats_new.selected = Some(i);
            cx.notify();
        })));
    }
    let head = div().flex().flex_none().items_center().gap(px(10.)).px(px(14.)).py(px(8.)).border_b_1().border_color(theme::border())
        .child(div().flex_1().min_w_0().child(tabs))
        .child(div().text_size(px(12.)).text_color(rgb(dim())).child(if version.is_empty() { String::new() } else { format!("You are on v{version}") }))
        .child(ui::button("Open on GitHub", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(|_, _, _, _| open_in_browser(CHANGELOG_PAGE))));
    let mut list = div().flex().flex_col().gap(px(8.)).p(px(18.)).max_w(px(820.));
    for item in &sections[shown].items {
        list = list.child(match item {
            Item::Heading(t) => div().pt(px(8.)).text_size(px(11.)).font_weight(FontWeight::SEMIBOLD).text_color(rgb(dim())).child(t.to_uppercase()),
            Item::Bullet(t) => bullet("•", 0., t),
            Item::Sub(t) => bullet("–", 20., t),
            Item::Text(t) => div().text_color(rgb(muted())).child(t.clone()),
        });
    }
    if sections[shown].items.is_empty() {
        list = list.child(div().text_color(rgb(dim())).child("Nothing listed for this entry."));
    }
    div().flex().flex_col().flex_1().min_h_0().child(head)
        .child(div().flex().flex_col().flex_1().min_h_0().child(div().id("whats-new").size_full().overflow_y_scroll().child(list)))
}

/// A line with its marker on the left and the text wrapping under itself: a plain block (not a flex row), so the
/// wrapped height is measured at the width it is painted at.
fn bullet(marker: &str, indent: f32, text: &str) -> Div {
    div().relative().w_full().pl(px(indent + 16.)).text_color(rgb(fg()))
        .child(div().absolute().left(px(indent + 2.)).text_color(rgb(muted())).child(marker.to_string()))
        .child(text.to_string())
}

fn error_view(e: &str, cx: &mut Context<DroverApp>) -> Div {
    div().flex().flex_col().flex_1().items_center().justify_center().gap(px(10.))
        .child(div().text_color(rgb(theme::error())).child(format!("Could not load the changelog: {e}")))
        .child(ui::button("Open on GitHub", Kind::Plain).on_mouse_down(MouseButton::Left, cx.listener(|_, _, _, _| open_in_browser(CHANGELOG_PAGE))))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# Changelog\n\n## Unreleased\n### Added\n- First thing\n  that wraps.\n  - a sub item\n    that wraps too\n- Second\n### Fixed\n- A bug\n\n## 0.1.48\n- Old\n";

    #[test]
    fn splits_into_entries_with_headings_and_wrapped_bullets() {
        let s = parse_changelog(SAMPLE);
        assert_eq!(s.iter().map(|s| s.title.as_str()).collect::<Vec<_>>(), vec!["Unreleased", "0.1.48"]);
        assert_eq!(s[0].items, vec![Item::Heading("Added".into()), Item::Bullet("First thing that wraps.".into()), Item::Sub("a sub item that wraps too".into()), Item::Bullet("Second".into()),
                                    Item::Heading("Fixed".into()), Item::Bullet("A bug".into())]);
    }

    #[test]
    fn opens_on_the_running_version_else_the_first_entry() {
        let s = parse_changelog(SAMPLE);
        assert_eq!(pick(&s, "0.1.48"), 1);
        assert_eq!(pick(&s, "v0.1.48"), 1);
        assert_eq!(pick(&s, "9.9.9"), 0);
        assert_eq!(pick(&s, ""), 0);
    }
}
