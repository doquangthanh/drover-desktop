//! Ctrl+F: find text in the focused terminal's screen and history (case-insensitive, literal). Typing jumps to the
//! newest match, Enter to the next older one; the match is selected (painted like a selection) and scrolled into view.

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Direction, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::search::RegexSearch;

use crate::workspace_app::DroverApp;

/// `text` as a regex that matches it literally, ignoring case.
fn pattern(text: &str) -> String {
    let mut out = String::from("(?i)");
    for c in text.chars() {
        if "\\.+*?()|[]{}^$#&-~".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

impl DroverApp {
    /// `restart`: search again from the bottom (the text changed); else go on to the next older match.
    pub fn find_step(&mut self, restart: bool) {
        let Some((pane, text)) = self.picker.as_ref().map(|p| (p.pane.clone(), p.query.clone())) else { return };
        let Some(pane) = pane else { return };
        let mut screens = self.screens.lock().unwrap();
        let Some(screen) = screens.get_mut(&pane) else { return };
        let term = screen.term_mut();
        if text.trim().is_empty() {
            term.selection = None;
            drop(screens);
            self.set_find_note(None);
            return;
        }
        let Ok(mut regex) = RegexSearch::new(&pattern(&text)) else { return };
        let bottom = Point::new(Line(term.screen_lines() as i32 - 1), Column(term.columns().saturating_sub(1)));
        let origin = match (&term.selection, restart) {
            (Some(sel), false) => sel.to_range(term).map(|r| r.start).map(|p| {
                // Just before the current match, so the search moves on.
                if p.column.0 > 0 { Point::new(p.line, Column(p.column.0 - 1)) } else { Point::new(p.line - 1, Column(term.columns() - 1)) }
            }).unwrap_or(bottom),
            _ => bottom,
        };
        let found = term.search_next(&mut regex, origin, Direction::Left, Side::Left, None);
        let note = match found {
            Some(m) => {
                let mut sel = Selection::new(SelectionType::Simple, *m.start(), Side::Left);
                sel.update(*m.end(), Side::Right);
                term.selection = Some(sel);
                term.scroll_to_point(*m.start());
                None
            }
            None => {
                term.selection = None;
                Some(format!("No match for “{text}”."))
            }
        };
        drop(screens);
        self.set_find_note(note);
    }

    fn set_find_note(&mut self, note: Option<String>) {
        if let Some(p) = self.picker.as_mut() {
            p.note = note;
        }
    }
}
