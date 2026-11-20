//! Colour tokens of the three themes (dark, light, midnight), the same values as `frontend/src/styles/base.css` and
//! `themes.css`, so both clients look alike. The current theme is a process-wide setting: every colour is a function
//! that reads it at paint time, so switching needs no rebuild of any view.

use std::sync::atomic::{AtomicU8, Ordering};

use gpui::{Rgba, rgba};

pub struct Palette {
    pub canvas: u32, pub chrome: u32, pub pane: u32, pub raised: u32, pub card: u32,
    pub fg: u32, pub tfg: u32, pub muted: u32, pub dim: u32,
    pub accent: u32, pub working: u32, pub blocked: u32, pub done: u32, pub error: u32, pub yellow: u32, pub on_accent: u32,
    /// Overlay ink: white on dark themes, black on light.
    pub ink: u32,
}

pub const DARK: Palette = Palette {
    canvas: 0x131518, chrome: 0x101114, pane: 0x17191d, raised: 0x2c2e31, card: 0x1d1f23, fg: 0xeeeeef, tfg: 0xd2d4d9,
    muted: 0x98999b, dim: 0x85868a, accent: 0x96b2d7, working: 0x76a8ff, blocked: 0xf2a33c, done: 0x4cc48c, error: 0xe8848c,
    yellow: 0xd8be87, on_accent: 0x101114, ink: 0xffffff,
};
pub const LIGHT: Palette = Palette {
    canvas: 0xeef0f3, chrome: 0xe4e7eb, pane: 0xffffff, raised: 0xdfe3e8, card: 0xf6f7f9, fg: 0x1a1c20, tfg: 0x2b2f36,
    muted: 0x5b6270, dim: 0x6c7380, accent: 0x2f6bb3, working: 0x2563eb, blocked: 0xb86e00, done: 0x1a8f5b, error: 0xc93a47,
    yellow: 0x946200, on_accent: 0xffffff, ink: 0x000000,
};
pub const MIDNIGHT: Palette = Palette {
    canvas: 0x0b0e17, chrome: 0x080a12, pane: 0x0f1320, raised: 0x1f2538, card: 0x131828, fg: 0xe6e9f5, tfg: 0xcdd3e8,
    muted: 0x8e97b5, dim: 0x7a84a3, accent: 0x7c9cff, working: 0x5aa2ff, blocked: 0xffb454, done: 0x52d6a0, error: 0xff7a8a,
    yellow: 0xd8be87, on_accent: 0x101114, ink: 0xffffff,
};

static CURRENT: AtomicU8 = AtomicU8::new(0);

/// "dark" | "light" | "midnight" | "auto" (auto asks the desktop). Unknown names keep the current theme.
pub fn set_theme(name: &str) {
    let i = match name {
        "dark" => 0, "light" => 1, "midnight" => 2,
        "auto" => if system_prefers_dark() { 0 } else { 1 },
        _ => return,
    };
    CURRENT.store(i, Ordering::Relaxed);
}

pub fn is_light() -> bool { CURRENT.load(Ordering::Relaxed) == 1 }

fn system_prefers_dark() -> bool {
    if !cfg!(target_os = "linux") { return true; }
    std::process::Command::new("gsettings").args(["get", "org.gnome.desktop.interface", "color-scheme"]).output().ok()
        .map(|o| !String::from_utf8_lossy(&o.stdout).contains("default")).unwrap_or(true)
}

pub fn p() -> &'static Palette {
    match CURRENT.load(Ordering::Relaxed) { 1 => &LIGHT, 2 => &MIDNIGHT, _ => &DARK }
}

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
