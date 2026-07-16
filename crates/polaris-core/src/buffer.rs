//! Rope-backed text buffer. All positions are char indices; grapheme
//! awareness lives in [`crate::cursor`].

use ropey::Rope;
use std::fmt;
use std::ops::Range;

#[derive(Debug, Clone, Default)]
pub struct Buffer {
    rope: Rope,
}

impl Buffer {
    pub fn new() -> Self {
        Self::default()
    }

    // Infallible constructor, like `Rope::from_str`; not the `FromStr` trait.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
        }
    }

    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn is_empty(&self) -> bool {
        self.rope.len_chars() == 0
    }

    pub fn char_to_line(&self, char_idx: usize) -> usize {
        self.rope.char_to_line(char_idx)
    }

    pub fn line_to_char(&self, line_idx: usize) -> usize {
        self.rope.line_to_char(line_idx)
    }

    /// Char index for a byte offset (clamped). Bridges byte-based tools
    /// (e.g. the markdown parser) back to char-based cursor positions.
    pub fn byte_to_char(&self, byte_idx: usize) -> usize {
        self.rope.byte_to_char(byte_idx.min(self.rope.len_bytes()))
    }

    /// Byte offset for a char index (clamped). The inverse of
    /// [`byte_to_char`](Self::byte_to_char).
    pub fn char_to_byte(&self, char_idx: usize) -> usize {
        self.rope.char_to_byte(char_idx.min(self.rope.len_chars()))
    }

    /// Line text, including any trailing newline.
    pub fn line(&self, line_idx: usize) -> String {
        self.rope.line(line_idx).to_string()
    }

    /// Chars in the line, excluding any trailing newline.
    pub fn line_content_len(&self, line_idx: usize) -> usize {
        let line = self.rope.line(line_idx);
        let len = line.len_chars();
        if len > 0 && line.char(len - 1) == '\n' {
            len - 1
        } else {
            len
        }
    }

    pub fn slice(&self, range: Range<usize>) -> String {
        self.rope.slice(range).to_string()
    }

    pub fn insert(&mut self, char_idx: usize, text: &str) {
        self.rope.insert(char_idx, text);
    }

    /// Remove a char range, returning the removed text (for undo).
    pub fn remove(&mut self, range: Range<usize>) -> String {
        let removed = self.rope.slice(range.clone()).to_string();
        self.rope.remove(range);
        removed
    }
}

impl fmt::Display for Buffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.rope)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_remove_roundtrip() {
        let mut b = Buffer::from_str("hello world");
        b.insert(5, ",");
        assert_eq!(b.to_string(), "hello, world");
        let removed = b.remove(5..6);
        assert_eq!(removed, ",");
        assert_eq!(b.to_string(), "hello world");
    }

    #[test]
    fn char_indexing_with_multibyte() {
        let mut b = Buffer::from_str("café");
        b.insert(4, "!");
        assert_eq!(b.to_string(), "café!");
        assert_eq!(b.len_chars(), 5);
    }

    #[test]
    fn byte_char_conversions_roundtrip_through_multibyte() {
        let b = Buffer::from_str("aé→b"); // a=1B, é=2B, →=3B, b=1B
                                          // Char starts land on bytes 0,1,3,6.
        assert_eq!(b.char_to_byte(0), 0);
        assert_eq!(b.char_to_byte(1), 1);
        assert_eq!(b.char_to_byte(2), 3);
        assert_eq!(b.char_to_byte(3), 6);
        for c in 0..=b.len_chars() {
            assert_eq!(b.byte_to_char(b.char_to_byte(c)), c);
        }
        // Both clamp instead of panicking past the end.
        assert_eq!(b.char_to_byte(999), b.char_to_byte(b.len_chars()));
        assert_eq!(b.byte_to_char(999), b.len_chars());
    }

    #[test]
    fn line_content_len_excludes_newline() {
        let b = Buffer::from_str("ab\ncdé\n");
        assert_eq!(b.line_content_len(0), 2);
        assert_eq!(b.line_content_len(1), 3);
        // trailing newline yields a final empty line
        assert_eq!(b.len_lines(), 3);
        assert_eq!(b.line_content_len(2), 0);
    }
}
