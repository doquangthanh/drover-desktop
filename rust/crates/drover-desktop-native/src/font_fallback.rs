//! A font per character for the terminal. GPUI on Linux falls back to proportional UI fonts (Ubuntu, DejaVu Sans)
//! for characters the terminal font lacks, so text the terminal font only half covers looks broken: Ubuntu Mono
//! has no precomposed Vietnamese letters (ỉ ổ ở ế ể, U+1E00-1EFF). Such characters get the first monospace font
//! here that has them, as GNOME Terminal and the browser do through fontconfig. Looked up once per character.

use std::cell::RefCell;
use std::collections::HashMap;

use gpui::{SharedString, Window, font, px};

const MONO_FALLBACKS: [&str; 7] =
    ["Noto Sans Mono", "DejaVu Sans Mono", "Liberation Mono", "FreeMono", "Cascadia Mono", "Consolas", "Menlo"];

/// Bundled (assets.rs): the terminal font when the system one has no Vietnamese letters.
pub const BUNDLED_MONO: &str = "JetBrains Mono";

thread_local! {
    static VIETNAMESE: RefCell<HashMap<SharedString, bool>> = RefCell::default();
    static CACHE: RefCell<HashMap<(SharedString, char), Option<SharedString>>> = RefCell::default();
}

/// True when `family` is installed and has a glyph for `ch`.
fn has(window: &Window, family: &str, ch: char) -> bool {
    let ts = window.text_system();
    let id = ts.resolve_font(&font(family.to_string()));
    ts.get_font_for_id(id).is_some_and(|f| f.family.as_ref() == family) && ts.advance(id, px(12.), ch).is_ok()
}

/// True when `family` draws Vietnamese with its own letters (ệ ỉ ổ ở ữ): mixing in another font per letter looks broken.
pub fn covers_vietnamese(window: &Window, family: &SharedString) -> bool {
    VIETNAMESE.with(|v| *v.borrow_mut().entry(family.clone())
        .or_insert_with(|| ['ệ', 'ỉ', 'ổ', 'ở', 'ữ', 'đ', 'ơ'].iter().all(|c| has(window, family, *c))))
}

/// The family to draw `ch` with when the terminal font `family` lacks it; None keeps the terminal font.
pub fn family_for(window: &Window, family: &SharedString, ch: char) -> Option<SharedString> {
    if ch.is_ascii() {
        return None;
    }
    CACHE.with(|cache| {
        cache.borrow_mut().entry((family.clone(), ch)).or_insert_with(|| {
            if has(window, family, ch) {
                return None;
            }
            MONO_FALLBACKS.iter().find(|f| has(window, f, ch)).map(|f| SharedString::from(*f))
        }).clone()
    })
}
