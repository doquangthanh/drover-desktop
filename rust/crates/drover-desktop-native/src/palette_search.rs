//! Palette search (pure): the same ranking as the web UI's `searchCommands` (frontend/src/overlays/palette-commands.ts).
//! Substring hits first (label start, then word start, then inside the label); if nothing contains the text, the
//! letters in order ("jnat" finds "Jump to next agent that needs you"). Accents are ignored, so "viet" finds "Việt".

/// One match: which entry, and which letters of its label matched (char indexes, for highlighting).
#[derive(Debug, PartialEq, Eq)]
pub struct Hit {
    pub index: usize,
    pub marks: Vec<usize>,
}

/// Lower case without Vietnamese accents: "filé" finds "Files", "Viet" finds "Việt" (Telex composes while you type).
/// One char in, one char out, so indexes into the folded text are indexes into the original.
pub fn fold(s: &str) -> String {
    const GROUPS: [(&str, char); 6] = [
        ("àáảãạăằắẳẵặâầấẩẫậ", 'a'), ("èéẻẽẹêềếểễệ", 'e'), ("ìíỉĩị", 'i'),
        ("òóỏõọôồốổỗộơờớởỡợ", 'o'), ("ùúủũụưừứửữự", 'u'), ("ỳýỷỹỵ", 'y'),
    ];
    s.to_lowercase().chars().map(|c| match c {
        'đ' => 'd',
        c => GROUPS.iter().find(|(set, _)| set.contains(c)).map(|(_, base)| *base).unwrap_or(c),
    }).collect()
}

fn subsequence(text: &[char], q: &[char]) -> Option<Vec<usize>> {
    let mut marks = Vec::with_capacity(q.len());
    let mut from = 0;
    for ch in q {
        let i = text[from..].iter().position(|c| c == ch)? + from;
        marks.push(i);
        from = i + 1;
    }
    Some(marks)
}

/// Entries matching `query`, best first. An empty query keeps everything in the given order.
pub fn search<S: AsRef<str>>(labels: &[S], query: &str) -> Vec<Hit> {
    let q: Vec<char> = fold(query.trim()).chars().collect();
    if q.is_empty() {
        return (0..labels.len()).map(|index| Hit { index, marks: vec![] }).collect();
    }
    let folded: Vec<Vec<char>> = labels.iter().map(|l| fold(l.as_ref()).chars().collect()).collect();
    let mut scored: Vec<(u8, Hit)> = Vec::new();
    for (index, text) in folded.iter().enumerate() {
        let found = text.windows(q.len()).position(|w| w == q.as_slice());
        if let Some(at) = found {
            let word_start = at == 0 || !text[at - 1].is_alphanumeric();
            let rank = if at == 0 { 0 } else if word_start { 1 } else { 2 };
            scored.push((rank, Hit { index, marks: (at..at + q.len()).collect() }));
        }
    }
    if !scored.is_empty() {
        scored.sort_by_key(|(rank, _)| *rank); // stable: ties keep the list order
        return scored.into_iter().map(|(_, h)| h).collect();
    }
    let letters: Vec<char> = q.iter().copied().filter(|c| !c.is_whitespace()).collect();
    if letters.len() < 2 {
        return vec![];
    }
    folded.iter().enumerate().filter_map(|(index, text)| subsequence(text, &letters).map(|marks| Hit { index, marks })).collect()
}

/// Byte ranges of the marked letters of `label`, neighbours merged: what a styled text run needs.
pub fn mark_ranges(label: &str, marks: &[usize]) -> Vec<std::ops::Range<usize>> {
    let starts: Vec<usize> = label.char_indices().map(|(b, _)| b).collect();
    let mut out: Vec<std::ops::Range<usize>> = Vec::new();
    for &m in marks {
        let Some(&start) = starts.get(m) else { continue };
        let end = starts.get(m + 1).copied().unwrap_or(label.len());
        match out.last_mut() {
            Some(last) if last.end == start => last.end = end,
            _ => out.push(start..end),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const LABELS: [&str; 5] = ["Split right", "Split down", "Toggle split view", "Jump to next agent that needs you", "Việt hóa"];

    fn found(q: &str) -> Vec<usize> { search(&LABELS, q).into_iter().map(|h| h.index).collect() }

    #[test]
    fn label_start_comes_before_word_start_before_inside() {
        assert_eq!(found("split"), vec![0, 1, 2]);
        let labels = ["Close pane", "Pane list", "Open spaces"];
        let order: Vec<usize> = search(&labels, "pane").into_iter().map(|h| h.index).collect();
        assert_eq!(order, vec![1, 0]);
    }

    #[test]
    fn marks_are_the_matched_letters() {
        let hit = &search(&LABELS, "down")[0];
        assert_eq!((hit.index, hit.marks.clone()), (1, vec![6, 7, 8, 9]));
    }

    #[test]
    fn falls_back_to_letters_in_order() {
        assert_eq!(found("jnat"), vec![3]);
        assert!(found("zzzz").is_empty());
        assert!(found("q").is_empty(), "one stray letter is not a search");
    }

    #[test]
    fn ignores_accents_and_case() {
        assert_eq!(found("VIET"), vec![4]);
        assert_eq!(found("việt"), vec![4]);
    }

    #[test]
    fn empty_query_keeps_everything_in_order() {
        assert_eq!(found("  "), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn mark_ranges_merge_neighbours_and_count_bytes() {
        assert_eq!(mark_ranges("Split down", &[6, 7, 8, 9]), vec![6..10]);
        assert_eq!(mark_ranges("jump", &[0, 2]), vec![0..1, 2..3]);
        assert_eq!(mark_ranges("Việt", &[1, 2]), vec![1..5]); // "i" is 1 byte, "ệ" is 3
    }
}
