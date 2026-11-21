//! Kitty graphics protocol, the wire side: cut `ESC _G <keys> ; <base64> ESC \` commands out of a pane's output and
//! read their keys. Output arrives in arbitrary chunks, so a command may start in one chunk and end in another.

/// A piece of output: plain text for the terminal, or the inside of one graphics command.
#[derive(Debug, PartialEq)]
pub enum Piece {
    Text(String),
    Graphics(String),
}

const START: &str = "\x1b_G";
/// A command that never ends is dropped after this much, so one bad program cannot fill memory.
const MAX_OPEN: usize = 64 << 20;

#[derive(Default)]
pub struct Splitter {
    /// The start of a command whose end has not arrived yet (or a lone ESC / ESC _ that may become one).
    carry: String,
}

impl Splitter {
    /// None: there is nothing to cut out, feed `data` as it is.
    pub fn split(&mut self, data: &str) -> Option<Vec<Piece>> {
        if self.carry.is_empty() && !data.contains(START) && !data.ends_with('\x1b') && !data.ends_with("\x1b_") {
            return None;
        }
        let whole = std::mem::take(&mut self.carry) + data;
        let (mut out, mut rest) = (vec![], whole.as_str());
        while let Some(i) = rest.find(START) {
            if i > 0 { out.push(Piece::Text(rest[..i].to_string())); }
            let body = &rest[i + START.len()..];
            let end = body.find("\x1b\\").map(|e| (e, 2)).or_else(|| body.find('\x07').map(|e| (e, 1)));
            let Some((e, len)) = end else {
                if rest.len() - i <= MAX_OPEN { self.carry = rest[i..].to_string(); }
                return Some(out);
            };
            out.push(Piece::Graphics(body[..e].to_string()));
            rest = &body[e + len..];
        }
        let keep = if rest.ends_with("\x1b_") { 2 } else if rest.ends_with('\x1b') { 1 } else { 0 };
        let (text, tail) = rest.split_at(rest.len() - keep);
        if !text.is_empty() { out.push(Piece::Text(text.to_string())); }
        self.carry = tail.to_string();
        Some(out)
    }
}

/// The keys of one command (the ones Drover uses) and its base64 payload.
#[derive(Debug, Clone)]
pub struct Cmd {
    /// a: t transmit (default), T transmit and show, p show, d delete, q query.
    pub action: char,
    /// f: 24 RGB, 32 RGBA (default), 100 PNG.
    pub format: u32,
    /// t: d inline (default), f file, t temporary file.
    pub medium: char,
    pub width: u32,
    pub height: u32,
    pub id: u32,
    pub cols: u32,
    pub rows: u32,
    /// m=1: more chunks follow.
    pub more: bool,
    /// o=z: zlib compressed.
    pub zlib: bool,
    /// q: 1 no OK replies, 2 no replies.
    pub quiet: u8,
    /// C=1: leave the cursor where it is.
    pub keep_cursor: bool,
    /// d: what to delete (a all, i by id; capital also frees the image).
    pub delete: char,
    pub data: String,
}

pub fn parse(payload: &str) -> Cmd {
    let (keys, data) = payload.split_once(';').unwrap_or((payload, ""));
    let mut c = Cmd { action: 't', format: 32, medium: 'd', width: 0, height: 0, id: 0, cols: 0, rows: 0, more: false, zlib: false,
                      quiet: 0, keep_cursor: false, delete: 'a', data: data.to_string() };
    for (k, v) in keys.split(',').filter_map(|kv| kv.split_once('=')) {
        let (ch, n) = (v.chars().next().unwrap_or(' '), v.parse::<u32>().unwrap_or(0));
        match k {
            "a" => c.action = ch,
            "f" => c.format = n,
            "t" => c.medium = ch,
            "s" => c.width = n,
            "v" => c.height = n,
            "i" => c.id = n,
            "c" => c.cols = n,
            "r" => c.rows = n,
            "m" => c.more = n == 1,
            "o" => c.zlib = ch == 'z',
            "q" => c.quiet = n as u8,
            "C" => c.keep_cursor = n == 1,
            "d" => c.delete = ch,
            _ => {}
        }
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_is_left_alone() {
        assert_eq!(Splitter::default().split("hello \x1b[31mred\x1b[0m"), None);
    }

    #[test]
    fn cuts_commands_out_of_text_and_across_chunks() {
        let mut s = Splitter::default();
        let a = s.split("ab\x1b_Ga=T,i=3;AAAA\x1b\\cd").unwrap();
        assert_eq!(a, vec![Piece::Text("ab".into()), Piece::Graphics("a=T,i=3;AAAA".into()), Piece::Text("cd".into())]);
        assert_eq!(s.split("x\x1b_Ga=T,m=1;AA").unwrap(), vec![Piece::Text("x".into())]);
        assert_eq!(s.split("AA\x1b").unwrap(), vec![]);
        assert_eq!(s.split("\\y").unwrap(), vec![Piece::Graphics("a=T,m=1;AAAA".into()), Piece::Text("y".into())]);
        // A lone ESC at the end is held back until the next chunk shows what it is.
        assert_eq!(s.split("z\x1b").unwrap(), vec![Piece::Text("z".into())]);
        assert_eq!(s.split("[0m").unwrap(), vec![Piece::Text("\x1b[0m".into())]);
    }

    #[test]
    fn reads_the_keys() {
        let c = parse("a=T,f=100,i=7,c=4,r=2,m=1,q=2,C=1,o=z;QUJD");
        assert_eq!((c.action, c.format, c.id, c.cols, c.rows, c.more, c.quiet, c.keep_cursor, c.zlib), ('T', 100, 7, 4, 2, true, 2, true, true));
        assert_eq!(c.data, "QUJD");
        assert_eq!((parse("").action, parse("").format, parse("").medium), ('t', 32, 'd'));
    }
}
