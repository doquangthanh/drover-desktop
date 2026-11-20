//! Mouse reporting: a program that turned on mouse tracking (DECSET 1000 / 1002 / 1003 with SGR 1006), such as Claude
//! Code's full-screen view with its "Jump to bottom" button, gets the clicks and drags itself, as in xterm. Holding
//! Shift keeps the local behaviour (select text, open links). Only the SGR encoding is sent; without it the mouse
//! stays local, as the wheel does (terminal_input.rs).

use alacritty_terminal::term::TermMode;
use gpui::{Context, Modifiers, Pixels, Window};

use crate::terminal_paint;
use crate::workspace_app::DroverApp;

/// Button code of an SGR report: 0 left, +32 while moving, +4 Shift, +8 Alt, +16 Ctrl. 35 = moving with no button.
pub fn button_code(button: Option<u32>, mods: Modifiers, motion: bool) -> u32 {
    let base = button.unwrap_or(3) + if motion { 32 } else { 0 };
    base + if mods.shift { 4 } else { 0 } + if mods.alt { 8 } else { 0 } + if mods.control { 16 } else { 0 }
}

/// `ESC [ < code ; col ; row M` (press, motion) or `m` (release); col and row start at 1.
pub fn sgr(code: u32, col: usize, row: usize, release: bool) -> String {
    format!("\x1b[<{code};{};{}{}", col + 1, row + 1, if release { 'm' } else { 'M' })
}

/// What the program asked for, from its terminal modes: (reports clicks, wants drags, wants every motion).
pub fn wants(mode: TermMode) -> Option<(bool, bool)> {
    if !mode.contains(TermMode::SGR_MOUSE) || !mode.intersects(TermMode::MOUSE_MODE) { return None }
    Some((mode.contains(TermMode::MOUSE_DRAG) || mode.contains(TermMode::MOUSE_MOTION), mode.contains(TermMode::MOUSE_MOTION)))
}

impl DroverApp {
    fn mouse_mode(&self, pane: &str) -> Option<(bool, bool)> {
        wants(*self.screens.lock().unwrap().get(pane)?.term().mode())
    }

    /// Screen cell (column, row from 0, inside the pane's terminal area) under `pos`.
    fn screen_cell(&self, pane: &str, pos: gpui::Point<Pixels>, window: &Window) -> Option<(usize, usize)> {
        use alacritty_terminal::grid::Dimensions;
        let bounds = *self.term_bounds.lock().unwrap().get(pane)?;
        let cell_w = f32::from(terminal_paint::cell_width(&self.term_font, window));
        let screens = self.screens.lock().unwrap();
        let term = screens.get(pane)?.term();
        let x = f32::from(pos.x - bounds.origin.x).max(0.);
        let y = f32::from(pos.y - bounds.origin.y).max(0.);
        Some((((x / cell_w) as usize).min(term.columns() - 1), ((y / self.term_font.line_height) as usize).min(term.screen_lines() - 1)))
    }

    fn send_mouse(&self, pane: &str, data: String) {
        let _ = self.client.commands.send(drover_proto::client::Command::Input { pane: pane.to_string(), data });
    }

    /// Left button pressed: true when the program took it (Shift not held, tracking on).
    pub fn mouse_press(&mut self, pane: &str, pos: gpui::Point<Pixels>, mods: Modifiers, window: &Window, cx: &mut Context<Self>) -> bool {
        if mods.shift || self.mouse_mode(pane).is_none() { return false }
        let Some((col, row)) = self.screen_cell(pane, pos, window) else { return false };
        self.send_mouse(pane, sgr(button_code(Some(0), mods, false), col, row, false));
        self.mouse_reporting = Some(pane.to_string());
        self.mouse_last = Some((pane.to_string(), col, row));
        cx.notify();
        true
    }

    /// Pointer moved: true when it was reported (a drag with the button held, or any motion if the program asked).
    pub fn mouse_motion(&mut self, pane: &str, pos: gpui::Point<Pixels>, left_down: bool, mods: Modifiers, window: &Window) -> bool {
        let Some((drags, all_motion)) = self.mouse_mode(pane) else { return false };
        let held = left_down && self.mouse_reporting.as_deref() == Some(pane);
        if mods.shift || !(if held { drags } else { all_motion }) { return false }
        let Some((col, row)) = self.screen_cell(pane, pos, window) else { return false };
        if self.mouse_last.as_ref().is_none_or(|(p, c, r)| (p.as_str(), *c, *r) != (pane, col, row)) {
            self.send_mouse(pane, sgr(button_code(held.then_some(0), mods, true), col, row, false));
            self.mouse_last = Some((pane.to_string(), col, row));
        }
        true
    }

    /// Left button released after a reported press: tell the program; true when it was ours.
    pub fn mouse_release(&mut self, pane: &str, pos: gpui::Point<Pixels>, mods: Modifiers, window: &Window) -> bool {
        if self.mouse_reporting.as_deref() != Some(pane) { return false }
        self.mouse_reporting = None;
        if let Some((col, row)) = self.screen_cell(pane, pos, window) {
            self.send_mouse(pane, sgr(button_code(Some(0), mods, false), col, row, true));
            self.mouse_last = Some((pane.to_string(), col, row));
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_are_one_based_and_end_with_m_on_release() {
        assert_eq!(sgr(0, 0, 0, false), "\x1b[<0;1;1M");
        assert_eq!(sgr(0, 9, 4, true), "\x1b[<0;10;5m");
    }

    #[test]
    fn button_codes_add_motion_and_modifiers() {
        let none = Modifiers::default();
        assert_eq!(button_code(Some(0), none, false), 0);
        assert_eq!(button_code(Some(0), none, true), 32); // drag
        assert_eq!(button_code(None, none, true), 35); // motion with no button
        assert_eq!(button_code(Some(0), Modifiers { control: true, alt: true, ..none }, false), 24);
    }

    #[test]
    fn needs_sgr_and_a_tracking_mode() {
        assert_eq!(wants(TermMode::NONE), None);
        assert_eq!(wants(TermMode::MOUSE_REPORT_CLICK), None); // no SGR: the mouse stays local
        assert_eq!(wants(TermMode::MOUSE_REPORT_CLICK | TermMode::SGR_MOUSE), Some((false, false)));
        assert_eq!(wants(TermMode::MOUSE_DRAG | TermMode::SGR_MOUSE), Some((true, false)));
        assert_eq!(wants(TermMode::MOUSE_MOTION | TermMode::SGR_MOUSE), Some((true, true)));
    }
}
