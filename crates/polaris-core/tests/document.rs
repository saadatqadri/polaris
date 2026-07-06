//! Integration tests for the Document API — the acceptance suite for M1.
//! Everything the old TUI buffer got wrong (byte-indexed Unicode editing)
//! must be impossible by construction here.

use polaris_core::{AutosaveTimer, Document};

fn type_str(doc: &mut Document, s: &str) {
    for c in s.chars() {
        doc.insert_char(c);
    }
}

// --- Unicode editing (the old byte-panic class) ---

#[test]
fn typing_multibyte_then_more_is_safe() {
    let mut doc = Document::new();
    type_str(&mut doc, "éx — “ok” 👍");
    assert_eq!(doc.text(), "éx — “ok” 👍");
}

#[test]
fn backspace_removes_whole_grapheme() {
    // é as e + combining acute: two chars, one grapheme, one backspace
    let mut doc = Document::from_str("caf\u{65}\u{301}");
    doc.move_line_end(false);
    doc.backspace();
    assert_eq!(doc.text(), "caf");
}

#[test]
fn backspace_removes_whole_zwj_emoji() {
    let mut doc = Document::from_str("a\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}");
    doc.move_line_end(false);
    doc.backspace();
    assert_eq!(doc.text(), "a");
}

#[test]
fn arrow_keys_step_graphemes_not_chars() {
    let mut doc = Document::from_str("e\u{301}x"); // é(2 chars) + x
    doc.move_right(false);
    assert_eq!(doc.cursor().pos, 2); // past the full é cluster
    doc.move_right(false);
    assert_eq!(doc.cursor().pos, 3);
    doc.move_left(false);
    doc.move_left(false);
    assert_eq!(doc.cursor().pos, 0);
}

#[test]
fn cjk_editing() {
    let mut doc = Document::new();
    type_str(&mut doc, "日本語のテキスト");
    doc.backspace();
    assert_eq!(doc.text(), "日本語のテキス");
    // UAX-29 segments CJK by ideograph / kana run
    assert_eq!(doc.word_count(), 5);
}

// --- newline / line movement ---

#[test]
fn newline_split_and_vertical_movement_with_sticky_column() {
    let mut doc = Document::new();
    type_str(&mut doc, "a long first line");
    doc.insert_newline();
    type_str(&mut doc, "ab");
    doc.insert_newline();
    type_str(&mut doc, "the third line here");

    // cursor at end of line 2; up to short line 1 clamps, up again restores col
    let col = 19;
    doc.move_up(false);
    assert_eq!(doc.line_col(), (1, 2)); // clamped to "ab"
    doc.move_up(false);
    assert_eq!(doc.line_col(), (0, col.min(17)));
    doc.move_down(false);
    assert_eq!(doc.line_col(), (1, 2)); // sticky col survives round trip
    doc.move_down(false);
    assert_eq!(doc.line_col(), (2, 19));
}

#[test]
fn home_end() {
    let mut doc = Document::from_str("one\ntwo three");
    doc.set_cursor_pos(8, false);
    doc.move_line_start(false);
    assert_eq!(doc.cursor().pos, 4);
    doc.move_line_end(false);
    assert_eq!(doc.cursor().pos, 13);
}

// --- word jump ---

#[test]
fn word_jump() {
    let mut doc = Document::from_str("hello brave new world");
    doc.move_word_right(false);
    assert_eq!(doc.cursor().pos, 5); // end of "hello"
    doc.move_word_right(false);
    assert_eq!(doc.cursor().pos, 11); // end of "brave"
    doc.move_line_end(false);
    doc.move_word_left(false);
    assert_eq!(doc.cursor().pos, 16); // start of "world"
}

// --- selection ---

#[test]
fn selection_and_replace() {
    let mut doc = Document::from_str("hello world");
    doc.move_word_right(true); // select "hello"
    assert_eq!(doc.selected_text().as_deref(), Some("hello"));
    doc.insert_str("goodbye");
    assert_eq!(doc.text(), "goodbye world");
    // undo restores both the deletion and the insertion
    doc.undo();
    assert_eq!(doc.text(), "hello world");
}

#[test]
fn selection_backspace_deletes_selection_only() {
    let mut doc = Document::from_str("hello world");
    doc.select_all();
    doc.backspace();
    assert_eq!(doc.text(), "");
    doc.undo();
    assert_eq!(doc.text(), "hello world");
}

#[test]
fn plain_movement_collapses_selection() {
    let mut doc = Document::from_str("abc");
    doc.select_all();
    doc.move_left(false);
    assert_eq!(doc.selection(), None);
    assert_eq!(doc.cursor().pos, 0); // collapse to selection start
}

// --- undo grouping ---

#[test]
fn undo_removes_word_sized_chunks() {
    let mut doc = Document::new();
    type_str(&mut doc, "hello brave world");
    doc.undo();
    assert_eq!(doc.text(), "hello brave "); // last word gone, not last char
    doc.undo();
    assert_eq!(doc.text(), "hello ");
    doc.undo();
    assert_eq!(doc.text(), "");
    assert!(!doc.undo());
}

#[test]
fn pause_commits_a_group() {
    let mut doc = Document::new();
    type_str(&mut doc, "abc");
    doc.commit_undo_group(); // e.g. the GUI's pause timer fired
    type_str(&mut doc, "def");
    doc.undo();
    assert_eq!(doc.text(), "abc");
}

#[test]
fn backspace_run_undoes_together() {
    let mut doc = Document::new();
    type_str(&mut doc, "hello");
    doc.backspace();
    doc.backspace();
    doc.backspace();
    assert_eq!(doc.text(), "he");
    doc.undo();
    assert_eq!(doc.text(), "hello"); // one undo restores the whole run
}

#[test]
fn redo_roundtrip_and_new_edit_clears_redo() {
    let mut doc = Document::new();
    type_str(&mut doc, "one two");
    doc.undo();
    assert_eq!(doc.text(), "one ");
    assert!(doc.redo());
    assert_eq!(doc.text(), "one two");
    doc.undo();
    type_str(&mut doc, "three");
    assert!(!doc.redo());
    assert_eq!(doc.text(), "one three");
}

#[test]
fn undo_restores_cursor() {
    let mut doc = Document::new();
    type_str(&mut doc, "hello world");
    doc.undo();
    assert_eq!(doc.cursor().pos, doc.text().chars().count());
    assert_eq!(doc.text(), "hello ");
}

// --- replace_range (front-end sync shim path) ---

#[test]
fn replace_range_typing_simulation_groups_into_words() {
    // A text_editor-style front-end feeds one replace_range per keystroke.
    let mut doc = Document::new();
    for (i, c) in "hello world".chars().enumerate() {
        doc.replace_range(i..i, &c.to_string());
    }
    assert_eq!(doc.text(), "hello world");
    doc.undo();
    assert_eq!(doc.text(), "hello "); // word-sized undo, same as insert_char
    doc.undo();
    assert_eq!(doc.text(), "");
}

#[test]
fn replace_range_replaces_and_deletes() {
    let mut doc = Document::from_str("hello world");
    doc.replace_range(0..5, "goodbye");
    assert_eq!(doc.text(), "goodbye world");
    doc.replace_range(7..13, "");
    assert_eq!(doc.text(), "goodbye");
    doc.undo();
    assert_eq!(doc.text(), "goodbye world");
}

#[test]
fn replace_range_backspace_simulation_groups() {
    let mut doc = Document::from_str("abc");
    doc.replace_range(2..3, "");
    doc.replace_range(1..2, "");
    assert_eq!(doc.text(), "a");
    doc.undo();
    assert_eq!(doc.text(), "abc"); // contiguous deletes undo together
}

#[test]
fn replace_range_clamps_out_of_bounds() {
    let mut doc = Document::from_str("ab");
    doc.replace_range(1..99, "x");
    assert_eq!(doc.text(), "ax");
    doc.replace_range(50..60, "!");
    assert_eq!(doc.text(), "ax!");
}

// --- find ---

#[test]
fn find_matches_ascii_case_insensitively() {
    let doc = Document::from_str("Hello hello HELLO");
    assert_eq!(doc.find("hello"), vec![0..5, 6..11, 12..17]);
}

#[test]
fn find_returns_char_ranges_with_multibyte_text() {
    let doc = Document::from_str("café au lait, café crème");
    let matches = doc.find("café");
    assert_eq!(matches, vec![0..4, 14..18]);
}

#[test]
fn find_empty_query_and_no_match() {
    let doc = Document::from_str("text");
    assert!(doc.find("").is_empty());
    assert!(doc.find("missing").is_empty());
}

#[test]
fn find_is_non_overlapping() {
    let doc = Document::from_str("aaaa");
    assert_eq!(doc.find("aa"), vec![0..2, 2..4]);
}

// --- word count ---

#[test]
fn word_count_counts_unicode_words() {
    let doc = Document::from_str("Hello, world — it’s fine.");
    assert_eq!(doc.word_count(), 4);
    assert_eq!(Document::from_str("").word_count(), 0);
    assert_eq!(Document::from_str("one\ntwo\n\nthree").word_count(), 3);
}

// --- file binding ---

#[test]
fn open_save_roundtrip() {
    let dir = std::env::temp_dir().join("polaris-core-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("roundtrip.md");
    std::fs::write(&path, "# Draft\n\ncafé — “quoted”\n").unwrap();

    let mut doc = Document::open(&path).unwrap();
    assert!(!doc.is_dirty());
    doc.move_line_end(false);
    doc.insert_str(" One");
    assert!(doc.is_dirty());
    doc.save().unwrap();
    assert!(!doc.is_dirty());

    let reread = std::fs::read_to_string(&path).unwrap();
    assert_eq!(reread, "# Draft One\n\ncafé — “quoted”\n");
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn rename_moves_the_file_and_flushes_edits() {
    let dir = std::env::temp_dir().join("polaris-core-test");
    std::fs::create_dir_all(&dir).unwrap();
    let old = dir.join("old-name.md");
    let new = dir.join("new-name.md");
    let _ = std::fs::remove_file(&new);
    std::fs::write(&old, "content").unwrap();

    let mut doc = Document::open(&old).unwrap();
    doc.move_line_end(false);
    doc.insert_str(" plus edits");
    doc.rename(&new).unwrap();

    assert_eq!(doc.path().unwrap(), new.as_path());
    assert!(!old.exists(), "old file is gone");
    assert_eq!(
        std::fs::read_to_string(&new).unwrap(),
        "content plus edits",
        "pending edits flushed through the rename"
    );
    assert!(!doc.is_dirty());
    std::fs::remove_file(&new).unwrap();
}

#[test]
fn rename_refuses_to_overwrite() {
    let dir = std::env::temp_dir().join("polaris-core-test");
    std::fs::create_dir_all(&dir).unwrap();
    let a = dir.join("refuse-a.md");
    let b = dir.join("refuse-b.md");
    std::fs::write(&a, "aaa").unwrap();
    std::fs::write(&b, "precious").unwrap();

    let mut doc = Document::open(&a).unwrap();
    assert!(doc.rename(&b).is_err());
    assert_eq!(doc.path().unwrap(), a.as_path(), "path unchanged on error");
    assert_eq!(std::fs::read_to_string(&b).unwrap(), "precious");
    std::fs::remove_file(&a).unwrap();
    std::fs::remove_file(&b).unwrap();
}

#[test]
fn rename_untitled_binds_and_saves() {
    let dir = std::env::temp_dir().join("polaris-core-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("was-untitled.md");
    let _ = std::fs::remove_file(&path);

    let mut doc = Document::from_str("draft");
    doc.rename(&path).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "draft");
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn rename_to_same_path_is_a_noop() {
    let dir = std::env::temp_dir().join("polaris-core-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("same.md");
    std::fs::write(&path, "x").unwrap();
    let mut doc = Document::open(&path).unwrap();
    doc.rename(&path).unwrap();
    assert_eq!(doc.path().unwrap(), path.as_path());
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn save_without_path_errors() {
    let mut doc = Document::from_str("text");
    assert!(doc.save().is_err());
}

// --- autosave policy ---

#[test]
fn autosave_debounce() {
    let mut timer = AutosaveTimer::default();
    assert!(!timer.should_save(0));
    timer.note_edit(1_000);
    assert!(!timer.should_save(1_500)); // still inside the window
    timer.note_edit(1_600); // keystroke resets the debounce
    assert!(!timer.should_save(2_400));
    assert!(timer.should_save(2_600));
    timer.saved();
    assert!(!timer.should_save(10_000));
}
