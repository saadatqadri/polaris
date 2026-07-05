//! Grapheme-aware cursor, selection, and movement primitives.
//!
//! Positions are char indices into the buffer, but movement always lands on
//! grapheme cluster boundaries, so combining accents, emoji (including ZWJ
//! sequences), and CJK behave as single visual units.

use crate::buffer::Buffer;
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cursor {
    /// Position as a char index into the buffer.
    pub pos: usize,
    /// Selection anchor (char index). `None` means no selection.
    pub anchor: Option<usize>,
    /// Preferred grapheme column for vertical movement (sticky column).
    pub sticky_col: Option<usize>,
}

impl Cursor {
    /// The selected char range, if a non-empty selection exists.
    pub fn selection(&self) -> Option<Range<usize>> {
        match self.anchor {
            Some(a) if a < self.pos => Some(a..self.pos),
            Some(a) if a > self.pos => Some(self.pos..a),
            _ => None,
        }
    }
}

fn char_to_byte(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}

fn byte_to_char(s: &str, byte_idx: usize) -> usize {
    s[..byte_idx].chars().count()
}

/// The grapheme boundary immediately before `pos` (0 if already at start).
pub fn prev_grapheme_boundary(buffer: &Buffer, pos: usize) -> usize {
    if pos == 0 {
        return 0;
    }
    let line_idx = buffer.char_to_line(pos);
    let line_start = buffer.line_to_char(line_idx);
    if pos == line_start {
        // Step over the previous line's newline.
        return pos - 1;
    }
    let line = buffer.line(line_idx);
    let target_byte = char_to_byte(&line, pos - line_start);
    let mut prev_byte = 0;
    for (b, _) in line.grapheme_indices(true) {
        if b >= target_byte {
            break;
        }
        prev_byte = b;
    }
    line_start + byte_to_char(&line, prev_byte)
}

/// The grapheme boundary immediately after `pos` (clamped to buffer length).
pub fn next_grapheme_boundary(buffer: &Buffer, pos: usize) -> usize {
    let len = buffer.len_chars();
    if pos >= len {
        return len;
    }
    let line_idx = buffer.char_to_line(pos);
    let line_start = buffer.line_to_char(line_idx);
    let line = buffer.line(line_idx);
    let target_byte = char_to_byte(&line, pos - line_start);
    for (b, g) in line.grapheme_indices(true) {
        if b == target_byte {
            return line_start + byte_to_char(&line, b + g.len());
        }
        if b > target_byte {
            break;
        }
    }
    // `pos` was inside a grapheme; fall back to a single char step.
    pos + 1
}

/// Start of the word before `pos` (crossing lines if needed).
pub fn prev_word_boundary(buffer: &Buffer, mut pos: usize) -> usize {
    while pos > 0 {
        let line_idx = buffer.char_to_line(pos);
        let line_start = buffer.line_to_char(line_idx);
        if pos == line_start {
            pos -= 1; // hop over the previous line's newline
            continue;
        }
        let line = buffer.line(line_idx);
        let target_byte = char_to_byte(&line, pos - line_start);
        let mut best = None;
        for (b, seg) in line.split_word_bound_indices() {
            if b >= target_byte {
                break;
            }
            if seg.chars().any(char::is_alphanumeric) {
                best = Some(b);
            }
        }
        if let Some(b) = best {
            return line_start + byte_to_char(&line, b);
        }
        pos = line_start;
    }
    0
}

/// End of the word at/after `pos` (crossing lines if needed).
pub fn next_word_boundary(buffer: &Buffer, mut pos: usize) -> usize {
    let len = buffer.len_chars();
    while pos < len {
        let line_idx = buffer.char_to_line(pos);
        let line_start = buffer.line_to_char(line_idx);
        let line = buffer.line(line_idx);
        let target_byte = char_to_byte(&line, pos - line_start);
        for (b, seg) in line.split_word_bound_indices() {
            let end = b + seg.len();
            if end > target_byte && seg.chars().any(char::is_alphanumeric) {
                return line_start + byte_to_char(&line, end);
            }
        }
        // No word end after pos on this line — continue from the next line.
        pos = line_start + line.chars().count();
    }
    len
}

/// The grapheme column of `pos` within its line.
pub fn grapheme_col(buffer: &Buffer, pos: usize) -> usize {
    let line_idx = buffer.char_to_line(pos);
    let line_start = buffer.line_to_char(line_idx);
    let line = buffer.line(line_idx);
    let target_byte = char_to_byte(&line, pos - line_start);
    line[..target_byte].graphemes(true).count()
}

/// The char position at grapheme column `col` (clamped to line content) on
/// line `line_idx`.
pub fn pos_at_grapheme_col(buffer: &Buffer, line_idx: usize, col: usize) -> usize {
    let line_start = buffer.line_to_char(line_idx);
    let line = buffer.line(line_idx);
    let content = line.strip_suffix('\n').unwrap_or(&line);
    let mut byte = content.len();
    for (i, (b, _)) in content.grapheme_indices(true).enumerate() {
        if i == col {
            byte = b;
            break;
        }
    }
    line_start + byte_to_char(content, byte)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grapheme_steps_over_combining_accent() {
        // "e" + U+0301 combining acute: one grapheme, two chars
        let b = Buffer::from_str("ae\u{301}b");
        assert_eq!(next_grapheme_boundary(&b, 1), 3);
        assert_eq!(prev_grapheme_boundary(&b, 3), 1);
    }

    #[test]
    fn grapheme_steps_over_zwj_emoji() {
        // Family emoji: multiple scalars joined by ZWJ, one grapheme
        let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
        let text = format!("a{}b", family);
        let b = Buffer::from_str(&text);
        let family_chars = family.chars().count();
        assert_eq!(next_grapheme_boundary(&b, 1), 1 + family_chars);
        assert_eq!(prev_grapheme_boundary(&b, 1 + family_chars), 1);
    }

    #[test]
    fn grapheme_crosses_lines() {
        let b = Buffer::from_str("ab\ncd");
        assert_eq!(next_grapheme_boundary(&b, 2), 3); // over the newline
        assert_eq!(prev_grapheme_boundary(&b, 3), 2); // back over it
    }

    #[test]
    fn boundaries_clamp_at_ends() {
        let b = Buffer::from_str("ab");
        assert_eq!(prev_grapheme_boundary(&b, 0), 0);
        assert_eq!(next_grapheme_boundary(&b, 2), 2);
    }

    #[test]
    fn word_boundaries_basic() {
        let b = Buffer::from_str("hello brave world");
        assert_eq!(prev_word_boundary(&b, 17), 12); // start of "world"
        assert_eq!(prev_word_boundary(&b, 12), 6); // start of "brave"
        assert_eq!(next_word_boundary(&b, 0), 5); // end of "hello"
        assert_eq!(next_word_boundary(&b, 5), 11); // end of "brave"
    }

    #[test]
    fn word_boundaries_cross_lines() {
        let b = Buffer::from_str("one\ntwo");
        assert_eq!(next_word_boundary(&b, 3), 7); // end of "two"
        assert_eq!(prev_word_boundary(&b, 4), 0); // start of "one"
    }

    #[test]
    fn word_boundary_with_punctuation() {
        let b = Buffer::from_str("it's done.");
        // "it's" is one word under UAX-29
        assert_eq!(next_word_boundary(&b, 0), 4);
        assert_eq!(prev_word_boundary(&b, 9), 5); // start of "done"
    }

    #[test]
    fn sticky_column_helpers() {
        let b = Buffer::from_str("e\u{301}xy\nab");
        // col of 'y' on line 0: é(1 grapheme, 2 chars) + x = col 2
        assert_eq!(grapheme_col(&b, 3), 2);
        assert_eq!(pos_at_grapheme_col(&b, 1, 2), 7); // clamped to end of "ab"
        assert_eq!(pos_at_grapheme_col(&b, 0, 1), 2); // after é = char 2
    }

    #[test]
    fn selection_normalizes_direction() {
        let c = Cursor {
            pos: 2,
            anchor: Some(5),
            sticky_col: None,
        };
        assert_eq!(c.selection(), Some(2..5));
        let c2 = Cursor {
            pos: 5,
            anchor: Some(5),
            sticky_col: None,
        };
        assert_eq!(c2.selection(), None);
    }
}
