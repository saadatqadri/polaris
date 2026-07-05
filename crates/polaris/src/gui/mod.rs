//! The iced GUI shell (Phase 1, M2).
//!
//! Per PLAN §7 decision #3, the view/interaction layer is iced's
//! `text_editor`; `polaris-core::Document` stays the document model. Every
//! editor action that changes text is synced into the `Document` as a
//! char-level diff via `replace_range`, which preserves core's word-sized
//! undo grouping. Undo/redo run in core and rebuild the widget content.
//! The custom cosmic-text widget replaces this shim in Phase 2.

mod fonts;
mod theme;

use std::path::PathBuf;

use iced::widget::text_editor;
use iced::widget::text_editor::{Binding, Content, KeyPress};
use iced::widget::{column, container, row, space, text};
use iced::{keyboard, Background, Border, Element, Fill, Padding, Theme};

use polaris_core::Document;

pub fn run(path: Option<PathBuf>) -> iced::Result {
    iced::application(move || App::boot(path.clone()), App::update, App::view)
        .title(App::title)
        .theme(App::theme)
        .font(fonts::QUATTRO_REGULAR_BYTES)
        .font(fonts::QUATTRO_BOLD_BYTES)
        .font(fonts::QUATTRO_ITALIC_BYTES)
        .font(fonts::MONO_REGULAR_BYTES)
        .default_font(fonts::QUATTRO)
        .window_size(iced::Size::new(760.0, 940.0))
        .run()
}

struct App {
    doc: Document,
    content: Content,
    /// The widget's text as of the last sync; diffed against on each edit.
    last_text: String,
    dark: bool,
    status: Option<String>,
}

#[derive(Debug, Clone)]
enum Message {
    Edit(text_editor::Action),
    Save,
    Undo,
    Redo,
}

impl App {
    fn boot(path: Option<PathBuf>) -> (Self, iced::Task<Message>) {
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

    fn filename(&self) -> String {
        self.doc
            .path()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("untitled")
            .to_string()
    }

    fn update(&mut self, message: Message) {
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
                    }
                }
            }
            Message::Save => {
                if self.doc.path().is_none() {
                    // In-window save-as lands in M3.
                    self.status = Some("untitled — run with a filename to save".to_string());
                } else if let Err(e) = self.doc.save() {
                    self.status = Some(format!("save failed: {e}"));
                }
            }
            Message::Undo => {
                if self.doc.undo() {
                    self.sync_from_doc();
                }
            }
            Message::Redo => {
                if self.doc.redo() {
                    self.sync_from_doc();
                }
            }
        }
    }

    /// Rebuild widget content from the model (after undo/redo).
    fn sync_from_doc(&mut self) {
        self.content = Content::with_text(&self.doc.text());
        let (line, column) = self.doc.line_col();
        self.content.move_to(text_editor::Cursor {
            position: text_editor::Position { line, column },
            selection: None,
        });
        self.last_text = self.content.text();
    }

    fn view(&self) -> Element<'_, Message> {
        let t = theme::tokens(self.dark);

        let right = match &self.status {
            Some(status) => status.clone(),
            None => format!(
                "{} words{}",
                self.doc.word_count(),
                if self.doc.is_dirty() {
                    ""
                } else {
                    "  ·  saved"
                }
            ),
        };
        let chrome = row![
            text(self.filename())
                .font(fonts::MONO)
                .size(13)
                .color(t.quiet),
            space().width(Fill),
            text(right).font(fonts::MONO).size(13).color(t.quiet),
        ];

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
}

fn key_binding(key_press: KeyPress) -> Option<Binding<Message>> {
    let modifiers = key_press.modifiers;
    if modifiers.command() {
        if let keyboard::Key::Character(c) = key_press.key.as_ref() {
            match c {
                "s" => return Some(Binding::Custom(Message::Save)),
                "z" if modifiers.shift() => return Some(Binding::Custom(Message::Redo)),
                "z" => return Some(Binding::Custom(Message::Undo)),
                _ => {}
            }
        }
    }
    Binding::from_key_press(key_press)
}

fn detect_dark() -> bool {
    matches!(dark_light::detect(), Ok(dark_light::Mode::Dark))
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
    use super::apply_diff;
    use polaris_core::Document;

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
}
