use super::*;

#[test]
fn line_bounds_and_vertical_moves_keep_the_column() {
    let t = "abcd\nxy\nlmnop";
    assert_eq!(line_bounds(t, 6), (5, 7));
    assert_eq!(vertical(t, 3, true), 7); // column 3 is past "xy": its end
    assert_eq!(vertical(t, 7, true), 10);
    assert_eq!(vertical(t, 2, false), 2); // first line: stays
    let u = "đỗ\nabc";
    assert_eq!(vertical(u, "đỗ".len(), true), "đỗ\nab".len()); // columns count chars, not bytes
}

#[test]
fn line_col_and_pos_round_trip() {
    let b = Buffer::new("ab\r\nđỗc\n\nz");
    assert_eq!(b.text, "ab\nđỗc\n\nz"); // CRLF read as LF
    let at = b.pos(1, 2);
    assert_eq!(&b.text[at..at + 1], "c");
    assert_eq!(b.line_col(at), (1, 2));
    assert_eq!(b.pos(1, 99), "ab\nđỗc".len()); // past the end: the line's end
    assert_eq!(b.pos(9, 0), b.text.len());
    assert_eq!(b.line_count(), 4);
}

#[test]
fn typing_a_word_is_one_undo_step_and_redo_brings_it_back() {
    let mut b = Buffer::new("x");
    b.move_to(1, false);
    for c in [" ", "f", "o", "o"] { b.insert(c); }
    assert_eq!(b.text, "x foo");
    assert!(b.undo());
    assert_eq!(b.text, "x "); // the word goes in one step, the space was its own
    assert!(b.undo());
    assert_eq!(b.text, "x");
    assert!(!b.undo());
    assert!(b.redo());
    assert!(b.redo());
    assert_eq!(b.text, "x foo");
}

#[test]
fn selection_is_replaced_by_typing_and_removed_by_backspace() {
    let mut b = Buffer::new("hello world");
    b.move_to(0, false);
    b.move_to(5, true);
    assert_eq!(b.selected_text(), Some("hello"));
    b.insert("bye");
    assert_eq!(b.text, "bye world");
    b.move_to(3, false);
    b.move_to(b.text.len(), true);
    b.delete(false, false);
    assert_eq!(b.text, "bye");
}

#[test]
fn word_moves_and_word_delete() {
    let mut b = Buffer::new("let foo_bar = 42;");
    assert_eq!(b.word_right(0), 3);
    assert_eq!(b.word_right(3), 11);
    assert_eq!(b.word_left(11), 4);
    assert_eq!(b.word_left(4), 0);
    assert_eq!(b.word_at(6), 4..11); // inside foo_bar
    assert_eq!(b.word_at(3), 0..3); // right after "let"
    assert_eq!(b.word_at(12), 12..12); // on "=": nothing
    b.move_to(11, false);
    b.delete(false, true);
    assert_eq!(b.text, "let  = 42;");
}

#[test]
fn enter_keeps_indentation_and_home_toggles_to_it() {
    let mut b = Buffer::new("    fn x() {");
    b.move_to(b.text.len(), false);
    b.newline();
    assert_eq!(b.text, "    fn x() {\n    ");
    b.insert("y");
    assert_eq!(b.home(), "    fn x() {\n    ".len());
    b.move_to(b.home(), false);
    assert_eq!(b.home(), "    fn x() {\n".len());
}

#[test]
fn tab_indents_selected_lines_and_shift_tab_outdents() {
    let mut b = Buffer::new("a\nb\nc");
    b.move_to(0, false);
    b.move_to(3, true); // "a\nb"
    b.indent(false, "    ");
    assert_eq!(b.text, "    a\n    b\nc");
    b.indent(true, "    ");
    assert_eq!(b.text, "a\nb\nc");
    b.move_to(0, false);
    b.indent(false, "  "); // no selection: inserts
    assert_eq!(b.text, "  a\nb\nc");
}
