//! The iced GUI shell (Phase 1, M2–M3).
//!
//! Per PLAN §7 decision #3, the view/interaction layer is iced's
//! `text_editor`; `polaris-core::Document` stays the document model. Every
//! editor action that changes text is synced into the `Document` as a
//! char-level diff via `replace_range`, which preserves core's word-sized
//! undo grouping. Undo/redo run in core and rebuild the widget content.
//! The custom cosmic-text widget replaces this shim in Phase 2.

mod fonts;
mod theme;

use std::ops::Range;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use iced::widget::text_editor;
use iced::widget::text_editor::{Binding, Content, KeyPress};
use iced::widget::{column, container, row, space, text, text_input};
use iced::{
    event, keyboard, Background, Border, Element, Fill, Padding, Subscription, Task, Theme,
};

use polaris_core::buffer::Buffer;
use polaris_core::{AutosaveTimer, Document};

const CHROME_INPUT_ID: &str = "chrome-input";

pub fn run(path: Option<PathBuf>) -> iced::Result {
    iced::application(move || App::boot(path.clone()), App::update, App::view)
        .title(App::title)
        .theme(App::theme)
        .subscription(App::subscription)
        .font(fonts::QUATTRO_REGULAR_BYTES)
        .font(fonts::QUATTRO_BOLD_BYTES)
        .font(fonts::QUATTRO_ITALIC_BYTES)
        .font(fonts::MONO_REGULAR_BYTES)
        .default_font(fonts::QUATTRO)
        .window_size(iced::Size::new(760.0, 940.0))
        .run()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Overlay {
    None,
    Find,
    SaveAs,
}

struct App {
    doc: Document,
    content: Content,
    /// The widget's text as of the last sync; diffed against on each edit.
    last_text: String,
    dark: bool,
    status: Option<String>,
    overlay: Overlay,
    input: String,
    matches: Vec<Range<usize>>,
    current_match: usize,
    epoch: Instant,
    autosave: AutosaveTimer,
}

#[derive(Debug, Clone)]
enum Message {
    Edit(text_editor::Action),
    Save,
    Undo,
    Redo,
    AutosaveTick,
    FindOpen,
    OverlayInput(String),
    OverlaySubmit { backwards: bool },
    OverlayClose,
}

impl App {
    fn boot(path: Option<PathBuf>) -> (Self, Task<Message>) {
        let doc = match &path {
            // Readability is pre-checked in the CLI before `run`.
            Some(p) if p.exists() => Document::open(p).expect("file readable"),
            Some(p) => {
                let mut doc = Document::from_str("");
                doc.save_as(p).expect("file creatable");
                doc
            }
            None => Document::new(),
        };

        let content = Content::with_text(&doc.text());
        let mut app = Self {
            doc,
            last_text: content.text(),
            content,
            dark: detect_dark(),
            status: None,
            overlay: Overlay::None,
            input: String::new(),
            matches: Vec::new(),
            current_match: 0,
            epoch: Instant::now(),
            autosave: AutosaveTimer::default(),
        };
        // The widget normalizes to a trailing newline; rebase the model once
        // so subsequent diffs line up.
        let doc_text = app.doc.text();
        if app.last_text != doc_text {
            apply_diff(&mut app.doc, &doc_text, &app.last_text.clone());
            app.doc.commit_undo_group();
        }

        (app, iced::widget::operation::focus_next())
    }

    fn title(&self) -> String {
        format!("{} — Polaris", self.filename())
    }

    fn theme(&self) -> Theme {
        theme::theme(self.dark)
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subs = Vec::new();
        if self.doc.is_dirty() && self.doc.path().is_some() {
            subs.push(iced::time::every(Duration::from_millis(250)).map(|_| Message::AutosaveTick));
        }
        if self.overlay != Overlay::None {
            subs.push(event::listen_with(overlay_key_events));
        }
        Subscription::batch(subs)
    }

    fn filename(&self) -> String {
        self.doc
            .path()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("untitled")
            .to_string()
    }

    fn now_ms(&self) -> u64 {
        self.epoch.elapsed().as_millis() as u64
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Edit(action) => {
                let is_edit = action.is_edit();
                self.content.perform(action);
                if is_edit {
                    self.status = None;
                    let new_text = self.content.text();
                    if new_text != self.last_text {
                        apply_diff(&mut self.doc, &self.last_text, &new_text);
                        self.last_text = new_text;
                        self.autosave.note_edit(self.now_ms());
                        if self.overlay == Overlay::Find {
                            self.refresh_matches();
                        }
                    }
                }
                Task::none()
            }
            Message::Save => {
                if self.doc.path().is_none() {
                    self.open_overlay(Overlay::SaveAs)
                } else {
                    self.save_now();
                    Task::none()
                }
            }
            Message::Undo => {
                if self.doc.undo() {
                    self.sync_from_doc();
                    self.autosave.note_edit(self.now_ms());
                }
                Task::none()
            }
            Message::Redo => {
                if self.doc.redo() {
                    self.sync_from_doc();
                    self.autosave.note_edit(self.now_ms());
                }
                Task::none()
            }
            Message::AutosaveTick => {
                if self.doc.is_dirty()
                    && self.doc.path().is_some()
                    && self.autosave.should_save(self.now_ms())
                {
                    self.save_now();
                }
                Task::none()
            }
            Message::FindOpen => self.open_overlay(Overlay::Find),
            Message::OverlayInput(value) => {
                self.input = value;
                if self.overlay == Overlay::Find {
                    self.refresh_matches();
                    // Jump to the first match at or after the caret.
                    let caret = char_index_of(self.doc.buffer(), self.content.cursor().position);
                    let first = self
                        .matches
                        .iter()
                        .position(|m| m.start >= caret)
                        .unwrap_or(0);
                    self.select_match(first);
                }
                Task::none()
            }
            Message::OverlaySubmit { backwards } => match self.overlay {
                Overlay::Find => {
                    if !self.matches.is_empty() {
                        let len = self.matches.len();
                        let next = if backwards {
                            (self.current_match + len - 1) % len
                        } else {
                            (self.current_match + 1) % len
                        };
                        self.select_match(next);
                    }
                    Task::none()
                }
                Overlay::SaveAs => {
                    let name = self.input.trim().to_string();
                    if name.is_empty() {
                        return Task::none();
                    }
                    match self.doc.save_as(PathBuf::from(&name)) {
                        Ok(()) => {
                            self.autosave.saved();
                            self.status = None;
                            self.close_overlay()
                        }
                        Err(e) => {
                            self.status = Some(format!("save failed: {e}"));
                            Task::none()
                        }
                    }
                }
                Overlay::None => Task::none(),
            },
            Message::OverlayClose => self.close_overlay(),
        }
    }

    fn open_overlay(&mut self, overlay: Overlay) -> Task<Message> {
        self.overlay = overlay;
        match overlay {
            Overlay::Find => self.refresh_matches(),
            Overlay::SaveAs => self.input.clear(),
            Overlay::None => {}
        }
        iced::widget::operation::focus(CHROME_INPUT_ID)
    }

    fn close_overlay(&mut self) -> Task<Message> {
        self.overlay = Overlay::None;
        iced::widget::operation::focus_next()
    }

    fn refresh_matches(&mut self) {
        self.matches = self.doc.find(&self.input);
        if self.current_match >= self.matches.len() {
            self.current_match = 0;
        }
    }

    /// Move the editor caret to select match `idx` (start..end).
    fn select_match(&mut self, idx: usize) {
        if let Some(range) = self.matches.get(idx).cloned() {
            self.current_match = idx;
            let buffer = self.doc.buffer();
            self.content.move_to(text_editor::Cursor {
                position: position_of(buffer, range.end),
                selection: Some(position_of(buffer, range.start)),
            });
        }
    }

    fn save_now(&mut self) {
        match self.doc.save() {
            Ok(()) => {
                self.autosave.saved();
                self.status = None;
            }
            Err(e) => self.status = Some(format!("save failed: {e}")),
        }
    }

    /// Rebuild widget content from the model (after undo/redo).
    fn sync_from_doc(&mut self) {
        self.content = Content::with_text(&self.doc.text());
        let position = position_of(self.doc.buffer(), self.doc.cursor().pos);
        self.content.move_to(text_editor::Cursor {
            position,
            selection: None,
        });
        self.last_text = self.content.text();
        if self.overlay == Overlay::Find {
            self.refresh_matches();
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let t = theme::tokens(self.dark);
        let quiet_text = |s: String| text(s).font(fonts::MONO).size(13).color(t.quiet);

        let chrome: Element<'_, Message> = match self.overlay {
            Overlay::None => {
                let right = match &self.status {
                    Some(status) => status.clone(),
                    None => format!(
                        "{} words{}",
                        self.doc.word_count(),
                        if self.doc.is_dirty() {
                            ""
                        } else {
                            "  ·  ● saved"
                        }
                    ),
                };
                row![
                    quiet_text(self.filename()),
                    space().width(Fill),
                    quiet_text(right),
                ]
                .into()
            }
            Overlay::Find => {
                let count = if self.input.is_empty() {
                    String::new()
                } else if self.matches.is_empty() {
                    "0/0".to_string()
                } else {
                    format!("{}/{}", self.current_match + 1, self.matches.len())
                };
                row![
                    quiet_text("find".to_string()),
                    self.chrome_input(""),
                    quiet_text(count),
                ]
                .spacing(12)
                .into()
            }
            Overlay::SaveAs => row![
                quiet_text("save as".to_string()),
                self.chrome_input("filename.md"),
            ]
            .spacing(12)
            .into(),
        };

        let editor = text_editor(&self.content)
            .on_action(Message::Edit)
            .key_binding(key_binding)
            .font(fonts::QUATTRO)
            .size(17.5)
            .line_height(text::LineHeight::Relative(1.62))
            .height(Fill)
            .padding(Padding {
                top: 4.0,
                right: 2.0,
                // Breathing room so the caret never writes at the window edge
                // (typewriter scrolling proper lands in Phase 2).
                bottom: 220.0,
                left: 2.0,
            })
            .style(move |_theme, _status| text_editor::Style {
                background: Background::Color(t.bg),
                border: Border::default(),
                placeholder: t.quiet,
                value: t.ink,
                selection: iced::Color { a: 0.35, ..t.star },
            });

        // ~62ch of Quattro at 17.5px
        let page = container(column![chrome, editor].spacing(26))
            .max_width(600)
            .height(Fill);

        container(page)
            .style(move |_| container::Style {
                background: Some(Background::Color(t.bg)),
                ..container::Style::default()
            })
            .center_x(Fill)
            .height(Fill)
            .padding(Padding {
                top: 76.0,
                right: 32.0,
                bottom: 0.0,
                left: 32.0,
            })
            .into()
    }

    fn chrome_input(&self, placeholder: &str) -> Element<'_, Message> {
        let t = theme::tokens(self.dark);
        text_input(placeholder, &self.input)
            .id(CHROME_INPUT_ID)
            .on_input(Message::OverlayInput)
            .font(fonts::MONO)
            .size(13)
            .padding(0)
            .style(move |_theme, _status| text_input::Style {
                background: Background::Color(t.bg),
                border: Border::default(),
                icon: t.quiet,
                placeholder: t.quiet,
                value: t.ink,
                selection: iced::Color { a: 0.35, ..t.star },
            })
            .into()
    }
}

fn key_binding(key_press: KeyPress) -> Option<Binding<Message>> {
    let modifiers = key_press.modifiers;
    if modifiers.command() {
        if let keyboard::Key::Character(c) = key_press.key.as_ref() {
            match c {
                "s" => return Some(Binding::Custom(Message::Save)),
                "f" => return Some(Binding::Custom(Message::FindOpen)),
                "z" if modifiers.shift() => return Some(Binding::Custom(Message::Redo)),
                "z" => return Some(Binding::Custom(Message::Undo)),
                _ => {}
            }
        }
    }
    Binding::from_key_press(key_press)
}

/// Enter / Shift+Enter / Esc for the chrome overlays. Only subscribed while
/// an overlay is open; `text_input` has no key-binding hook of its own.
fn overlay_key_events(
    event: iced::Event,
    _status: event::Status,
    _window: iced::window::Id,
) -> Option<Message> {
    if let iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event {
        match key.as_ref() {
            keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::OverlayClose),
            keyboard::Key::Named(keyboard::key::Named::Enter) => Some(Message::OverlaySubmit {
                backwards: modifiers.shift(),
            }),
            _ => None,
        }
    } else {
        None
    }
}

fn detect_dark() -> bool {
    matches!(dark_light::detect(), Ok(dark_light::Mode::Dark))
}

/// Widget `Position.column` is a byte offset within the line (cosmic-text's
/// cursor index); these convert to/from the model's char indices.
fn position_of(buffer: &Buffer, char_idx: usize) -> text_editor::Position {
    let line = buffer.char_to_line(char_idx);
    let line_start = buffer.line_to_char(line);
    let text = buffer.line(line);
    let column = text
        .char_indices()
        .nth(char_idx - line_start)
        .map(|(b, _)| b)
        .unwrap_or(text.len());
    text_editor::Position { line, column }
}

fn char_index_of(buffer: &Buffer, position: text_editor::Position) -> usize {
    let line = position.line.min(buffer.len_lines().saturating_sub(1));
    let line_start = buffer.line_to_char(line);
    let text = buffer.line(line);
    let byte = position.column.min(text.len());
    let mut chars = 0;
    for (b, _) in text.char_indices() {
        if b >= byte {
            break;
        }
        chars += 1;
    }
    line_start + chars
}

/// Sync one widget edit into the model as a minimal char-level edit
/// (common prefix/suffix), preserving core's undo grouping.
fn apply_diff(doc: &mut Document, old: &str, new: &str) {
    if old == new {
        return;
    }
    let old_chars: Vec<char> = old.chars().collect();
    let new_chars: Vec<char> = new.chars().collect();

    let mut prefix = 0;
    while prefix < old_chars.len()
        && prefix < new_chars.len()
        && old_chars[prefix] == new_chars[prefix]
    {
        prefix += 1;
    }
    let mut suffix = 0;
    while suffix < old_chars.len() - prefix
        && suffix < new_chars.len() - prefix
        && old_chars[old_chars.len() - 1 - suffix] == new_chars[new_chars.len() - 1 - suffix]
    {
        suffix += 1;
    }

    let inserted: String = new_chars[prefix..new_chars.len() - suffix].iter().collect();
    doc.replace_range(prefix..old_chars.len() - suffix, &inserted);
}

#[cfg(test)]
mod tests {
    use super::{apply_diff, char_index_of, position_of, App, Message, Overlay};
    use iced::widget::text_editor::{Action, Edit};
    use polaris_core::Document;

    fn type_into(app: &mut App, s: &str) {
        for c in s.chars() {
            let edit = if c == '\n' {
                Edit::Enter
            } else {
                Edit::Insert(c)
            };
            let _ = app.update(Message::Edit(Action::Edit(edit)));
        }
    }

    /// The full M3 loop, headless: edit -> debounce -> autosave hits disk.
    #[test]
    fn update_loop_autosaves_after_debounce() {
        let dir = std::env::temp_dir().join("polaris-gui-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("autosave.md");
        std::fs::write(&path, "start\n").unwrap();

        let (mut app, _) = App::boot(Some(path.clone()));
        type_into(&mut app, "more words ");

        // Before the debounce window: tick must not save.
        let _ = app.update(Message::AutosaveTick);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "start\n");
        assert!(app.doc.is_dirty());

        std::thread::sleep(std::time::Duration::from_millis(1050));
        let _ = app.update(Message::AutosaveTick);
        assert!(!app.doc.is_dirty());
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            app.content.text(),
            "autosave wrote the widget text"
        );
        assert!(std::fs::read_to_string(&path)
            .unwrap()
            .starts_with("more words start"));
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn save_as_overlay_binds_untitled_buffer_to_a_file() {
        let dir = std::env::temp_dir().join("polaris-gui-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("untitled-save.md");
        let _ = std::fs::remove_file(&path);

        let (mut app, _) = App::boot(None);
        type_into(&mut app, "draft one");
        let _ = app.update(Message::Save); // untitled -> opens save-as
        assert_eq!(app.overlay, Overlay::SaveAs);
        let _ = app.update(Message::OverlayInput(path.to_str().unwrap().to_string()));
        let _ = app.update(Message::OverlaySubmit { backwards: false });
        assert_eq!(app.overlay, Overlay::None);
        assert!(std::fs::read_to_string(&path)
            .unwrap()
            .starts_with("draft one"));
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn find_overlay_matches_and_cycles() {
        let (mut app, _) = App::boot(None);
        type_into(&mut app, "alpha beta alpha gamma Alpha");
        let _ = app.update(Message::FindOpen);
        let _ = app.update(Message::OverlayInput("alpha".to_string()));
        assert_eq!(app.matches.len(), 3, "case-insensitive matches");

        let first = app.current_match;
        let _ = app.update(Message::OverlaySubmit { backwards: false });
        assert_eq!(app.current_match, (first + 1) % 3);
        let _ = app.update(Message::OverlaySubmit { backwards: true });
        assert_eq!(app.current_match, first);

        // Selection follows the current match in the widget.
        let selected = app.content.selection().map(|s| s.to_lowercase());
        assert_eq!(selected.as_deref(), Some("alpha"));

        let _ = app.update(Message::OverlayClose);
        assert_eq!(app.overlay, Overlay::None);
    }

    fn diff_roundtrip(old: &str, new: &str) {
        let mut doc = Document::from_str(old);
        apply_diff(&mut doc, old, new);
        assert_eq!(doc.text(), new, "old={old:?} new={new:?}");
    }

    #[test]
    fn apply_diff_covers_edit_shapes() {
        diff_roundtrip("", "a");
        diff_roundtrip("abc", "abxc");
        diff_roundtrip("abc", "ac");
        diff_roundtrip("abc", "abc\n");
        diff_roundtrip("hello world", "hello brave world");
        diff_roundtrip("aaa", "aa"); // ambiguous repeats
        diff_roundtrip("café", "cafés");
        diff_roundtrip("x", "");
        diff_roundtrip("one two", "one three"); // replace tail word
    }

    #[test]
    fn apply_diff_typing_preserves_undo_grouping() {
        let mut doc = Document::new();
        let mut text = String::new();
        for c in "hello world".chars() {
            let new = format!("{text}{c}");
            apply_diff(&mut doc, &text, &new);
            text = new;
        }
        doc.undo();
        assert_eq!(doc.text(), "hello ");
    }

    #[test]
    fn position_roundtrip_with_multibyte_lines() {
        let doc = Document::from_str("café line\nsecond — line\nzz");
        let buffer = doc.buffer();
        for char_idx in [0, 3, 4, 9, 10, 12, 18, 24, 26] {
            let pos = position_of(buffer, char_idx);
            assert_eq!(
                char_index_of(buffer, pos),
                char_idx,
                "roundtrip at {char_idx}"
            );
        }
        // é is 2 bytes: char 4 (space after café) is at byte column 5
        assert_eq!(position_of(buffer, 4).column, 5);
    }
}
