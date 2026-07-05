//! A document: buffer + cursor + history + file binding + autosave policy.
//! This is the API front-ends drive; it owns no UI and no clock.

use std::fs;
use std::io;
use std::ops::Range;
use std::path::{Path, PathBuf};

use unicode_segmentation::UnicodeSegmentation;

use crate::buffer::Buffer;
use crate::cursor::{self, Cursor};
use crate::history::{EditOp, History};

#[derive(Debug, Clone, Default)]
pub struct Document {
    buffer: Buffer,
    cursor: Cursor,
    history: History,
    path: Option<PathBuf>,
    dirty: bool,
}

impl Document {
    pub fn new() -> Self {
        Self::default()
    }

    // Infallible constructor, like `Rope::from_str`; not the `FromStr` trait.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(text: &str) -> Self {
        Self {
            buffer: Buffer::from_str(text),
            ..Self::default()
        }
    }

    pub fn open(path: impl Into<PathBuf>) -> io::Result<Self> {
        let path = path.into();
        let text = fs::read_to_string(&path)?;
        Ok(Self {
            buffer: Buffer::from_str(&text),
            path: Some(path),
            ..Self::default()
        })
    }

    // --- accessors ---

    pub fn text(&self) -> String {
        self.buffer.to_string()
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    pub fn selection(&self) -> Option<Range<usize>> {
        self.cursor.selection()
    }

    pub fn selected_text(&self) -> Option<String> {
        self.cursor.selection().map(|r| self.buffer.slice(r))
    }

    /// (line, grapheme column), both 0-based — for the status chrome.
    pub fn line_col(&self) -> (usize, usize) {
        let line = self.buffer.char_to_line(self.cursor.pos);
        (line, cursor::grapheme_col(&self.buffer, self.cursor.pos))
    }

    pub fn word_count(&self) -> usize {
        self.text().unicode_words().count()
    }

    /// All non-overlapping match ranges (char indices) of `query`,
    /// ASCII-case-insensitively. Empty query matches nothing.
    pub fn find(&self, query: &str) -> Vec<Range<usize>> {
        let needle: Vec<char> = query.chars().collect();
        if needle.is_empty() {
            return Vec::new();
        }
        let haystack: Vec<char> = self.text().chars().collect();
        let mut matches = Vec::new();
        let mut i = 0;
        while i + needle.len() <= haystack.len() {
            let hit = haystack[i..i + needle.len()]
                .iter()
                .zip(&needle)
                .all(|(a, b)| a.eq_ignore_ascii_case(b));
            if hit {
                matches.push(i..i + needle.len());
                i += needle.len();
            } else {
                i += 1;
            }
        }
        matches
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    // --- file binding ---

    pub fn save(&mut self) -> io::Result<()> {
        let path = self
            .path
            .clone()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file path set"))?;
        fs::write(&path, self.text())?;
        self.dirty = false;
        Ok(())
    }

    pub fn save_as(&mut self, path: impl Into<PathBuf>) -> io::Result<()> {
        self.path = Some(path.into());
        self.save()
    }

    // --- editing ---

    pub fn insert_char(&mut self, c: char) {
        let mut buf = [0u8; 4];
        self.insert_str(c.encode_utf8(&mut buf));
    }

    pub fn insert_str(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if let Some(sel) = self.cursor.selection() {
            // Replacing a selection is one atomic undo group.
            self.history.commit();
            self.remove_range(sel);
            self.insert_at_cursor(text, true);
        } else {
            self.insert_at_cursor(text, false);
        }
    }

    /// Apply an edit at an explicit char range, with the same undo grouping
    /// as typed input. For front-ends whose interaction layer owns the caret
    /// (e.g. the M2 `text_editor` shim) and sync edits into the model.
    pub fn replace_range(&mut self, range: Range<usize>, text: &str) {
        let len = self.buffer.len_chars();
        let start = range.start.min(len);
        let end = range.end.clamp(start, len);
        if start == end && text.is_empty() {
            return;
        }
        self.cursor.anchor = None;
        let removed = start < end;
        if removed {
            let range = start..end;
            if self.remove_starts_new_group(&range) {
                self.history.commit();
            }
            self.remove_range(range);
        } else {
            self.cursor.pos = start;
        }
        if !text.is_empty() {
            // A remove+insert from one call is one atomic replacement.
            self.insert_at_cursor(text, removed);
        }
    }

    pub fn insert_newline(&mut self) {
        self.insert_char('\n');
    }

    pub fn backspace(&mut self) {
        if let Some(sel) = self.cursor.selection() {
            self.history.commit();
            self.remove_range(sel);
            return;
        }
        if self.cursor.pos == 0 {
            return;
        }
        let start = cursor::prev_grapheme_boundary(&self.buffer, self.cursor.pos);
        let range = start..self.cursor.pos;
        if self.remove_starts_new_group(&range) {
            self.history.commit();
        }
        self.remove_range(range);
    }

    pub fn delete_forward(&mut self) {
        if let Some(sel) = self.cursor.selection() {
            self.history.commit();
            self.remove_range(sel);
            return;
        }
        if self.cursor.pos >= self.buffer.len_chars() {
            return;
        }
        let end = cursor::next_grapheme_boundary(&self.buffer, self.cursor.pos);
        let range = self.cursor.pos..end;
        if self.remove_starts_new_group(&range) {
            self.history.commit();
        }
        self.remove_range(range);
    }

    /// Close the open undo group; the next edit starts a new one. Front-ends
    /// call this on typing pauses so undo works in human-sized chunks.
    pub fn commit_undo_group(&mut self) {
        self.history.commit();
    }

    pub fn undo(&mut self) -> bool {
        match self.history.undo(&mut self.buffer) {
            Some(pos) => {
                self.cursor = Cursor {
                    pos: pos.min(self.buffer.len_chars()),
                    anchor: None,
                    sticky_col: None,
                };
                self.dirty = true;
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        match self.history.redo(&mut self.buffer) {
            Some(pos) => {
                self.cursor = Cursor {
                    pos: pos.min(self.buffer.len_chars()),
                    anchor: None,
                    sticky_col: None,
                };
                self.dirty = true;
                true
            }
            None => false,
        }
    }

    // --- movement (extend = keep/grow selection) ---

    pub fn move_left(&mut self, extend: bool) {
        self.history.commit();
        let target = match (extend, self.cursor.selection()) {
            (false, Some(sel)) => sel.start,
            _ => cursor::prev_grapheme_boundary(&self.buffer, self.cursor.pos),
        };
        self.set_cursor(target, extend);
        self.cursor.sticky_col = None;
    }

    pub fn move_right(&mut self, extend: bool) {
        self.history.commit();
        let target = match (extend, self.cursor.selection()) {
            (false, Some(sel)) => sel.end,
            _ => cursor::next_grapheme_boundary(&self.buffer, self.cursor.pos),
        };
        self.set_cursor(target, extend);
        self.cursor.sticky_col = None;
    }

    pub fn move_up(&mut self, extend: bool) {
        self.history.commit();
        let line = self.buffer.char_to_line(self.cursor.pos);
        let col = self
            .cursor
            .sticky_col
            .unwrap_or_else(|| cursor::grapheme_col(&self.buffer, self.cursor.pos));
        let target = if line == 0 {
            0
        } else {
            cursor::pos_at_grapheme_col(&self.buffer, line - 1, col)
        };
        self.set_cursor(target, extend);
        self.cursor.sticky_col = Some(col);
    }

    pub fn move_down(&mut self, extend: bool) {
        self.history.commit();
        let line = self.buffer.char_to_line(self.cursor.pos);
        let col = self
            .cursor
            .sticky_col
            .unwrap_or_else(|| cursor::grapheme_col(&self.buffer, self.cursor.pos));
        let target = if line + 1 >= self.buffer.len_lines() {
            self.buffer.len_chars()
        } else {
            cursor::pos_at_grapheme_col(&self.buffer, line + 1, col)
        };
        self.set_cursor(target, extend);
        self.cursor.sticky_col = Some(col);
    }

    pub fn move_word_left(&mut self, extend: bool) {
        self.history.commit();
        let target = cursor::prev_word_boundary(&self.buffer, self.cursor.pos);
        self.set_cursor(target, extend);
        self.cursor.sticky_col = None;
    }

    pub fn move_word_right(&mut self, extend: bool) {
        self.history.commit();
        let target = cursor::next_word_boundary(&self.buffer, self.cursor.pos);
        self.set_cursor(target, extend);
        self.cursor.sticky_col = None;
    }

    pub fn move_line_start(&mut self, extend: bool) {
        self.history.commit();
        let line = self.buffer.char_to_line(self.cursor.pos);
        self.set_cursor(self.buffer.line_to_char(line), extend);
        self.cursor.sticky_col = None;
    }

    pub fn move_line_end(&mut self, extend: bool) {
        self.history.commit();
        let line = self.buffer.char_to_line(self.cursor.pos);
        let target = self.buffer.line_to_char(line) + self.buffer.line_content_len(line);
        self.set_cursor(target, extend);
        self.cursor.sticky_col = None;
    }

    pub fn select_all(&mut self) {
        self.history.commit();
        self.cursor.anchor = Some(0);
        self.cursor.pos = self.buffer.len_chars();
        self.cursor.sticky_col = None;
    }

    pub fn set_cursor_pos(&mut self, pos: usize, extend: bool) {
        self.history.commit();
        self.set_cursor(pos.min(self.buffer.len_chars()), extend);
        self.cursor.sticky_col = None;
    }

    // --- internals ---

    fn set_cursor(&mut self, pos: usize, extend: bool) {
        if extend {
            if self.cursor.anchor.is_none() {
                self.cursor.anchor = Some(self.cursor.pos);
            }
        } else {
            self.cursor.anchor = None;
        }
        self.cursor.pos = pos;
    }

    fn set_cursor_after_edit(&mut self, pos: usize) {
        self.cursor = Cursor {
            pos,
            anchor: None,
            sticky_col: None,
        };
        self.dirty = true;
    }

    fn insert_at_cursor(&mut self, text: &str, join_open_group: bool) {
        if !join_open_group && self.insert_starts_new_group(text) {
            self.history.commit();
        }
        let cursor_before = self.cursor.pos;
        let at = self.cursor.pos;
        self.buffer.insert(at, text);
        let new_pos = at + text.chars().count();
        self.history.record(
            EditOp::Insert {
                at,
                text: text.to_string(),
            },
            cursor_before,
            new_pos,
        );
        self.set_cursor_after_edit(new_pos);
    }

    fn remove_range(&mut self, range: Range<usize>) {
        let cursor_before = self.cursor.pos;
        let start = range.start;
        let removed = self.buffer.remove(range);
        self.history.record(
            EditOp::Remove {
                at: start,
                text: removed,
            },
            cursor_before,
            start,
        );
        self.set_cursor_after_edit(start);
    }

    /// Word-sized grouping: a group is "word + trailing whitespace", so break
    /// when a non-whitespace char follows whitespace, on newlines, when the
    /// edit isn't contiguous with the open group, or when switching from
    /// deleting to inserting.
    fn insert_starts_new_group(&self, text: &str) -> bool {
        let Some(last) = self.history.open_last_op() else {
            return false;
        };
        match last {
            EditOp::Insert { at, text: prev } => {
                let contiguous = at + prev.chars().count() == self.cursor.pos;
                let word_break = text.chars().next().is_some_and(|c| !c.is_whitespace())
                    && prev.chars().last().is_some_and(|c| c.is_whitespace());
                let newline = text.contains('\n');
                !contiguous || word_break || newline
            }
            EditOp::Remove { .. } => true,
        }
    }

    fn remove_starts_new_group(&self, range: &Range<usize>) -> bool {
        let Some(last) = self.history.open_last_op() else {
            return false;
        };
        match last {
            // Contiguous with backspace runs (at == range.end) or
            // delete-forward runs (at == range.start).
            EditOp::Remove { at, .. } => *at != range.end && *at != range.start,
            EditOp::Insert { .. } => true,
        }
    }
}

/// Debounced-autosave decision logic. `polaris-core` owns no clock; callers
/// pass monotonic milliseconds.
#[derive(Debug, Clone, Copy, Default)]
pub struct AutosaveTimer {
    last_edit_ms: Option<u64>,
}

impl AutosaveTimer {
    /// DESIGN.md: autosave fires ~1s after the last keystroke.
    pub const DEBOUNCE_MS: u64 = 1000;

    pub fn note_edit(&mut self, now_ms: u64) {
        self.last_edit_ms = Some(now_ms);
    }

    /// True once the debounce window has elapsed since the last edit.
    pub fn should_save(&self, now_ms: u64) -> bool {
        self.last_edit_ms
            .is_some_and(|t| now_ms.saturating_sub(t) >= Self::DEBOUNCE_MS)
    }

    pub fn saved(&mut self) {
        self.last_edit_ms = None;
    }
}
