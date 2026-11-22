//! Typing into the focused pane: special keys and chords through `keystroke_bytes`, text through the input method
//! (so ibus/Telex compose finished Vietnamese words), mouse wheel through the scrollback or the app's mouse mode.

use std::ops::Range;

use alacritty_terminal::grid::Scroll;
use alacritty_terminal::term::TermMode;
use drover_core::keystroke_bytes::{Mods, keystroke_bytes};
use drover_proto::client::Command;
use gpui::{
    Bounds, Context, EntityInputHandler, KeyDownEvent, Pixels, ScrollDelta, ScrollWheelEvent, UTF16Selection, Window,
};

use crate::workspace_app::DroverApp;

impl DroverApp {
    /// Send bytes to the focused pane and bring its view back to the bottom, as terminals do on input.
    pub fn send_to_focused(&mut self, data: String) {
        let Some(pane) = self.focused.clone() else { return };
        if let Some(screen) = self.screens.lock().unwrap().get_mut(&pane) {
            screen.term_mut().scroll_display(Scroll::Bottom);
        }
        let _ = self.client.commands.send(Command::Input { pane, data });
    }

    fn app_cursor(&self) -> bool {
        let Some(pane) = self.focused.as_ref() else { return false };
        self.screens.lock().unwrap().get(pane).is_some_and(|s| s.term().mode().contains(TermMode::APP_CURSOR))
    }

    pub fn on_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.confirm.is_some() {
            // A question is open: Enter answers yes, Escape no; nothing reaches the agent meanwhile.
            match ev.keystroke.key.as_str() {
                "enter" => self.answer_confirm(true, window, cx),
                "escape" => self.answer_confirm(false, window, cx),
                _ => {}
            }
            cx.stop_propagation();
            return;
        }
        if self.menu.is_some() {
            // A right-click menu is open: any key closes it, and Escape stops there.
            self.close_menu(cx);
            if ev.keystroke.key == "escape" {
                cx.stop_propagation();
                return;
            }
        }
        if self.copy_mode.is_some() {
            self.copy_key(ev, cx);
            cx.stop_propagation();
            return;
        }
        if self.show_shortcuts && ev.keystroke.key == "escape" {
            self.show_shortcuts = false;
            cx.stop_propagation();
            cx.notify();
            return;
        }
        // Drover's shortcuts first, even inside a terminal (as VS Code; keys.rs).
        if self.shortcut(ev, window, cx) {
            cx.stop_propagation();
            return;
        }
        if self.view != crate::views::View::Terminals {
            // A view is shown: keys are not for the agents; Escape goes back to the terminals.
            if ev.keystroke.key == "escape" {
                self.set_view(crate::views::View::Terminals, window, cx);
            }
            return;
        }
        let m = &ev.keystroke.modifiers;
        let key = ev.keystroke.key.as_str();
        // Copy while text is selected (else Ctrl+C reaches the agent); paste the clipboard (terminal_selection.rs).
        let copy = m.control && !m.alt && key == "c" && (self.copy_selection(cx) || m.shift);
        let paste = (m.control && !m.alt && key == "v") || (m.shift && !m.control && key == "insert");
        if copy || paste {
            if paste {
                self.paste(cx);
            }
            cx.stop_propagation();
            return;
        }
        let mods = Mods { ctrl: m.control, alt: m.alt, shift: m.shift };
        if let Some(bytes) = keystroke_bytes(&ev.keystroke.key, mods, self.app_cursor()) {
            self.send_to_focused(bytes);
            cx.stop_propagation(); // handled: do not also deliver it as text
            cx.notify();
        }
    }

    pub fn on_wheel(&mut self, pane: &str, ev: &ScrollWheelEvent, cx: &mut Context<Self>) {
        let lines = match ev.delta {
            ScrollDelta::Lines(p) => p.y * 3.0,
            ScrollDelta::Pixels(p) => f32::from(p.y) / self.term_font.line_height,
        };
        let n = lines.round() as i32;
        if n == 0 {
            return;
        }
        let mut screens = self.screens.lock().unwrap();
        let Some(screen) = screens.get_mut(pane) else { return };
        let mode = *screen.term().mode();
        if mode.intersects(TermMode::MOUSE_MODE) && mode.contains(TermMode::SGR_MOUSE) {
            // The program wants the wheel itself (e.g. a full-screen app): SGR wheel up = 64, down = 65.
            let button = if n > 0 { 64 } else { 65 };
            let seq = format!("\x1b[<{button};1;1M").repeat(n.unsigned_abs() as usize);
            drop(screens);
            let _ = self.client.commands.send(Command::Input { pane: pane.to_string(), data: seq });
        } else if mode.contains(TermMode::ALT_SCREEN) {
            let key = if n > 0 { "\x1b[A" } else { "\x1b[B" };
            drop(screens);
            let _ = self.client.commands.send(Command::Input { pane: pane.to_string(), data: key.repeat(n.unsigned_abs() as usize) });
        } else {
            screen.term_mut().scroll_display(Scroll::Delta(n));
        }
        cx.notify();
    }
}

/// The terminal has no editable text of its own: the input method only commits text, which goes to the agent.
impl EntityInputHandler for DroverApp {
    fn text_for_range(&mut self, _: Range<usize>, _: &mut Option<Range<usize>>, _: &mut Window, _: &mut Context<Self>) -> Option<String> {
        None
    }

    fn selected_text_range(&mut self, _: bool, _: &mut Window, _: &mut Context<Self>) -> Option<UTF16Selection> {
        Some(UTF16Selection { range: 0..0, reversed: false })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.as_ref().map(|t| 0..t.encode_utf16().count())
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked = None;
    }

    fn replace_text_in_range(&mut self, _: Option<Range<usize>>, text: &str, _: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
        if !text.is_empty() && self.confirm.is_none() && self.view == crate::views::View::Terminals {
            self.send_to_focused(text.to_string());
        }
        cx.notify();
    }

    fn replace_and_mark_text_in_range(&mut self, _: Option<Range<usize>>, text: &str, _: Option<Range<usize>>,
                                      _: &mut Window, cx: &mut Context<Self>) {
        self.marked = (!text.is_empty()).then(|| text.to_string()); // being composed: shown, not sent yet
        cx.notify();
    }

    fn bounds_for_range(&mut self, _: Range<usize>, element: Bounds<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<Bounds<Pixels>> {
        Some(self.cursor_bounds.lock().unwrap().unwrap_or(element)) // where the IME candidate window should appear
    }

    fn character_index_for_point(&mut self, _: gpui::Point<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        None
    }
}

const WHEEL_MAX_STREAK: u32 = 8;

/// 1x for a slow wheel, up to about 3.4x when notches come in quick succession.
fn wheel_boost(streak: u32) -> f32 {
    1.0 + streak as f32 * 0.3
}

/// The whole lines in `rest + lines` (towards zero), and what is left over.
fn whole_lines(rest: f32, lines: f32) -> (i32, f32) {
    let total = rest + lines;
    let n = total.trunc();
    (n as i32, total - n)
}

#[cfg(test)]
mod wheel_tests {
    use super::*;

    #[test]
    fn small_moves_add_up_instead_of_vanishing() {
        let (mut rest, mut scrolled) = (0.0, 0);
        for _ in 0..10 {
            let (n, r) = whole_lines(rest, 0.3);
            rest = r;
            scrolled += n;
        }
        assert_eq!(scrolled, 3);
        assert_eq!(whole_lines(0.0, -2.6).0, -2);
    }

    #[test]
    fn quick_turning_goes_further() {
        assert_eq!(wheel_boost(0), 1.0);
        assert!(wheel_boost(WHEEL_MAX_STREAK) > 3.0);
    }
}
