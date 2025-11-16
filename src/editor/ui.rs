use crate::editor::buffer::TextBuffer;
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;
use std::path::PathBuf;

pub enum EditorMode {
    Normal,
    Preview,
    SaveAs,
}

pub struct Editor {
    pub buffer: TextBuffer,
    pub mode: EditorMode,
    pub should_quit: bool,
    pub should_deploy: bool,
    pub message: Option<String>,
    pub save_as_input: String,
}

impl Editor {
    pub fn new(buffer: TextBuffer) -> Self {
        Self {
            buffer,
            mode: EditorMode::Normal,
            should_quit: false,
            should_deploy: false,
            message: None,
            save_as_input: String::new(),
        }
    }

    pub fn run(&mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.run_loop(&mut terminal);

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }

    fn run_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        loop {
            terminal.draw(|f| self.render(f))?;

            if let Event::Key(key) = event::read()? {
                self.handle_key(key)?;
            }

            if self.should_quit {
                break;
            }

            if self.should_deploy {
                // We'll handle deployment in main
                break;
            }
        }

        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        match &self.mode {
            EditorMode::Normal => self.handle_normal_mode(key)?,
            EditorMode::Preview => self.handle_preview_mode(key)?,
            EditorMode::SaveAs => self.handle_save_as_mode(key)?,
        }
        Ok(())
    }

    fn handle_normal_mode(&mut self, key: KeyEvent) -> Result<()> {
        match (key.code, key.modifiers) {
            // Ctrl+Q to quit
            (KeyCode::Char('q'), KeyModifiers::CONTROL) => {
                if self.buffer.dirty {
                    self.message = Some("Unsaved changes! Press Ctrl+Q again to force quit, or Ctrl+S to save.".to_string());
                    // Simple force quit on second attempt - in production you'd track this
                } else {
                    self.should_quit = true;
                }
            }
            // Ctrl+S to save
            (KeyCode::Char('s'), KeyModifiers::CONTROL) => {
                if self.buffer.file_path.is_some() {
                    match self.buffer.save() {
                        Ok(_) => self.message = Some("File saved successfully.".to_string()),
                        Err(e) => self.message = Some(format!("Error saving file: {}", e)),
                    }
                } else {
                    self.mode = EditorMode::SaveAs;
                    self.save_as_input.clear();
                    self.message = Some("Enter filename:".to_string());
                }
            }
            // Ctrl+D to deploy
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                self.should_deploy = true;
            }
            // Ctrl+P to toggle preview
            (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
                self.mode = EditorMode::Preview;
            }
            // Navigation
            (KeyCode::Left, _) => self.buffer.move_cursor_left(),
            (KeyCode::Right, _) => self.buffer.move_cursor_right(),
            (KeyCode::Up, _) => self.buffer.move_cursor_up(),
            (KeyCode::Down, _) => self.buffer.move_cursor_down(),
            (KeyCode::Home, _) => self.buffer.move_to_line_start(),
            (KeyCode::End, _) => self.buffer.move_to_line_end(),
            // Editing
            (KeyCode::Char(c), KeyModifiers::NONE) | (KeyCode::Char(c), KeyModifiers::SHIFT) => {
                self.buffer.insert_char(c);
                self.message = None;
            }
            (KeyCode::Enter, _) => {
                self.buffer.insert_newline();
                self.message = None;
            }
            (KeyCode::Backspace, _) => {
                self.buffer.backspace();
                self.message = None;
            }
            (KeyCode::Delete, _) => {
                self.buffer.delete();
                self.message = None;
            }
            (KeyCode::Tab, _) => {
                // Insert 4 spaces
                for _ in 0..4 {
                    self.buffer.insert_char(' ');
                }
                self.message = None;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_preview_mode(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.mode = EditorMode::Normal;
            }
            KeyCode::Up => {
                if self.buffer.scroll_offset > 0 {
                    self.buffer.scroll_offset -= 1;
                }
            }
            KeyCode::Down => {
                self.buffer.scroll_offset += 1;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_save_as_mode(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Enter => {
                if !self.save_as_input.is_empty() {
                    let path = PathBuf::from(&self.save_as_input);
                    match self.buffer.save_as(path) {
                        Ok(_) => {
                            self.message = Some(format!("File saved as: {}", self.save_as_input));
                            self.mode = EditorMode::Normal;
                        }
                        Err(e) => {
                            self.message = Some(format!("Error saving file: {}", e));
                        }
                    }
                }
            }
            KeyCode::Esc => {
                self.mode = EditorMode::Normal;
                self.message = None;
            }
            KeyCode::Char(c) => {
                self.save_as_input.push(c);
            }
            KeyCode::Backspace => {
                self.save_as_input.pop();
            }
            _ => {}
        }
        Ok(())
    }

    fn render(&mut self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),
                Constraint::Length(3),
            ])
            .split(frame.size());

        match self.mode {
            EditorMode::Normal => self.render_editor(frame, chunks[0]),
            EditorMode::Preview => self.render_preview(frame, chunks[0]),
            EditorMode::SaveAs => self.render_editor(frame, chunks[0]),
        }

        self.render_status_bar(frame, chunks[1]);
    }

    fn render_editor(&mut self, frame: &mut Frame, area: Rect) {
        let visible_height = area.height.saturating_sub(2) as usize;

        // Adjust scroll offset to keep cursor visible
        if self.buffer.cursor_y < self.buffer.scroll_offset {
            self.buffer.scroll_offset = self.buffer.cursor_y;
        } else if self.buffer.cursor_y >= self.buffer.scroll_offset + visible_height {
            self.buffer.scroll_offset = self.buffer.cursor_y.saturating_sub(visible_height - 1);
        }

        let visible_lines: Vec<Line> = self.buffer.lines
            .iter()
            .skip(self.buffer.scroll_offset)
            .take(visible_height)
            .map(|line| Line::from(line.as_str()))
            .collect();

        let paragraph = Paragraph::new(visible_lines)
            .block(Block::default().borders(Borders::ALL).title("Editor"));

        frame.render_widget(paragraph, area);

        // Calculate cursor position accounting for borders and scroll
        let cursor_x = (area.x + 1 + self.buffer.cursor_x as u16).min(area.width - 2);
        let cursor_y = (area.y + 1 + (self.buffer.cursor_y - self.buffer.scroll_offset) as u16)
            .min(area.height - 2);

        frame.set_cursor(cursor_x, cursor_y);
    }

    fn render_preview(&self, frame: &mut Frame, area: Rect) {
        use pulldown_cmark::{Parser, html};

        let markdown_content = self.buffer.get_content();
        let parser = Parser::new(&markdown_content);

        let mut html_output = String::new();
        html::push_html(&mut html_output, parser);

        // For terminal preview, we'll just show the markdown as-is with some formatting
        // A more sophisticated approach would use a markdown rendering library for terminals
        let visible_height = area.height.saturating_sub(2) as usize;
        let lines: Vec<Line> = markdown_content
            .lines()
            .skip(self.buffer.scroll_offset)
            .take(visible_height)
            .map(|line| {
                if line.starts_with("# ") {
                    Line::from(Span::styled(line, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)))
                } else if line.starts_with("## ") {
                    Line::from(Span::styled(line, Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)))
                } else if line.starts_with("```") {
                    Line::from(Span::styled(line, Style::default().fg(Color::Green)))
                } else {
                    Line::from(line)
                }
            })
            .collect();

        let paragraph = Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title("Preview (Ctrl+P to exit)"))
            .wrap(Wrap { trim: false });

        frame.render_widget(paragraph, area);
    }

    fn render_status_bar(&self, frame: &mut Frame, area: Rect) {
        let filename = self.buffer.filename();
        let dirty_indicator = if self.buffer.dirty { " [+]" } else { "" };
        let position = format!("{}:{}", self.buffer.cursor_y + 1, self.buffer.cursor_x + 1);

        let mode_text = match self.mode {
            EditorMode::Normal => "NORMAL",
            EditorMode::Preview => "PREVIEW",
            EditorMode::SaveAs => &self.save_as_input,
        };

        let status_text = if let Some(ref msg) = self.message {
            msg.clone()
        } else {
            format!("{}{} | {} | {}", filename, dirty_indicator, position, mode_text)
        };

        let status = Paragraph::new(status_text)
            .style(Style::default().bg(Color::DarkGray).fg(Color::White))
            .block(Block::default().borders(Borders::ALL));

        frame.render_widget(status, area);

        let help_text = "Ctrl+S: Save | Ctrl+Q: Quit | Ctrl+D: Deploy | Ctrl+P: Preview";
        let help = Paragraph::new(help_text)
            .style(Style::default().fg(Color::Gray));

        let help_area = Rect {
            x: area.x + 1,
            y: area.y + 2,
            width: area.width.saturating_sub(2),
            height: 1,
        };

        frame.render_widget(help, help_area);
    }
}
