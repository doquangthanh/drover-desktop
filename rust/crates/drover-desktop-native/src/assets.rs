//! Built-in assets: the line icons of the web UI (`ui/atoms/icon.tsx`, same paths), the logo and the Geist font.

use std::borrow::Cow;

use gpui::{App, AssetSource, Result, SharedString};

const ICONS: &[(&str, &str)] = &[
    ("search", r#"<circle cx="11" cy="11" r="7"/><path d="M20 20l-3.5-3.5"/>"#),
    ("sidebar", r#"<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M9 4v16"/>"#),
    ("monitor", r#"<rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 20h8"/>"#),
    ("server", r#"<rect x="3" y="4" width="18" height="7" rx="1.5"/><rect x="3" y="13" width="18" height="7" rx="1.5"/>"#),
    ("layers", r#"<path d="M12 3l9 5-9 5-9-5z"/><path d="M3 13l9 5 9-5"/>"#),
    ("inbox", r#"<path d="M3 13h5l2 3h4l2-3h5"/><path d="M5 5h14l2 8v6H3v-6z"/>"#),
    ("gear", r#"<circle cx="12" cy="12" r="3"/><path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M4.9 19.1L7 17M17 7l2.1-2.1"/>"#),
    ("folder", r#"<path d="M3 7h6l2 2h10v10H3z"/>"#),
    ("plus", r#"<path d="M12 5v14M5 12h14"/>"#),
    ("split", r#"<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M12 4v16"/>"#),
    ("splitdown", r#"<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M3 12h18"/>"#),
    ("zoom", r#"<path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5"/>"#),
    ("close", r#"<path d="M6 6l12 12M18 6L6 18"/>"#),
    ("branch", r#"<circle cx="6" cy="6" r="2"/><circle cx="6" cy="18" r="2"/><circle cx="18" cy="8" r="2"/><path d="M6 8v8M18 10c0 4-6 3-12 6"/>"#),
    ("more", r#"<path d="M5 12h.01M12 12h.01M19 12h.01" stroke-width="3"/>"#),
    ("check", r#"<path d="M5 12.5l4.5 4.5L19 7.5"/>"#),
    ("alert", r#"<path d="M12 4l9 16H3z"/><path d="M12 10v4M12 17.5h.01"/>"#),
    ("info", r#"<circle cx="12" cy="12" r="9"/><path d="M12 11v5M12 7.5h.01"/>"#),
    ("up", r#"<path d="M12 19V5M5 12l7-7 7 7"/>"#),
    ("tab", r#"<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M3 9h18"/>"#),
    // The working spinner: a ring with a gap, rotated by an animation (`.spin` in base.css).
    ("spin", r#"<path d="M12 3a9 9 0 1 1-9 9"/>"#),
];

pub const LOGO: &[u8] = include_bytes!("../assets/logo.png");

/// Geist for the UI; JetBrains Mono (OFL, `assets/fonts/JetBrainsMono-OFL.txt`) for terminals whose system font has no
/// Vietnamese letters (terminal_paint::resolved), as Herdr ships it.
const FONTS: [&[u8]; 5] = [
    include_bytes!("../assets/fonts/Geist-Regular.ttf"),
    include_bytes!("../assets/fonts/Geist-Medium.ttf"),
    include_bytes!("../assets/fonts/Geist-SemiBold.ttf"),
    include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf"),
    include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf"),
];

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let Some(name) = path.strip_prefix("icons/").and_then(|p| p.strip_suffix(".svg")) else { return Ok(None) };
        Ok(ICONS.iter().find(|(n, _)| *n == name).map(|(_, body)| {
            // Drawn as a mask and tinted with the text colour, so the stroke colour itself does not matter.
            Cow::Owned(format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{body}</svg>"#
            ).into_bytes())
        }))
    }

    fn list(&self, _path: &str) -> Result<Vec<SharedString>> {
        Ok(ICONS.iter().map(|(n, _)| SharedString::from(format!("icons/{n}.svg"))).collect())
    }
}

pub fn load_fonts(cx: &App) {
    if let Err(e) = cx.text_system().add_fonts(FONTS.iter().map(|f| Cow::Borrowed(*f)).collect()) {
        eprintln!("drover: could not load the bundled fonts: {e}");
    }
}
