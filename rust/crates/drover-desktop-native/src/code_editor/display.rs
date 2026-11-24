//! How a line is drawn: tabs expanded to the next stop of 4, the input method's word inserted at the caret, and the
//! colour, selection and underline ranges merged into runs that never overlap (what StyledText needs). Pure.

use std::ops::Range;

pub const TAB: usize = 4;

/// A line as drawn: its text, and for every byte of the source line (plus its end) the byte in `text`.
pub struct Shown {
    pub text: String,
    map: Vec<usize>,
}

impl Shown {
    pub fn new(line: &str) -> Self {
        let (mut text, mut map, mut col) = (String::with_capacity(line.len()), Vec::with_capacity(line.len() + 1), 0);
        for c in line.chars() {
            let at = text.len();
            map.extend(std::iter::repeat_n(at, c.len_utf8()));
            if c == '\t' {
                let n = TAB - col % TAB;
                text.extend(std::iter::repeat_n(' ', n));
                col += n;
            } else {
                text.push(c);
                col += 1;
            }
        }
        map.push(text.len());
        Shown { text, map }
    }

    /// The drawn byte of source byte `b` (clamped to the line).
    pub fn at(&self, b: usize) -> usize { self.map[b.min(self.map.len() - 1)] }

    pub fn range(&self, r: &Range<usize>) -> Range<usize> { self.at(r.start)..self.at(r.end) }

    /// Column (in drawn chars) of source byte `b`: where the caret is drawn.
    pub fn col(&self, b: usize) -> usize { self.text[..self.at(b)].chars().count() }

    /// Source char column for a drawn column (a click): inside a tab it goes to the nearer side.
    pub fn source_col(line: &str, drawn: usize) -> usize {
        let mut col = 0;
        for (i, c) in line.chars().enumerate() {
            let w = if c == '\t' { TAB - col % TAB } else { 1 };
            if drawn < col + w.div_ceil(2) { return i }
            col += w;
        }
        line.chars().count()
    }

    /// Put the input method's word in at drawn byte `at`.
    pub fn insert(&mut self, at: usize, s: &str) {
        self.text.insert_str(at, s);
        for m in self.map.iter_mut().filter(|m| **m >= at) { *m += s.len(); }
    }
}

/// Split overlapping styled ranges into sorted runs; each run gets every style that covers it, later ones on top.
pub fn merge<S: Clone>(ranges: &[(Range<usize>, S)], combine: impl Fn(&mut S, &S), empty: S) -> Vec<(Range<usize>, S)> {
    let mut cuts: Vec<usize> = ranges.iter().flat_map(|(r, _)| [r.start, r.end]).collect();
    cuts.sort_unstable();
    cuts.dedup();
    let mut out = Vec::new();
    for w in cuts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let mut style: Option<S> = None;
        for (r, s) in ranges.iter().filter(|(r, _)| r.start <= a && b <= r.end) {
            let _ = r;
            combine(style.get_or_insert_with(|| empty.clone()), s);
        }
        if let Some(s) = style { out.push((a..b, s)); }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_expand_to_stops_and_map_back() {
        let s = Shown::new("a\tb\t\tc");
        assert_eq!(s.text, "a   b       c");
        assert_eq!(s.at(2), 4); // "b"
        assert_eq!(s.col(2), 4);
        assert_eq!(Shown::source_col("a\tb", 0), 0);
        assert_eq!(Shown::source_col("a\tb", 1), 1); // the tab's first half: before it
        assert_eq!(Shown::source_col("a\tb", 3), 2); // its second half: after it
        assert_eq!(Shown::source_col("a\tb", 9), 3); // past the end
        let u = Shown::new("đ\tx");
        assert_eq!(u.text, "đ   x");
        assert_eq!(u.col("đ\t".len()), 4);
    }

    #[test]
    fn inserted_word_shifts_what_follows() {
        let mut s = Shown::new("ab");
        s.insert(1, "XY");
        assert_eq!(s.text, "aXYb");
        assert_eq!(s.at(1), 3);
        assert_eq!(s.at(0), 0);
    }

    #[test]
    fn merge_splits_overlaps_into_runs() {
        let runs = merge(&[(0..6, 1u32), (2..4, 10)], |a, b| *a += b, 0);
        assert_eq!(runs, vec![(0..2, 1), (2..4, 11), (4..6, 1)]);
        assert!(merge::<u32>(&[], |a, b| *a += b, 0).is_empty());
        assert_eq!(merge(&[(0..2, 1u32), (5..6, 2)], |a, b| *a += b, 0), vec![(0..2, 1), (5..6, 2)]);
    }
}
