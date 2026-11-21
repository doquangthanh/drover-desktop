//! Copy mode (Herdr's `prefix+[`; here Ctrl+K [): move a cursor over the focused pane's screen and scrollback with
//! the keyboard, select, and copy. h j k l / arrows move, w b e by word, 0 ^ $ along the line, g G to the top and
//! bottom, Ctrl+U / Ctrl+D and PageUp / PageDown by page, v selects characters, V lines, y or Enter copies (with
//! nothing selected, the line), Escape or q leaves. The cursor and selection are the terminal's own selection, so
//! the highlight is painted like a mouse selection.

use alacritty_terminal::grid::{Dimensions, Grid, Scroll};
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::cell::Cell;
use gpui::{Context, KeyDownEvent};

use crate::workspace_app::DroverApp;

pub struct CopyMode {
    pane: String,
    cursor: Point,
    anchor: Option<Point>,
    lines: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Class { Space, Word, Other }

fn class(g: &Grid<Cell>, p: Point) -> Class {
    let c = g[p].c;
    if c.is_whitespace() || c == '\0' { Class::Space } else if c.is_alphanumeric() || c == '_' { Class::Word } else { Class::Other }
}

fn next(g: &Grid<Cell>, p: Point) -> Option<Point> {
    if p.column.0 + 1 < g.columns() { Some(Point::new(p.line, Column(p.column.0 + 1))) }
    else if p.line < g.bottommost_line() { Some(Point::new(Line(p.line.0 + 1), Column(0))) }
    else { None }
}

fn prev(g: &Grid<Cell>, p: Point) -> Option<Point> {
    if p.column.0 > 0 { Some(Point::new(p.line, Column(p.column.0 - 1))) }
    else if p.line > g.topmost_line() { Some(Point::new(Line(p.line.0 - 1), g.last_column())) }
    else { None }
}

/// `w`: the start of the next word.
fn word_forward(g: &Grid<Cell>, mut p: Point) -> Point {
    let c0 = class(g, p);
    if c0 != Class::Space {
        loop {
            match next(g, p) { Some(n) => { p = n; if class(g, p) != c0 { break } } None => return p }
        }
    }
    while class(g, p) == Class::Space {
        match next(g, p) { Some(n) => p = n, None => return p }
    }
    p
}

/// `b`: the start of this or the previous word.
fn word_back(g: &Grid<Cell>, mut p: Point) -> Point {
    match prev(g, p) { Some(n) => p = n, None => return p }
    while class(g, p) == Class::Space {
        match prev(g, p) { Some(n) => p = n, None => return p }
    }
    let c0 = class(g, p);
    while let Some(n) = prev(g, p) {
        if class(g, n) != c0 { break }
        p = n;
    }
    p
}

/// `e`: the end of this or the next word.
fn word_end(g: &Grid<Cell>, mut p: Point) -> Point {
    match next(g, p) { Some(n) => p = n, None => return p }
    while class(g, p) == Class::Space {
        match next(g, p) { Some(n) => p = n, None => return p }
    }
    let c0 = class(g, p);
    while let Some(n) = next(g, p) {
        if class(g, n) != c0 { break }
        p = n;
    }
    p
}

fn first_non_blank(g: &Grid<Cell>, line: Line) -> Column {
    (0..g.columns()).find(|c| class(g, Point::new(line, Column(*c))) != Class::Space).map(Column).unwrap_or(Column(0))
}

impl DroverApp {
    /// Ctrl+K [: start at the pane's cursor, showing the end of the output.
    pub fn enter_copy_mode(&mut self, cx: &mut Context<Self>) {
        let Some(pane) = self.focused.clone() else { return };
        let cursor = {
            let mut screens = self.screens.lock().unwrap();
            let Some(screen) = screens.get_mut(&pane) else { return };
            screen.term_mut().scroll_display(Scroll::Bottom);
            screen.term().grid().cursor.point
        };
        self.copy_mode = Some(CopyMode { pane, cursor, anchor: None, lines: false });
        self.copy_move(cx, |_, p, _| p);
    }

    /// A key while copy mode is on: it never reaches the agent.
    pub fn copy_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let m = ev.keystroke.modifiers;
        let key = ev.keystroke.key.as_str();
        // Named keys by name; letters and symbols as typed ("G", "$"), so Shift+g and Shift+4 work.
        let typed = match key {
            "escape" | "enter" | "left" | "right" | "up" | "down" | "home" | "end" | "pageup" | "pagedown" => key.to_string(),
            _ if m.control => key.to_string(),
            _ => ev.keystroke.key_char.clone().unwrap_or_else(|| if m.shift { key.to_uppercase() } else { key.to_string() }),
        };
        let ctrl = m.control;
        let (mut exit, mut yank) = (false, false);
        let Some(st) = self.copy_mode.as_mut() else { return };
        match (typed.as_str(), ctrl) {
            ("escape" | "q", _) => exit = true,
            ("y" | "enter", _) => { yank = true; exit = true; }
            ("v", false) => { st.anchor = if st.anchor.is_some() && !st.lines { None } else { Some(st.cursor) }; st.lines = false; }
            ("V", false) => { st.anchor = if st.anchor.is_some() && st.lines { None } else { Some(st.cursor) }; st.lines = true; }
            _ => {}
        }
        if exit {
            self.leave_copy_mode(yank, cx);
            return;
        }
        let typed = typed.clone();
        self.copy_move(cx, move |g, mut p, rows| {
            let half = (rows / 2).max(1);
            let (top, bottom, last) = (g.topmost_line(), g.bottommost_line(), g.last_column());
            let line = |n: i32| Line(n.clamp(top.0, bottom.0));
            match (typed.as_str(), ctrl) {
                ("h" | "left", false) => p.column = Column(p.column.0.saturating_sub(1)),
                ("l" | "right", false) => p.column = Column((p.column.0 + 1).min(last.0)),
                ("k" | "up", false) => p.line = line(p.line.0 - 1),
                ("j" | "down", false) => p.line = line(p.line.0 + 1),
                ("u", true) => p.line = line(p.line.0 - half),
                ("d", true) => p.line = line(p.line.0 + half),
                ("pageup", _) => p.line = line(p.line.0 - rows),
                ("pagedown", _) => p.line = line(p.line.0 + rows),
                ("0" | "home", false) => p.column = Column(0),
                ("$" | "end", false) => p.column = last,
                ("^", false) => p.column = first_non_blank(g, p.line),
                ("g", false) => p = Point::new(top, Column(0)),
                ("G", false) => p = Point::new(bottom, Column(0)),
                ("w", false) => p = word_forward(g, p),
                ("b", false) => p = word_back(g, p),
                ("e", false) => p = word_end(g, p),
                _ => {}
            }
            p
        });
    }

    /// Move the cursor with `step(grid, cursor, rows)`, keep it on screen and repaint the selection.
    fn copy_move(&mut self, cx: &mut Context<Self>, step: impl FnOnce(&Grid<Cell>, Point, i32) -> Point) {
        let Some(st) = self.copy_mode.as_mut() else { return };
        let mut screens = self.screens.lock().unwrap();
        let Some(screen) = screens.get_mut(&st.pane) else { drop(screens); self.copy_mode = None; return };
        let term = screen.term_mut();
        let rows = term.screen_lines() as i32;
        let p = step(term.grid(), st.cursor, rows);
        st.cursor = p;
        let (top, bottom) = (-(term.grid().display_offset() as i32), rows - 1 - term.grid().display_offset() as i32);
        if p.line.0 < top { term.scroll_display(Scroll::Delta(top - p.line.0)); }
        else if p.line.0 > bottom { term.scroll_display(Scroll::Delta(bottom - p.line.0)); }
        term.selection = Some(match st.anchor {
            None => { let mut s = Selection::new(SelectionType::Simple, p, Side::Left); s.update(p, Side::Right); s }
            Some(a) if st.lines => { let mut s = Selection::new(SelectionType::Lines, a, Side::Left); s.update(p, Side::Right); s }
            Some(a) => {
                // Keep both end cells inside the highlight whichever way the selection runs.
                let (sa, sp) = if p >= a { (Side::Left, Side::Right) } else { (Side::Right, Side::Left) };
                let mut s = Selection::new(SelectionType::Simple, a, sa);
                s.update(p, sp);
                s
            }
        });
        cx.notify();
    }

    fn leave_copy_mode(&mut self, yank: bool, cx: &mut Context<Self>) {
        let Some(st) = self.copy_mode.take() else { return };
        if yank {
            if st.anchor.is_none() {
                // Nothing selected: copy the cursor's line.
                if let Some(screen) = self.screens.lock().unwrap().get_mut(&st.pane) {
                    let mut s = Selection::new(SelectionType::Lines, st.cursor, Side::Left);
                    s.update(st.cursor, Side::Right);
                    screen.term_mut().selection = Some(s);
                }
            }
            self.copy_selection(cx);
        }
        if let Some(screen) = self.screens.lock().unwrap().get_mut(&st.pane) {
            screen.term_mut().selection = None;
            screen.term_mut().scroll_display(Scroll::Bottom);
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(lines: &[&str]) -> Grid<Cell> {
        let mut g = Grid::<Cell>::new(lines.len(), 12, 0);
        for (l, text) in lines.iter().enumerate() {
            for (c, ch) in text.chars().enumerate() {
                g[Line(l as i32)][Column(c)].c = ch;
            }
        }
        g
    }

    fn at(line: i32, col: usize) -> Point { Point::new(Line(line), Column(col)) }

    #[test]
    fn words_move_forward_back_and_to_the_end() {
        let g = grid(&["foo bar_b.x", "  qux"]);
        assert_eq!(word_forward(&g, at(0, 0)), at(0, 4)); // foo -> bar_b
        assert_eq!(word_forward(&g, at(0, 4)), at(0, 9)); // bar_b -> .
        assert_eq!(word_forward(&g, at(0, 10)), at(1, 2)); // x -> next line's qux, over the blanks
        assert_eq!(word_back(&g, at(0, 6)), at(0, 4));
        assert_eq!(word_back(&g, at(0, 4)), at(0, 0));
        assert_eq!(word_end(&g, at(0, 0)), at(0, 2));
        assert_eq!(word_end(&g, at(0, 2)), at(0, 8));
    }

    #[test]
    fn first_non_blank_skips_leading_spaces() {
        let g = grid(&["  qux", ""]);
        assert_eq!(first_non_blank(&g, Line(0)), Column(2));
        assert_eq!(first_non_blank(&g, Line(1)), Column(0));
    }
}
