use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct TextBuffer {
    pub lines: Vec<String>,
    /// Cursor column as a character index (not a byte index).
    pub cursor_x: usize,
    pub cursor_y: usize,
    pub file_path: Option<PathBuf>,
    pub dirty: bool,
    pub scroll_offset: usize,
}

fn char_count(line: &str) -> usize {
    line.chars().count()
}

fn byte_index(line: &str, char_idx: usize) -> usize {
    line.char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or(line.len())
}

impl TextBuffer {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor_x: 0,
            cursor_y: 0,
            file_path: None,
            dirty: false,
            scroll_offset: 0,
        }
    }

    pub fn from_file(path: PathBuf) -> Result<Self> {
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read file: {:?}", path))?;

        let lines: Vec<String> = if contents.is_empty() {
            vec![String::new()]
        } else {
            contents.lines().map(String::from).collect()
        };

        Ok(Self {
            lines,
            cursor_x: 0,
            cursor_y: 0,
            file_path: Some(path),
            dirty: false,
            scroll_offset: 0,
        })
    }

    pub fn insert_char(&mut self, c: char) {
        if self.cursor_y >= self.lines.len() {
            self.lines.push(String::new());
        }

        let line = &mut self.lines[self.cursor_y];
        let count = char_count(line);
        if self.cursor_x > count {
            self.cursor_x = count;
        }

        let idx = byte_index(line, self.cursor_x);
        line.insert(idx, c);
        self.cursor_x += 1;
        self.dirty = true;
    }

    pub fn insert_newline(&mut self) {
        if self.cursor_y >= self.lines.len() {
            self.lines.push(String::new());
            self.cursor_y = self.lines.len() - 1;
            self.cursor_x = 0;
            self.dirty = true;
            return;
        }

        let cursor_x = self.cursor_x.min(char_count(&self.lines[self.cursor_y]));
        let idx = byte_index(&self.lines[self.cursor_y], cursor_x);
        let after = self.lines[self.cursor_y][idx..].to_string();
        self.lines[self.cursor_y].truncate(idx);
        self.lines.insert(self.cursor_y + 1, after);

        self.cursor_y += 1;
        self.cursor_x = 0;
        self.dirty = true;
    }

    pub fn backspace(&mut self) {
        if self.cursor_x > 0 {
            let line = &mut self.lines[self.cursor_y];
            if self.cursor_x <= char_count(line) {
                let idx = byte_index(line, self.cursor_x - 1);
                line.remove(idx);
                self.cursor_x -= 1;
                self.dirty = true;
            }
        } else if self.cursor_y > 0 {
            // Merge with previous line
            let current_line = self.lines.remove(self.cursor_y);
            self.cursor_y -= 1;
            self.cursor_x = char_count(&self.lines[self.cursor_y]);
            self.lines[self.cursor_y].push_str(&current_line);
            self.dirty = true;
        }
    }

    pub fn delete(&mut self) {
        if self.cursor_y >= self.lines.len() {
            return;
        }

        let line = &mut self.lines[self.cursor_y];
        if self.cursor_x < char_count(line) {
            let idx = byte_index(line, self.cursor_x);
            line.remove(idx);
            self.dirty = true;
        } else if self.cursor_y < self.lines.len() - 1 {
            // Merge with next line
            let next_line = self.lines.remove(self.cursor_y + 1);
            self.lines[self.cursor_y].push_str(&next_line);
            self.dirty = true;
        }
    }

    pub fn move_cursor_left(&mut self) {
        if self.cursor_x > 0 {
            self.cursor_x -= 1;
        } else if self.cursor_y > 0 {
            self.cursor_y -= 1;
            self.cursor_x = char_count(&self.lines[self.cursor_y]);
        }
    }

    pub fn move_cursor_right(&mut self) {
        if self.cursor_y >= self.lines.len() {
            return;
        }

        let line_len = char_count(&self.lines[self.cursor_y]);
        if self.cursor_x < line_len {
            self.cursor_x += 1;
        } else if self.cursor_y < self.lines.len() - 1 {
            self.cursor_y += 1;
            self.cursor_x = 0;
        }
    }

    pub fn move_cursor_up(&mut self) {
        if self.cursor_y > 0 {
            self.cursor_y -= 1;
            let line_len = char_count(&self.lines[self.cursor_y]);
            self.cursor_x = self.cursor_x.min(line_len);
        }
    }

    pub fn move_cursor_down(&mut self) {
        if self.cursor_y < self.lines.len() - 1 {
            self.cursor_y += 1;
            let line_len = char_count(&self.lines[self.cursor_y]);
            self.cursor_x = self.cursor_x.min(line_len);
        }
    }

    pub fn move_to_line_start(&mut self) {
        self.cursor_x = 0;
    }

    pub fn move_to_line_end(&mut self) {
        if self.cursor_y < self.lines.len() {
            self.cursor_x = char_count(&self.lines[self.cursor_y]);
        }
    }

    pub fn save(&mut self) -> Result<()> {
        let path = self
            .file_path
            .as_ref()
            .with_context(|| "No file path set")?;

        let contents = self.lines.join("\n");
        fs::write(path, contents).with_context(|| format!("Failed to write file: {:?}", path))?;

        self.dirty = false;
        Ok(())
    }

    pub fn save_as(&mut self, path: PathBuf) -> Result<()> {
        self.file_path = Some(path);
        self.save()
    }

    pub fn get_content(&self) -> String {
        self.lines.join("\n")
    }

    pub fn filename(&self) -> String {
        self.file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("[No Name]")
            .to_string()
    }
}

impl Default for TextBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn type_str(buffer: &mut TextBuffer, s: &str) {
        for c in s.chars() {
            buffer.insert_char(c);
        }
    }

    #[test]
    fn insert_ascii() {
        let mut b = TextBuffer::new();
        type_str(&mut b, "hello");
        assert_eq!(b.lines, vec!["hello"]);
        assert_eq!(b.cursor_x, 5);
        assert!(b.dirty);
    }

    #[test]
    fn insert_after_multibyte_char() {
        let mut b = TextBuffer::new();
        // Inserting after a multi-byte char used to panic on a non-boundary byte index
        type_str(&mut b, "éx");
        assert_eq!(b.lines, vec!["éx"]);
        assert_eq!(b.cursor_x, 2);
    }

    #[test]
    fn insert_smart_typography_chars() {
        let mut b = TextBuffer::new();
        type_str(&mut b, "“hello” — it’s fine…");
        assert_eq!(b.lines, vec!["“hello” — it’s fine…"]);
    }

    #[test]
    fn insert_mid_line_with_multibyte_prefix() {
        let mut b = TextBuffer::new();
        type_str(&mut b, "café");
        b.cursor_x = 3;
        b.insert_char('f');
        assert_eq!(b.lines, vec!["caffé"]);
        assert_eq!(b.cursor_x, 4);
    }

    #[test]
    fn backspace_multibyte() {
        let mut b = TextBuffer::new();
        type_str(&mut b, "café");
        b.backspace();
        assert_eq!(b.lines, vec!["caf"]);
        assert_eq!(b.cursor_x, 3);
    }

    #[test]
    fn backspace_before_multibyte() {
        let mut b = TextBuffer::new();
        type_str(&mut b, "aéb");
        b.cursor_x = 2; // between é and b
        b.backspace();
        assert_eq!(b.lines, vec!["ab"]);
        assert_eq!(b.cursor_x, 1);
    }

    #[test]
    fn backspace_at_line_start_merges_lines() {
        let mut b = TextBuffer::new();
        type_str(&mut b, "café");
        b.insert_newline();
        type_str(&mut b, "au lait");
        b.cursor_x = 0;
        b.backspace();
        assert_eq!(b.lines, vec!["caféau lait"]);
        assert_eq!(b.cursor_y, 0);
        assert_eq!(b.cursor_x, 4); // after é, in chars
    }

    #[test]
    fn delete_multibyte() {
        let mut b = TextBuffer::new();
        type_str(&mut b, "aéb");
        b.cursor_x = 1;
        b.delete();
        assert_eq!(b.lines, vec!["ab"]);
    }

    #[test]
    fn delete_at_line_end_merges_lines() {
        let mut b = TextBuffer::new();
        type_str(&mut b, "one");
        b.insert_newline();
        type_str(&mut b, "two");
        b.cursor_y = 0;
        b.cursor_x = 3;
        b.delete();
        assert_eq!(b.lines, vec!["onetwo"]);
    }

    #[test]
    fn newline_splits_at_multibyte_boundary() {
        let mut b = TextBuffer::new();
        type_str(&mut b, "aé b");
        b.cursor_x = 2; // after é
        b.insert_newline();
        assert_eq!(b.lines, vec!["aé", " b"]);
        assert_eq!(b.cursor_y, 1);
        assert_eq!(b.cursor_x, 0);
    }

    #[test]
    fn cursor_moves_over_multibyte_line() {
        let mut b = TextBuffer::new();
        type_str(&mut b, "é");
        b.insert_newline();
        type_str(&mut b, "long line");
        b.move_cursor_up();
        assert_eq!(b.cursor_x, 1); // clamped to char count, not byte count
        b.move_to_line_end();
        assert_eq!(b.cursor_x, 1);
        b.move_cursor_right();
        assert_eq!((b.cursor_y, b.cursor_x), (1, 0));
        b.move_cursor_left();
        assert_eq!((b.cursor_y, b.cursor_x), (0, 1));
    }

    #[test]
    fn get_content_joins_lines() {
        let mut b = TextBuffer::new();
        type_str(&mut b, "one");
        b.insert_newline();
        type_str(&mut b, "two");
        assert_eq!(b.get_content(), "one\ntwo");
    }
}
