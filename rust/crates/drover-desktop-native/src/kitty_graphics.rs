//! Kitty graphics protocol for a pane: programs (icat, yazi, chafa, agents) show real images inside the terminal.
//! `Graphics::feed` takes the pane's output instead of the screen: text goes to the screen as before, each
//! `ESC _G … ESC \` command is decoded here. An image is placed at the cursor (anchored to a line of the scrollback, so
//! it scrolls with the text), the cursor moves past it, and `paint` draws it under the text. Queries are answered
//! so programs know the terminal can show images. Not done: crop/offset keys, z-order, shared memory, animation.

use std::io::Read;
use std::sync::Arc;

use alacritty_terminal::grid::Dimensions;
use base64::{Engine, alphabet, engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig}};
use drover_core::screen::PaneScreen;
use gpui::{Bounds, ContentMask, Corners, Pixels, RenderImage, Window, point, size};

use crate::kitty_command::{self, Cmd, Piece, Splitter};

const B64: GeneralPurpose = GeneralPurpose::new(&alphabet::STANDARD, GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent));
/// Largest image kept (decoded RGBA), most images kept, most placements kept, longest pending upload.
const MAX_BYTES: u64 = 128 << 20;
const MAX_IMAGES: usize = 48;
const MAX_PLACEMENTS: usize = 256;
const MAX_UPLOAD: usize = 64 << 20;

struct Image {
    id: u32,
    render: Arc<RenderImage>,
    width: u32,
    height: u32,
}

/// Where an image is shown: `line` counts from the oldest scrollback line, so it stays put as output scrolls.
struct Placement {
    image: u32,
    line: i64,
    col: usize,
    cols: usize,
    rows: usize,
}

#[derive(Default)]
pub struct Graphics {
    splitter: Splitter,
    /// The first chunk of an upload that is still arriving (m=1).
    pending: Option<Cmd>,
    images: Vec<Image>,
    placements: Vec<Placement>,
    next_id: u32,
}

impl Graphics {
    /// Apply `data` to `screen` (text) and to the images (commands). Returns what to send back to the program.
    pub fn feed(&mut self, screen: &mut PaneScreen, data: &str, cell: (f32, f32)) -> Vec<String> {
        let mut replies = vec![];
        let Some(pieces) = self.splitter.split(data) else {
            self.text(screen, data);
            return replies;
        };
        for piece in pieces {
            match piece {
                Piece::Text(t) => self.text(screen, &t),
                Piece::Graphics(g) => self.command(screen, kitty_command::parse(&g), cell, &mut replies),
            }
        }
        replies
    }

    /// Clearing the screen or switching to / from the alternate screen takes the images with it.
    fn text(&mut self, screen: &mut PaneScreen, text: &str) {
        if !self.placements.is_empty() && ["\x1b[2J", "\x1b[3J", "\x1bc", "\x1b[?1049", "\x1b[?47"].iter().any(|s| text.contains(s)) {
            self.placements.clear();
        }
        screen.feed(text.as_bytes());
    }

    fn command(&mut self, screen: &mut PaneScreen, c: Cmd, cell: (f32, f32), replies: &mut Vec<String>) {
        let c = match self.pending.take() {
            Some(mut first) => { first.data.push_str(&c.data); first.more = c.more; first }
            None => c,
        };
        if c.more {
            if c.data.len() <= MAX_UPLOAD { self.pending = Some(c); }
            return;
        }
        match c.action {
            'q' => reply(replies, &c, decode(&c).err()),
            't' | 'T' => match decode(&c) {
                Ok((width, height, render)) => {
                    let id = if c.id != 0 { c.id } else { self.next_id += 1; u32::MAX - self.next_id };
                    self.images.retain(|i| i.id != id);
                    self.images.push(Image { id, render, width, height });
                    if self.images.len() > MAX_IMAGES {
                        let old = self.images.remove(0).id;
                        self.placements.retain(|p| p.image != old);
                    }
                    if c.action == 'T' { self.place(screen, id, &c, cell); }
                    reply(replies, &c, None);
                }
                Err(e) => reply(replies, &c, Some(e)),
            },
            'p' if self.images.iter().any(|i| i.id == c.id) => { self.place(screen, c.id, &c, cell); reply(replies, &c, None); }
            'p' => reply(replies, &c, Some("ENOENT:no such image".into())),
            'd' => match c.delete {
                'a' | 'A' => self.placements.clear(),
                'i' | 'I' => {
                    self.placements.retain(|p| p.image != c.id);
                    if c.delete == 'I' { self.images.retain(|i| i.id != c.id); }
                }
                _ => {}
            },
            _ => {}
        }
    }

    /// Show image `id` at the cursor, then move the cursor to the last row of the picture, past its right edge.
    fn place(&mut self, screen: &mut PaneScreen, id: u32, c: &Cmd, (cw, lh): (f32, f32)) {
        let Some(img) = self.images.iter().find(|i| i.id == id) else { return };
        let grid = screen.term().grid();
        let (cursor, columns) = (grid.cursor.point, grid.columns());
        let col = cursor.column.0.min(columns - 1);
        let cols = (if c.cols > 0 { c.cols as usize } else { (img.width as f32 / cw).ceil() as usize }).clamp(1, columns - col);
        let rows = (if c.rows > 0 { c.rows as usize } else { (img.height as f32 / lh).ceil() as usize }).clamp(1, 500);
        let line = (grid.total_lines() - grid.screen_lines()) as i64 + cursor.line.0 as i64;
        self.placements.push(Placement { image: id, line, col, cols, rows });
        if self.placements.len() > MAX_PLACEMENTS { self.placements.remove(0); }
        if !c.keep_cursor {
            screen.feed(format!("{}\x1b[{}G", "\n".repeat(rows - 1), (col + cols + 1).min(columns)).as_bytes());
        }
    }

    /// Draw the images that are on screen, clipped to `bounds` (they go under the text).
    pub fn paint(&self, screen: &PaneScreen, bounds: Bounds<Pixels>, cell_w: Pixels, lh: Pixels, window: &mut Window) {
        if self.placements.is_empty() { return }
        let grid = screen.term().grid();
        let (history, offset, rows) = ((grid.total_lines() - grid.screen_lines()) as i64, grid.display_offset() as i64, grid.screen_lines() as i64);
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            for p in &self.placements {
                let top = p.line - history + offset;
                let Some(img) = self.images.iter().find(|i| i.id == p.image).filter(|_| top + p.rows as i64 > 0 && top < rows) else { continue };
                let at = Bounds::new(point(bounds.origin.x + cell_w * p.col as f32, bounds.origin.y + lh * top as f32),
                                     size(cell_w * p.cols as f32, lh * p.rows as f32));
                let _ = window.paint_image(at, Corners::default(), img.render.clone(), 0, false);
            }
        });
    }
}

/// `ESC _G i=<id>;OK ESC \` or the error, when the program gave an id and did not ask for silence.
fn reply(replies: &mut Vec<String>, c: &Cmd, error: Option<String>) {
    if c.id == 0 { return }
    match error {
        None if c.quiet == 0 => replies.push(format!("\x1b_Gi={};OK\x1b\\", c.id)),
        Some(e) if c.quiet < 2 => replies.push(format!("\x1b_Gi={};{e}\x1b\\", c.id)),
        _ => {}
    }
}

/// The upload as (width, height, image for gpui).
fn decode(c: &Cmd) -> Result<(u32, u32, Arc<RenderImage>), String> {
    let bad = |what: &str| format!("EINVAL:{what}");
    let bytes = B64.decode(c.data.trim()).map_err(|_| bad("bad base64"))?;
    let raw = match c.medium {
        'd' => bytes,
        'f' | 't' => {
            let path = String::from_utf8(bytes).map_err(|_| bad("bad path"))?;
            let len = std::fs::metadata(&path).map_err(|e| format!("ENOENT:{e}"))?.len();
            if len > MAX_BYTES { return Err(bad("file too large")) }
            let data = std::fs::read(&path).map_err(|e| format!("ENOENT:{e}"))?;
            if c.medium == 't' && path.contains("tty-graphics-protocol") { let _ = std::fs::remove_file(&path); }
            data
        }
        _ => return Err(bad("unsupported transmission")),
    };
    let raw = if c.zlib {
        let mut out = vec![];
        flate2::read::ZlibDecoder::new(&raw[..]).take(MAX_BYTES).read_to_end(&mut out).map_err(|_| bad("bad zlib data"))?;
        out
    } else { raw };
    let (w, h, mut rgba) = match c.format {
        100 => {
            let img = image::load_from_memory_with_format(&raw, image::ImageFormat::Png).map_err(|_| bad("bad PNG"))?.to_rgba8();
            (img.width(), img.height(), img.into_raw())
        }
        24 | 32 => {
            let per = if c.format == 24 { 3 } else { 4 };
            if c.width == 0 || c.height == 0 || c.width as u64 * c.height as u64 * 4 > MAX_BYTES { return Err(bad("bad size")) }
            if raw.len() != (c.width * c.height) as usize * per { return Err(bad("data does not match s and v")) }
            let rgba = if per == 4 { raw } else { raw.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect() };
            (c.width, c.height, rgba)
        }
        _ => return Err(bad("unsupported format")),
    };
    if w as u64 * h as u64 * 4 > MAX_BYTES { return Err(bad("image too large")) }
    for px in rgba.chunks_exact_mut(4) { px.swap(0, 2); } // gpui paints BGRA
    let buffer = image::RgbaImage::from_raw(w, h, rgba).ok_or_else(|| bad("bad size"))?;
    Ok((w, h, Arc::new(RenderImage::new(vec![image::Frame::new(buffer)]))))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn upload(action: &str, extra: &str) -> String {
        let pixels = B64.encode([255u8, 0, 0, 255].repeat(4)); // 2x2 red
        format!("\x1b_Ga={action},f=32,s=2,v=2,{extra};{pixels}\x1b\\")
    }

    #[test]
    fn shows_an_image_at_the_cursor_and_moves_past_it() {
        let (mut screen, mut g) = (PaneScreen::new(20, 6, 100), Graphics::default());
        let replies = g.feed(&mut screen, &format!("hi{}x", upload("T", "i=5,c=3,r=2")), (8., 16.));
        assert_eq!(replies, vec!["\x1b_Gi=5;OK\x1b\\".to_string()]);
        assert_eq!((g.placements.len(), g.placements[0].col, g.placements[0].cols, g.placements[0].rows), (1, 2, 3, 2));
        // 2 columns of text + 3 of image: the cursor is on the picture's last row, so "x" lands at column 5.
        assert_eq!(screen.term().grid().cursor.point.line.0, 1);
        assert_eq!(screen.term().grid().cursor.point.column.0, 6);
    }

    #[test]
    fn size_comes_from_pixels_and_the_cell_size_when_not_given() {
        let (mut screen, mut g) = (PaneScreen::new(20, 6, 100), Graphics::default());
        g.feed(&mut screen, &upload("T", "C=1"), (1., 1.));
        assert_eq!((g.placements[0].cols, g.placements[0].rows), (2, 2));
        assert_eq!(screen.term().grid().cursor.point.line.0, 0); // C=1: the cursor stays
    }

    #[test]
    fn queries_answer_and_bad_uploads_report_errors() {
        let (mut screen, mut g) = (PaneScreen::new(20, 6, 100), Graphics::default());
        assert_eq!(g.feed(&mut screen, &upload("q", "i=31"), (8., 16.)), vec!["\x1b_Gi=31;OK\x1b\\".to_string()]);
        assert!(g.placements.is_empty());
        let bad = g.feed(&mut screen, "\x1b_Ga=T,f=32,s=9,v=9,i=2;AAAA\x1b\\", (8., 16.));
        assert!(bad[0].starts_with("\x1b_Gi=2;EINVAL"));
        assert!(g.feed(&mut screen, &upload("T", "q=2"), (8., 16.)).is_empty());
    }

    #[test]
    fn delete_and_clear_remove_placements() {
        let (mut screen, mut g) = (PaneScreen::new(20, 6, 100), Graphics::default());
        g.feed(&mut screen, &(upload("T", "i=1") + &upload("T", "i=2")), (8., 16.));
        assert_eq!(g.placements.len(), 2);
        g.feed(&mut screen, "\x1b_Ga=d,d=i,i=1\x1b\\", (8., 16.));
        assert_eq!(g.placements.len(), 1);
        g.feed(&mut screen, "\x1b[2J", (8., 16.));
        assert!(g.placements.is_empty());
    }

    #[test]
    fn chunked_uploads_join() {
        let (mut screen, mut g) = (PaneScreen::new(20, 6, 100), Graphics::default());
        let all = B64.encode([0u8, 255, 0, 255].repeat(4));
        let (a, b) = all.split_at(8);
        g.feed(&mut screen, &format!("\x1b_Ga=T,f=32,s=2,v=2,i=9,m=1;{a}\x1b\\\x1b_Gm=0;{b}\x1b\\"), (8., 16.));
        assert_eq!((g.images.len(), g.placements.len()), (1, 1));
    }
}
