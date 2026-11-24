//! Mouse selection, copy/paste and links in a pane, as the web UI's terminal (xterm) does: drag selects (double
//! click a word, triple click a line) and copies on release; Ctrl+C copies while text is selected, else it goes to
//! the agent; Ctrl+V, Ctrl+Shift+V and Shift+Insert paste; a link under the mouse is underlined, a click opens it.

use std::ops::Range;

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::TermMode;
use alacritty_terminal::term::cell::Flags;
use gpui::{ClipboardItem, Context, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Window};

use crate::terminal_paint;
use crate::workspace_app::DroverApp;

/// A link under the mouse: pane, grid line (scrollback is negative), columns, and what it opens: an address, or (with
/// Ctrl held) a file reference such as `src/a.ts:41`.
#[derive(Clone, PartialEq)]
pub struct HoverLink {
    pub pane: String,
    pub line: i32,
    pub cols: Range<usize>,
    pub url: String,
    pub file: Option<crate::path_refs::PathRef>,
}

impl DroverApp {
    /// Grid cell under `pos` in `pane`, clamped to its terminal area (a drag may leave it).
    fn cell_at(&self, pane: &str, pos: gpui::Point<Pixels>, window: &Window) -> Option<(Point, Side)> {
        let bounds = *self.term_bounds.lock().unwrap().get(pane)?;
        let cell_w = f32::from(terminal_paint::cell_width(&self.term_font, window));
        let screens = self.screens.lock().unwrap();
        let grid = screens.get(pane)?.term().grid();
        let x = f32::from(pos.x - bounds.origin.x).max(0.);
        let y = f32::from(pos.y - bounds.origin.y).max(0.);
        let col = ((x / cell_w) as usize).min(grid.columns() - 1);
        let row = ((y / self.term_font.line_height) as usize).min(grid.screen_lines() - 1);
        let side = if x - col as f32 * cell_w > cell_w / 2. { Side::Right } else { Side::Left };
        Some((Point::new(Line(row as i32 - grid.display_offset() as i32), Column(col)), side))
    }

    pub fn select_start(&mut self, pane: &str, ev: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.mouse_press(pane, ev.position, ev.modifiers, window, cx) { return } // the program tracks the mouse
        let Some((point, side)) = self.cell_at(pane, ev.position, window) else { return };
        let ty = match ev.click_count {
            2 => SelectionType::Semantic,
            n if n >= 3 => SelectionType::Lines,
            _ => SelectionType::Simple,
        };
        if let Some(screen) = self.screens.lock().unwrap().get_mut(pane) {
            screen.term_mut().selection = Some(Selection::new(ty, point, side));
        }
        self.selecting = Some(pane.to_string());
        cx.notify();
    }

    pub fn select_move(&mut self, pane: &str, ev: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.mouse_motion(pane, ev.position, ev.pressed_button == Some(MouseButton::Left), ev.modifiers, window) { return }
        let Some((point, side)) = self.cell_at(pane, ev.position, window) else { return };
        if ev.pressed_button == Some(MouseButton::Left) && self.selecting.as_deref() == Some(pane) {
            if let Some(sel) = self.screens.lock().unwrap().get_mut(pane).and_then(|s| s.term_mut().selection.as_mut()) {
                sel.update(point, side);
            }
            cx.notify();
            return;
        }
        let link = self.link_at(pane, point, ev.modifiers.control || ev.modifiers.platform);
        if link != self.hover_link {
            self.hover_link = link;
            cx.notify();
        }
    }

    /// Release: copy what was selected; a plain click (nothing selected) on a link opens it.
    pub fn select_end(&mut self, pane: &str, ev: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.mouse_release(pane, ev.position, ev.modifiers, window) { cx.notify(); return }
        self.selecting = None;
        let text = self.selected_text(pane);
        if self.take_click(pane, text.is_none(), ev.modifiers) {
            // A plain click in a program that tracks clicks (Claude Code): it went to the program.
        } else if let Some(text) = text {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        } else if let Some((point, _)) = self.cell_at(pane, ev.position, window)
            && let Some(link) = self.link_at(pane, point, ev.modifiers.control || ev.modifiers.platform)
        {
            match link.file {
                Some(file) => self.open_file_ref(pane, &file, cx),
                None => crate::actions::open_in_browser(&link.url),
            }
        }
        cx.notify();
    }

    fn selected_text(&self, pane: &str) -> Option<String> {
        let screens = self.screens.lock().unwrap();
        let term = screens.get(pane)?.term();
        term.selection.as_ref().filter(|s| !s.is_empty())?;
        term.selection_to_string().filter(|t| !t.is_empty())
    }

    /// Select the focused pane's whole screen and scrollback (Herdr's `terminal.select_all`); Ctrl+C then copies it.
    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        let Some(pane) = self.focused.clone() else { return };
        if let Some(screen) = self.screens.lock().unwrap().get_mut(&pane) {
            let term = screen.term_mut();
            let (top, bottom, last) = (term.topmost_line(), term.bottommost_line(), term.last_column());
            let mut sel = Selection::new(SelectionType::Simple, Point::new(top, Column(0)), Side::Left);
            sel.update(Point::new(bottom, last), Side::Right);
            term.selection = Some(sel);
        }
        cx.notify();
    }

    /// Ctrl+C with text selected in the focused pane: copy it and clear the selection. False: nothing selected.
    pub fn copy_selection(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(pane) = self.focused.clone() else { return false };
        let Some(text) = self.selected_text(&pane) else { return false };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        if let Some(screen) = self.screens.lock().unwrap().get_mut(&pane) {
            screen.term_mut().selection = None;
        }
        cx.notify();
        true
    }

    /// Clipboard text into the focused pane: line breaks as Enter (\r, as xterm does), wrapped in bracketed-paste
    /// markers when the program asked for them, so a multi-line paste doesn't run line by line.
    pub fn paste(&mut self, cx: &mut Context<Self>) {
        let item = cx.read_from_clipboard();
        let text = match item.as_ref().and_then(|i| i.text()) {
            Some(t) => t,
            // An image alone (a screenshot): save it as a file and paste its path, which is how agents take images.
            None => match item.as_ref().and_then(image_path_from) {
                Some(Ok(path)) => path,
                Some(Err(e)) => { self.toasts.error(e); cx.notify(); return; }
                None => return,
            },
        };
        let remote = self.focused.as_deref().and_then(|p| self.store.pane(p)).is_some_and(|p| !p.machine.is_empty() && p.machine != "local");
        if remote && item.as_ref().is_some_and(|i| i.text().is_none()) {
            self.toasts.error("Pasting an image into a remote machine's terminal is not supported yet.".into());
            cx.notify();
            return;
        }
        let text = text.replace("\r\n", "\r").replace('\n', "\r");
        let bracketed = self.focused.as_ref().is_some_and(|p| {
            self.screens.lock().unwrap().get(p).is_some_and(|s| s.term().mode().contains(TermMode::BRACKETED_PASTE))
        });
        self.send_to_focused(if bracketed { format!("\x1b[200~{text}\x1b[201~") } else { text });
        cx.notify();
    }

    /// The link that covers `point` in `pane`: an http(s) address, or, when `files` (Ctrl is held), a file reference.
    fn link_at(&self, pane: &str, point: Point, files: bool) -> Option<HoverLink> {
        let screens = self.screens.lock().unwrap();
        let grid = screens.get(pane)?.term().grid();
        let row = &grid[point.line];
        // One char per column, so an index into the text is a column.
        let cells: Vec<char> = (0..grid.columns())
            .map(|c| row[Column(c)].clone())
            .map(|cell| if cell.flags.contains(Flags::WIDE_CHAR_SPACER) || cell.c == '\0' { ' ' } else { cell.c })
            .collect();
        if let Some((cols, url)) = url_around(&cells, point.column.0) {
            return Some(HoverLink { pane: pane.to_string(), line: point.line.0, cols, url, file: None });
        }
        if !files { return None }
        let file = crate::path_refs::find_path_refs(&cells).into_iter().find(|r| (r.start..r.end).contains(&point.column.0))?;
        Some(HoverLink { pane: pane.to_string(), line: point.line.0, cols: file.start..file.end, url: String::new(), file: Some(file) })
    }

    /// Ctrl+Click on `src/a.ts:41`: open that file at that line in this pane's Files panel (the editor). A file of
    /// the space outside the pane's folder opens in the web UI instead (the panel only shows the pane's folder).
    fn open_file_ref(&mut self, pane: &str, file: &crate::path_refs::PathRef, cx: &mut Context<Self>) {
        let Some(p) = self.store.pane(pane).cloned() else { return };
        let (line, col) = (file.line.map(|l| l as usize), file.col.map(|c| c as usize));
        if let Some(rel) = crate::path_refs::to_space_path(&file.path, &p.cwd, &p.cwd) {
            self.focused = Some(p.id.clone());
            return self.open_in_files(&rel, line, col, cx);
        }
        let Some(space) = self.store.spaces.iter().find(|s| s.id == p.space_id) else { return };
        match crate::path_refs::to_space_path(&file.path, &p.cwd, &space.cwd) {
            Some(rel) => self.open_web_file(&space.id.clone(), &rel, file.line, file.col),
            None => self.toasts.error(format!("{} is outside this space", file.path)),
        }
    }
}

/// The http:// or https:// address in `cells` that covers column `col`: its columns and text.
fn url_around(cells: &[char], col: usize) -> Option<(Range<usize>, String)> {
    let text: String = cells.iter().collect();
    let mut from = 0;
    while let Some(found) = text[from..].find("http").map(|i| from + i) {
        let start = text[..found].chars().count();
        let rest: String = cells[start..].iter().take_while(|c| !c.is_whitespace() && !"<>\"'`".contains(**c)).collect();
        let url = rest.trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']', '}', '\'']);
        if (url.starts_with("http://") || url.starts_with("https://")) && url.len() > 8 {
            let end = start + url.chars().count();
            if (start..end).contains(&col) {
                return Some((start..end, url.to_string()));
            }
        }
        from = found + 4;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::url_around;

    #[test]
    fn finds_the_link_under_a_column() {
        let cells: Vec<char> = "see https://example.com/a?b=1. and http://x.io)".chars().collect();
        assert_eq!(url_around(&cells, 4), Some((4..29, "https://example.com/a?b=1".into())));
        assert_eq!(url_around(&cells, 29), None); // the trailing dot is not part of it
        assert_eq!(url_around(&cells, 40), Some((35..46, "http://x.io".into())));
        assert_eq!(url_around(&cells, 1), None);
        assert_eq!(url_around(&"http:// nothing".chars().collect::<Vec<_>>(), 2), None);
    }
}

/// The clipboard's first image written to a temp file; its path, quoted when it has spaces. None = no image on it.
fn image_path_from(item: &ClipboardItem) -> Option<Result<String, String>> {
    let image = item.entries().iter().find_map(|e| match e { gpui::ClipboardEntry::Image(i) => Some(i), _ => None })?;
    let ext = match image.format {
        gpui::ImageFormat::Png => "png", gpui::ImageFormat::Jpeg => "jpg", gpui::ImageFormat::Webp => "webp",
        gpui::ImageFormat::Gif => "gif", _ => "img",
    };
    let dir = std::env::temp_dir().join("drover-paste");
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let path = dir.join(format!("paste-{stamp}.{ext}"));
    let written = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, &image.bytes));
    Some(written.map(|_| {
        let p = path.to_string_lossy().into_owned();
        if p.contains(' ') { format!("\"{p}\"") } else { p }
    }).map_err(|e| format!("Could not save the pasted image: {e}")))
}
