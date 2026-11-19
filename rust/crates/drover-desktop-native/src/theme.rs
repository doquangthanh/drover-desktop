//! Dark theme tokens, the same values as `frontend/src/styles/base.css` (:root), so both clients look alike.

use gpui::{Rgba, rgba};

pub const CANVAS: u32 = 0x131518;
pub const CHROME: u32 = 0x101114;
pub const PANE: u32 = 0x17191d;
pub const RAISED: u32 = 0x2c2e31;
pub const FG: u32 = 0xeeeeef;
pub const MUTED: u32 = 0x98999b;
pub const DIM: u32 = 0x85868a;
pub const ACCENT: u32 = 0x96b2d7;
pub const WORKING: u32 = 0x76a8ff;
pub const BLOCKED: u32 = 0xf2a33c;
pub const DONE: u32 = 0x4cc48c;
pub const ERROR: u32 = 0xe8848c;
pub const YELLOW: u32 = 0xd8be87;
pub const ON_ACCENT: u32 = 0x101114;

/// White at an opacity, as the CSS `rgb(var(--ink) / a)` tokens.
pub fn ink(alpha: f32) -> Rgba {
    rgba(0xffffff00 | ((alpha * 255.0).round() as u32))
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
        "claude" => ("C".into(), 0xd97757, ON_ACCENT),
        "codex" => ("X".into(), 0x5b8def, ON_ACCENT),
        "gemini" => ("G".into(), 0x8f7ce0, ON_ACCENT),
        "opencode" => ("O".into(), 0xa6bd91, ON_ACCENT),
        "shell" => ("$".into(), 0x3a3c40, 0xd2d4d9),
        other => (other.chars().next().map(|c| c.to_ascii_uppercase().to_string()).unwrap_or("?".into()), 0x818793, ON_ACCENT),
    }
}
