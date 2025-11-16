use std::fs;
use std::path::PathBuf;
use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct TextBuffer {
    pub lines: Vec<String>,
    pub cursor_x: usize,
    pub cursor_y: usize,
    pub file_path: Option<PathBuf>,
    pub dirty: bool,
    pub scroll_offset: usize,
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
        if self.cursor_x > line.len() {
            self.cursor_x = line.len();
        }

        line.insert(self.cursor_x, c);
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

        let cursor_x = self.cursor_x.min(self.lines[self.cursor_y].len());
        let after = self.lines[self.cursor_y][cursor_x..].to_string();
        self.lines[self.cursor_y].truncate(cursor_x);
        self.lines.insert(self.cursor_y + 1, after);

        self.cursor_y += 1;
        self.cursor_x = 0;
        self.dirty = true;
    }

    pub fn backspace(&mut self) {
        if self.cursor_x > 0 {
            let line = &mut self.lines[self.cursor_y];
            if self.cursor_x <= line.len() {
                line.remove(self.cursor_x - 1);
                self.cursor_x -= 1;
                self.dirty = true;
            }
        } else if self.cursor_y > 0 {
            // Merge with previous line
            let current_line = self.lines.remove(self.cursor_y);
            self.cursor_y -= 1;
            self.cursor_x = self.lines[self.cursor_y].len();
            self.lines[self.cursor_y].push_str(&current_line);
            self.dirty = true;
        }
    }

    pub fn delete(&mut self) {
        if self.cursor_y >= self.lines.len() {
            return;
        }

        let line = &mut self.lines[self.cursor_y];
        if self.cursor_x < line.len() {
            line.remove(self.cursor_x);
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
            self.cursor_x = self.lines[self.cursor_y].len();
        }
    }

    pub fn move_cursor_right(&mut self) {
        if self.cursor_y >= self.lines.len() {
            return;
        }

        let line_len = self.lines[self.cursor_y].len();
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
            let line_len = self.lines[self.cursor_y].len();
            self.cursor_x = self.cursor_x.min(line_len);
        }
    }

    pub fn move_cursor_down(&mut self) {
        if self.cursor_y < self.lines.len() - 1 {
            self.cursor_y += 1;
            let line_len = self.lines[self.cursor_y].len();
            self.cursor_x = self.cursor_x.min(line_len);
        }
    }

    pub fn move_to_line_start(&mut self) {
        self.cursor_x = 0;
    }

    pub fn move_to_line_end(&mut self) {
        if self.cursor_y < self.lines.len() {
            self.cursor_x = self.lines[self.cursor_y].len();
        }
    }

    pub fn save(&mut self) -> Result<()> {
        let path = self.file_path.as_ref()
            .with_context(|| "No file path set")?;

        let contents = self.lines.join("\n");
        fs::write(path, contents)
            .with_context(|| format!("Failed to write file: {:?}", path))?;

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
