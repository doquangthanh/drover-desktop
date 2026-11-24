//! Colour tokens of every theme. The colours themselves are GENERATED from drover-core/design/tokens.json
//! (`theme_data.rs`), the same file the web UI's `themes.generated.css` comes from, so both clients look alike.
//! The current theme is a process-wide setting: every colour is a function that reads it at paint time, so switching
//! needs no rebuild of any view.

use std::sync::atomic::{AtomicU8, Ordering};

use gpui::{Rgba, rgba};

pub use crate::theme_data::{ThemeDef, ALL};

#[allow(dead_code)] // `faint`, `head`, `add`, `rem` are tokens of the web UI too; the native views take them into use as they grow
pub struct Palette {
    pub canvas: u32, pub chrome: u32, pub pane: u32, pub raised: u32, pub card: u32,
    pub fg: u32, pub tfg: u32, pub muted: u32, pub dim: u32, pub faint: u32,
    pub accent: u32, pub working: u32, pub blocked: u32, pub done: u32, pub error: u32, pub yellow: u32, pub on_accent: u32,
    /// Header strip, added / removed text of a diff (`--head`, `--add`, `--rem`).
    pub head: u32, pub add: u32, pub rem: u32,
    /// Overlay ink: white on dark themes, black on light.
    pub ink: u32,
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

/// A theme id from tokens.json ("dark", "dracula", ...) or "auto" (asks the desktop). Unknown names keep the current theme.
pub fn set_theme(name: &str) {
    let id = if name == "auto" { if system_prefers_dark() { "dark" } else { "light" } } else { name };
    if let Some(i) = ALL.iter().position(|t| t.id == id) {
        CURRENT.store(i as u8, Ordering::Relaxed);
    }
}

pub fn current() -> &'static ThemeDef { &ALL[(CURRENT.load(Ordering::Relaxed) as usize).min(ALL.len() - 1)] }

pub fn is_light() -> bool { current().light }

fn system_prefers_dark() -> bool {
    if !cfg!(target_os = "linux") { return true; }
    std::process::Command::new("gsettings").args(["get", "org.gnome.desktop.interface", "color-scheme"]).output().ok()
        .map(|o| !String::from_utf8_lossy(&o.stdout).contains("default")).unwrap_or(true)
}

pub fn p() -> &'static Palette { current().palette }

/// The 16 terminal colours of the current theme.
pub fn ansi() -> &'static [u32; 16] { &current().ansi }

pub fn canvas() -> u32 { p().canvas }
pub fn chrome() -> u32 { p().chrome }
pub fn pane() -> u32 { p().pane }
pub fn raised() -> u32 { p().raised }
pub fn card() -> u32 { p().card }
pub fn fg() -> u32 { p().fg }
pub fn tfg() -> u32 { p().tfg }
pub fn muted() -> u32 { p().muted }
pub fn dim() -> u32 { p().dim }
pub fn accent() -> u32 { p().accent }
pub fn working() -> u32 { p().working }
pub fn blocked() -> u32 { p().blocked }
pub fn done() -> u32 { p().done }
pub fn error() -> u32 { p().error }
pub fn yellow() -> u32 { p().yellow }
pub fn on_accent() -> u32 { p().on_accent }

/// White at an opacity, as the CSS `rgb(var(--ink) / a)` tokens.
pub fn ink(alpha: f32) -> Rgba {
    rgba((p().ink << 8) | ((alpha * 255.0).round() as u32))
}

pub fn border() -> Rgba { ink(0.094) }
pub fn border2() -> Rgba { ink(0.12) }
pub fn hover() -> Rgba { ink(0.055) }
pub fn selected() -> Rgba { ink(0.09) }

/// `rgb(var(--accent-rgb) / a)`.
pub fn tint(color: u32, alpha: f32) -> Rgba {
    rgba((color << 8) | ((alpha * 255.0).round() as u32))
}

pub const UI_FONT: &str = "Geist";
/// Terminal font while the daemon knows none: what the web UI's `ui-monospace, monospace` gives on each OS.
/// "monospace" is a fontconfig alias, unknown to DirectWrite / Core Text.
pub const DEFAULT_TERMINAL_FONT: &str =
    if cfg!(windows) { "Consolas" } else if cfg!(target_os = "macos") { "Menlo" } else { "monospace" };
pub const AVATAR: [u32; 8] = [0x24877b, 0xb9602f, 0x4e7fe0, 0x7a5fd6, 0x2f8a4a, 0xb2456b, 0x8a7a2b, 0x3d7f99];

/// Agent badge: letter, background, foreground (`kind-icon.tsx`).
pub fn kind_badge(kind: &str) -> (String, u32, u32) {
    match kind {
        "claude" => ("C".into(), 0xd97757, on_accent()),
        "codex" => ("X".into(), 0x5b8def, on_accent()),
        "gemini" => ("G".into(), 0x8f7ce0, on_accent()),
        "opencode" => ("O".into(), 0xa6bd91, on_accent()),
        "shell" => ("$".into(), 0x3a3c40, 0xd2d4d9),
        other => (other.chars().next().map(|c| c.to_ascii_uppercase().to_string()).unwrap_or("?".into()), 0x818793, on_accent()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// The current theme is global: tests that change it take turns.
    static TAKE_TURNS: Mutex<()> = Mutex::new(());

    #[test]
    fn every_generated_theme_can_be_selected_and_has_its_own_colours() {
        let _turn = TAKE_TURNS.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(ALL[0].id, "dark", "the first theme is the default");
        for t in ALL.iter() {
            set_theme(t.id);
            assert_eq!(p().canvas, t.palette.canvas, "{}", t.id);
            assert_eq!(is_light(), t.light, "{}", t.id);
        }
        set_theme("dracula");
        assert_eq!(p().accent, 0xbd93f9);
        set_theme("no-such-theme"); // unknown names keep the current theme
        assert_eq!(p().accent, 0xbd93f9);
        set_theme("dark");
    }

    #[test]
    fn terminal_colours_follow_the_theme_and_ids_are_unique() {
        let _turn = TAKE_TURNS.lock().unwrap_or_else(|e| e.into_inner());
        set_theme("dark");
        let dark = *ansi();
        set_theme("light");
        assert_ne!(*ansi(), dark);
        set_theme("dark");
        let mut ids: Vec<_> = ALL.iter().map(|t| t.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), ALL.len());
    }
}
