//! File icons for the Files tree and tabs: the web UI's "Badges" set (core/file-icons.ts `badgeSpec`), a small coloured
//! label per file type. The web's Material and Line sets and the choice in Settings are not ported.

use gpui::{Div, FontWeight, div, prelude::*, rgb};

use crate::ui_scale::px;

const COLORS: [(&str, u32); 16] = [
    ("py", 0x3572a5), ("js", 0xc9a800), ("ts", 0x3178c6), ("tsx", 0x3178c6), ("jsx", 0xc9a800), ("json", 0x8a8f98), ("md", 0x519aba),
    ("html", 0xe34c26), ("css", 0x563d7c), ("yml", 0xcb171e), ("yaml", 0xcb171e), ("toml", 0x9c4221), ("sh", 0x4eaa25), ("sql", 0xe38c00),
    ("go", 0x00add8), ("rs", 0xb7410e),
];
const PALETTE: [u32; 8] = [0xd97757, 0x5b8def, 0x8f7ce0, 0x4cc48c, 0xe0a030, 0xd6578c, 0x3fa7a7, 0x7d8590];

/// The label (up to 3 capitals) and background colour for a file name.
pub fn badge_spec(name: &str) -> (String, u32) {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default();
    let source = if ext.is_empty() { name } else { ext.as_str() };
    let label: String = if ext.is_empty() { name.chars().take(2).collect() } else { ext.chars().take(3).collect() };
    let hash = source.encode_utf16().fold(0u32, |h, c| h.wrapping_mul(31).wrapping_add(c as u32));
    let color = COLORS.iter().find(|(e, _)| *e == ext).map(|(_, c)| *c).unwrap_or(PALETTE[hash as usize % PALETTE.len()]);
    (label.to_uppercase(), color)
}

/// The badge, 15 px high.
pub fn file_badge(name: &str) -> Div {
    let (label, color) = badge_spec(name);
    div().h(px(15.)).min_w(px(17.)).px(px(3.)).flex_none().flex().items_center().justify_center().rounded(px(3.))
        .bg(rgb(color)).text_color(rgb(0xffffff)).text_size(px(8.5)).font_weight(FontWeight::BOLD).child(label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_types_have_their_colour_and_a_three_letter_label() {
        assert_eq!(badge_spec("main.rs"), ("RS".into(), 0xb7410e));
        assert_eq!(badge_spec("App.TSX"), ("TSX".into(), 0x3178c6));
        assert_eq!(badge_spec("a.markdown").0, "MAR");
    }

    #[test]
    fn a_file_without_extension_uses_its_first_two_letters_and_a_stable_colour() {
        let (label, c) = badge_spec("Makefile");
        assert_eq!(label, "MA");
        assert_eq!(badge_spec("Makefile").1, c);
        assert!(PALETTE.contains(&c));
    }
}
