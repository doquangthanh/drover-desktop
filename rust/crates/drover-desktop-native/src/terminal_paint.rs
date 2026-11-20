//! Draw an alacritty grid with GPUI: one shaped line per row (runs per style), background quads, cursor, IME text.
//! Colours are the web UI's dark xterm theme (`core/theme.ts`), so both clients show the same terminal.

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::TermMode;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::{Color, NamedColor};
use drover_core::screen::PaneScreen;
use gpui::{
    App, Bounds, Font, FontStyle, FontWeight, Hsla, Pixels, SharedString, TextRun, UnderlineStyle, Window, fill, font,
    point, px, rgb, size,
};

/// The terminal font: family, size in px, and the row height (size x 1.15, as the web UI's xterm `lineHeight`).
#[derive(Clone)]
pub struct TermFont {
    pub font: Font,
    pub size: f32,
    pub line_height: f32,
}

impl TermFont {
    pub fn new(family: &str, size: f32) -> TermFont {
        TermFont { font: font(family.to_string()), size, line_height: (size * 1.15).round() }
    }
}

const BG: u32 = 0x17191d;
const FG: u32 = 0xd2d4d9;
const ANSI: [u32; 16] = [
    0x2c2e31, 0xdc8c95, 0xa6bd91, 0xd8be87, 0x96b2d7, 0xb5a2ce, 0x8cbdc3, 0xd2d4d9,
    0x818793, 0xe8a1a9, 0xb9cfa5, 0xe6cf9c, 0xaec5e4, 0xc8b7de, 0xa3cfd4, 0xf0f0f2,
];

fn indexed(i: u8) -> u32 {
    match i {
        0..=15 => ANSI[i as usize],
        16..=231 => {
            let i = i - 16;
            let level = |v: u8| if v == 0 { 0 } else { 55 + v as u32 * 40 };
            (level(i / 36) << 16) | (level((i / 6) % 6) << 8) | level(i % 6)
        }
        _ => {
            let g = 8 + (i as u32 - 232) * 10;
            (g << 16) | (g << 8) | g
        }
    }
}

fn color(c: Color, default: u32) -> u32 {
    match c {
        Color::Spec(rgb) => ((rgb.r as u32) << 16) | ((rgb.g as u32) << 8) | rgb.b as u32,
        Color::Indexed(i) => indexed(i),
        Color::Named(NamedColor::Foreground) | Color::Named(NamedColor::BrightForeground) => FG,
        Color::Named(NamedColor::Background) => BG,
        Color::Named(n) if (n as usize) < 16 => ANSI[n as usize],
        Color::Named(_) => default,
    }
}

fn hsla(c: u32) -> Hsla {
    rgb(c).into()
}

/// Width of one cell for the terminal font.
pub fn cell_width(tf: &TermFont, window: &Window) -> Pixels {
    let ts = window.text_system();
    ts.advance(ts.resolve_font(&tf.font), px(tf.size), 'm').map(|s| s.width).unwrap_or(px(tf.size * 0.6))
}

/// The text style of one cell: colour (dim halves it), weight, slant, underline.
fn run_for(flags: Flags, fg: u32, tf: &TermFont) -> TextRun {
    let mut color = hsla(fg);
    if flags.contains(Flags::DIM) {
        color.a = 0.6;
    }
    let mut font = tf.font.clone();
    if flags.contains(Flags::BOLD) {
        font.weight = FontWeight::BOLD;
    }
    if flags.contains(Flags::ITALIC) {
        font.style = FontStyle::Italic;
    }
    let underline = flags.intersects(Flags::ALL_UNDERLINES)
        .then(|| UnderlineStyle { thickness: px(1.), color: Some(color), wavy: flags.contains(Flags::UNDERCURL) });
    TextRun { len: 0, font, color, background_color: None, underline, strikethrough: None }
}

/// The selection highlight of the web UI's dark theme: rgba(150,178,215,.32).
fn selection_color() -> Hsla {
    let mut c = hsla(0x96b2d7);
    c.a = 0.32;
    c
}

/// Paint `screen` into `bounds`, scrolled by its display offset. `marked` is text the input method is still
/// composing, drawn at the cursor; `link` (grid line, columns) is underlined. Returns where the cursor cell is
/// (for the IME candidate window).
#[allow(clippy::too_many_arguments)]
pub fn paint(screen: &PaneScreen, tf: &TermFont, bounds: Bounds<Pixels>, focused: bool, marked: Option<&str>,
             link: Option<(i32, std::ops::Range<usize>)>, window: &mut Window, cx: &mut App) -> Bounds<Pixels> {
    let ts = window.text_system().clone();
    let cell_w = cell_width(tf, window);
    let lh = px(tf.line_height);
    let term = screen.term();
    let grid = term.grid();
    let (rows, cols) = (grid.screen_lines(), grid.columns());
    let offset = grid.display_offset() as i32;
    let selection = term.selection.as_ref().and_then(|s| s.to_range(term));
    for r in 0..rows {
        let y = bounds.origin.y + lh * r as f32;
        if y > bounds.origin.y + bounds.size.height {
            break;
        }
        let row = &grid[Line(r as i32 - offset)];
        let mut text = String::with_capacity(cols);
        let mut runs: Vec<TextRun> = Vec::new();
        for c in 0..cols {
            let cell = &row[Column(c)];
            if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                continue;
            }
            let (mut fg, mut bg) = (color(cell.fg, FG), color(cell.bg, BG));
            if cell.flags.contains(Flags::INVERSE) {
                std::mem::swap(&mut fg, &mut bg);
            }
            if bg != BG {
                let x = bounds.origin.x + cell_w * c as f32;
                let w = if cell.flags.contains(Flags::WIDE_CHAR) { cell_w * 2. } else { cell_w };
                window.paint_quad(fill(Bounds::new(point(x, y), size(w, lh)), hsla(bg)));
            }
            if selection.as_ref().is_some_and(|s| s.contains(Point::new(Line(r as i32 - offset), Column(c)))) {
                let w = if cell.flags.contains(Flags::WIDE_CHAR) { cell_w * 2. } else { cell_w };
                window.paint_quad(fill(Bounds::new(point(bounds.origin.x + cell_w * c as f32, y), size(w, lh)), selection_color()));
            }
            let mut ch = if cell.c == '\0' || cell.flags.contains(Flags::HIDDEN) { ' ' } else { cell.c };
            ch = crate::powerline::substitute(ch).unwrap_or(ch);
            if crate::powerline::is_drawn(ch) {
                let at = Bounds::new(point(bounds.origin.x + cell_w * c as f32, y), size(cell_w, lh));
                crate::powerline::paint(ch, at, hsla(fg), window);
                ch = ' ';
            }
            let start = text.len();
            text.push(ch);
            let mut run = run_for(cell.flags, fg, tf);
            run.len = text.len() - start;
            match runs.last_mut() {
                Some(last) if last.color == run.color && last.font == run.font && last.underline == run.underline => last.len += run.len,
                _ => runs.push(run),
            }
        }
        if text.trim_end().is_empty() {
            continue;
        }
        let line = ts.shape_line(SharedString::from(text), px(tf.size), &runs, Some(cell_w));
        let _ = line.paint(point(bounds.origin.x, y), lh, window, cx);
        if let Some((_, cols)) = link.as_ref().filter(|(l, _)| *l == r as i32 - offset) {
            let under = Bounds::new(point(bounds.origin.x + cell_w * cols.start as f32, y + lh - px(2.)),
                                    size(cell_w * cols.len() as f32, px(1.)));
            window.paint_quad(fill(under, hsla(FG)));
        }
    }
    let cur = grid.cursor.point;
    let cursor = Bounds::new(point(bounds.origin.x + cell_w * cur.column.0 as f32, bounds.origin.y + lh * (cur.line.0 + offset) as f32),
                             size(cell_w, lh));
    if offset == 0 && term.mode().contains(TermMode::SHOW_CURSOR) {
        window.paint_quad(fill(cursor, gpui::hsla(0., 0., 0.82, if focused { 0.8 } else { 0.3 })));
    }
    if let Some(text) = marked.filter(|t| !t.is_empty()) {
        let mut run = run_for(Flags::UNDERLINE, FG, tf);
        run.len = text.len();
        run.background_color = Some(hsla(0x2c3340));
        let line = ts.shape_line(SharedString::from(text.to_string()), px(tf.size), &[run], None);
        let _ = line.paint(cursor.origin, lh, window, cx);
    }
    cursor
}
