//! Built-in assets: the line icons of the web UI (`ui/atoms/icon.tsx`, same paths), the logo and the Geist font.

use std::borrow::Cow;

use gpui::{App, AssetSource, Result, SharedString};

const AGENT_TAB: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 14 50" fill="none" stroke="black" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"><path d="M3,2L12,5.5L3,9M6,3.3L6,7.7M6.8,16.0C9,16.0 9,11.0 6.8,11.0C4.6,11.0 4.6,16.0 6.8,16.0M9,16.0L2,16.0C0,16.0 0,12.0 1.5,11.3M6.5,18.5L6.5,23.5C9,23.5 9.2,18.5 6.5,18.5C4,18.5 3.3,22 4.5,23.3M3,25.8L9,25.8M7.8,25.8C9.3,26.5 9.8,30.5 7,30.5L3,30.5M11.5,34.8L4.5,34.8C3.2,34.8 3,36 3.4,37M9,32.5L9,37.3M4.5,43.5L7.5,47L10.5,43.5"/></svg>"#;

const ICONS: &[(&str, &str)] = &[
    ("search", r#"<circle cx="11" cy="11" r="7"/><path d="M20 20l-3.5-3.5"/>"#),
    ("sidebar", r#"<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M9 4v16"/>"#),
    ("monitor", r#"<rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 20h8"/>"#),
    ("server", r#"<rect x="3" y="4" width="18" height="7" rx="1.5"/><rect x="3" y="13" width="18" height="7" rx="1.5"/>"#),
    ("layers", r#"<path d="M12 3l9 5-9 5-9-5z"/><path d="M3 13l9 5 9-5"/>"#),
    ("inbox", r#"<path d="M3 13h5l2 3h4l2-3h5"/><path d="M5 5h14l2 8v6H3v-6z"/>"#),
    ("gear", r#"<circle cx="12" cy="12" r="3"/><path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M4.9 19.1L7 17M17 7l2.1-2.1"/>"#),
    ("folder", r#"<path d="M3 7h6l2 2h10v10H3z"/>"#),
    ("chat", r#"<path d="M4 5h16v11H10l-6 4z"/>"#),
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
    ("back", r#"<path d="M19 12H5M12 19l-7-7 7-7"/>"#),
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
        if let Some(name) = path.strip_prefix("file-icons/").and_then(|p| p.strip_suffix(".svg")) {
            // Coloured Material icons (file_icons.rs): drawn as images, not tinted masks.
            return Ok(crate::material_icons::MATERIAL.binary_search_by_key(&name, |(k, _)| k).ok()
                .map(|i| Cow::Borrowed(crate::material_icons::MATERIAL[i].1)));
        }
        if path == "icons/agent-tab.svg" {
            // The word "Agent" and a chevron turned 90° clockwise, the web's vertical tab label (GPUI cannot turn text).
            return Ok(Some(Cow::Borrowed(AGENT_TAB.as_bytes())));
        }
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
