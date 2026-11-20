//! Powerline glyphs in prompts (agnoster, powerlevel, starship). Their font (PowerlineSymbols, Nerd Fonts) is
//! useless here: GPUI on Linux drops any font without an "m" glyph (`load_family` in its text system), and the
//! terminal font rarely has them. So the separators are drawn as shapes filling their cell, as alacritty and
//! wezterm do, and the branch icon becomes a symbol common fonts have.

use gpui::{Bounds, Hsla, PathBuilder, Pixels, Window, point, px};

/// A character to print instead of `c` (one a usual font has).
pub fn substitute(c: char) -> Option<char> {
    match c {
        '\u{E0A0}' => Some('\u{2387}'), // branch  -> ⎇
        _ => None,
    }
}

/// Separators drawn by `paint`: solid and thin arrows, right and left.
pub fn is_drawn(c: char) -> bool {
    matches!(c, '\u{E0B0}'..='\u{E0B3}')
}

/// Draw separator `c` over `cell` in `color`.
pub fn paint(c: char, cell: Bounds<Pixels>, color: Hsla, window: &mut Window) {
    let (l, t) = (cell.origin.x, cell.origin.y);
    let (r, b) = (l + cell.size.width, t + cell.size.height);
    let mid = t + cell.size.height / 2.;
    let (mut path, points, closed) = match c {
        '\u{E0B0}' => (PathBuilder::fill(), [point(l, t), point(r, mid), point(l, b)], true),
        '\u{E0B2}' => (PathBuilder::fill(), [point(r, t), point(l, mid), point(r, b)], true),
        '\u{E0B1}' => (PathBuilder::stroke(px(1.)), [point(l, t), point(r, mid), point(l, b)], false),
        '\u{E0B3}' => (PathBuilder::stroke(px(1.)), [point(r, t), point(l, mid), point(r, b)], false),
        _ => return,
    };
    path.add_polygon(&points, closed);
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}
