//! The iced GUI shell (Phase 1, M2–M3).
//!
//! Per PLAN §7 decision #3, the view/interaction layer is iced's
//! `text_editor`; `polaris-core::Document` stays the document model. Every
//! editor action that changes text is synced into the `Document` as a
//! char-level diff via `replace_range`, which preserves core's word-sized
//! undo grouping. Undo/redo run in core and rebuild the widget content.
//! The custom cosmic-text widget replaces this shim in Phase 2.

mod fonts;
mod preview;
mod theme;

use std::ops::Range;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use iced::widget::text_editor;
use iced::widget::text_editor::{Binding, Content, Edit, KeyPress};
use iced::widget::{column, container, row, scrollable, space, text, text_input};
use iced::{
    event, keyboard, Background, Border, Element, Fill, Padding, Subscription, Task, Theme,
};

use polaris_core::buffer::Buffer;
use polaris_core::{typography, AutosaveTimer, Document};

const CHROME_INPUT_ID: &str = "chrome-input";
const PREVIEW_SCROLL_ID: &str = "preview-scroll";

/// DESIGN.md chrome fade: 0.6s out on keystroke, back 1.2s after rest.
const FADE_OUT_SECS: f32 = 0.6;
const FADE_IN_SECS: f32 = 0.3;
const FADE_REST_MS: u64 = 1200;

pub fn run(path: Option<PathBuf>) -> iced::Result {
    iced::application(move || App::boot(path.clone()), App::update, App::view)
        .title(App::title)
        .theme(App::theme)
        .subscription(App::subscription)
        .font(fonts::SANS_REGULAR_BYTES)
        .font(fonts::SANS_ITALIC_BYTES)
        .font(fonts::SANS_SEMIBOLD_BYTES)
        .font(fonts::MONO_REGULAR_BYTES)
        .font(fonts::READING_REGULAR_BYTES)
        .font(fonts::READING_ITALIC_BYTES)
        .font(fonts::READING_SEMIBOLD_BYTES)
        .default_font(fonts::WRITING)
        .window_size(iced::Size::new(760.0, 940.0))
        .run()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Overlay {
    None,
    Find,
    SaveAs,
    /// Cmd+D confirmation: page + mode shown, Enter deploys, Esc cancels.
    Deploy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Write,
    Preview,
}

/// One backspace right after a smart-punctuation substitution restores the
/// literal keystrokes.
#[derive(Debug, Clone)]
struct Revert {
    /// Chars the substitution inserted (to delete).
    inserted: usize,
    /// The literal text the writer actually typed (to restore).
    literal: String,
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
    view_mode: ViewMode,
    /// Chrome opacity: fades toward 0 while typing, back to 1 at rest.
    chrome_alpha: f32,
    last_key_ms: Option<u64>,
    pending_revert: Option<Revert>,
    deploy_token: Option<String>,
    deploy_page: Option<String>,
    deploying: bool,
}

#[derive(Debug, Clone)]
enum Message {
    Edit(text_editor::Action),
    Save,
    Undo,
    Redo,
    AutosaveTick,
    FadeTick,
    TogglePreview,
    DeployOpen,
    DeployDone(Result<String, String>),
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
            view_mode: ViewMode::Write,
            chrome_alpha: 1.0,
            last_key_ms: None,
            pending_revert: None,
            deploy_token: None,
            deploy_page: None,
            deploying: false,
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
        if self.view_mode == ViewMode::Preview {
            subs.push(event::listen_with(preview_key_events));
        }
        // Fade animation ticks: while typing recently or not yet fully back.
        let recently_typed = self
            .last_key_ms
            .is_some_and(|t| self.now_ms().saturating_sub(t) < FADE_REST_MS + 100);
        if self.chrome_alpha < 1.0 || recently_typed {
            subs.push(iced::time::every(Duration::from_millis(40)).map(|_| Message::FadeTick));
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
                self.perform_with_typography(action);
                if is_edit {
                    self.status = None;
                    self.last_key_ms = Some(self.now_ms());
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
            Message::FadeTick => {
                let now = self.now_ms();
                let target = if self.overlay != Overlay::None || self.view_mode == ViewMode::Preview
                {
                    1.0
                } else if self
                    .last_key_ms
                    .is_some_and(|t| now.saturating_sub(t) < FADE_REST_MS)
                {
                    0.0
                } else {
                    1.0
                };
                let dt = 0.040_f32;
                if target < self.chrome_alpha {
                    self.chrome_alpha = (self.chrome_alpha - dt / FADE_OUT_SECS).max(0.0);
                } else if target > self.chrome_alpha {
                    self.chrome_alpha = (self.chrome_alpha + dt / FADE_IN_SECS).min(1.0);
                }
                Task::none()
            }
            Message::TogglePreview => match self.view_mode {
                ViewMode::Write => {
                    self.view_mode = ViewMode::Preview;
                    self.chrome_alpha = 1.0;
                    // Approximate scroll preservation: land at the caret's
                    // relative position in the document.
                    let caret = char_index_of(self.doc.buffer(), self.content.cursor().position);
                    let line = self.doc.buffer().char_to_line(caret) as f32;
                    let total = self.doc.buffer().len_lines().max(2) as f32;
                    iced::widget::operation::snap_to(
                        PREVIEW_SCROLL_ID,
                        scrollable::RelativeOffset {
                            x: 0.0,
                            y: (line / (total - 1.0)).clamp(0.0, 1.0),
                        },
                    )
                }
                ViewMode::Preview => {
                    self.view_mode = ViewMode::Write;
                    iced::widget::operation::focus_next()
                }
            },
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
            Message::DeployOpen => {
                if self.deploying {
                    return Task::none();
                }
                if self.doc.path().is_none() {
                    self.status = Some("save before deploying (Cmd+S)".to_string());
                    return Task::none();
                }
                match crate::config::Config::load() {
                    Ok(config) => match (config.notion.token, config.notion.default_page) {
                        (Some(token), Some(page)) => {
                            self.deploy_token = Some(token);
                            self.deploy_page = Some(page);
                            self.open_overlay(Overlay::Deploy)
                        }
                        _ => {
                            self.status = Some(
                                "notion not configured — polaris config --token … --default-page …"
                                    .to_string(),
                            );
                            Task::none()
                        }
                    },
                    Err(e) => {
                        self.status = Some(format!("config error: {e}"));
                        Task::none()
                    }
                }
            }
            Message::DeployDone(result) => {
                self.deploying = false;
                self.status = Some(match result {
                    Ok(url) => format!(
                        "✓ deployed {} → {}",
                        chrono::Local::now().format("%H:%M"),
                        url
                    ),
                    Err(e) => format!("deploy failed: {e}"),
                });
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
                Overlay::Deploy => {
                    self.save_now();
                    let (Some(token), Some(page)) =
                        (self.deploy_token.clone(), self.deploy_page.clone())
                    else {
                        return self.close_overlay();
                    };
                    let markdown = self.doc.text();
                    self.deploying = true;
                    self.status = Some("deploying…".to_string());
                    let close = self.close_overlay();
                    Task::batch([
                        close,
                        Task::perform(
                            async move {
                                polaris_notion::NotionClient::new(token)
                                    .deploy(&markdown, &page, polaris_notion::PublishMode::Append)
                                    .await
                                    .map_err(|e| e.to_string())
                            },
                            Message::DeployDone,
                        ),
                    ])
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
            // Deploy has no input; Enter/Esc arrive via the overlay
            // subscription, so the missing-id focus below is a no-op.
            Overlay::Deploy | Overlay::None => {}
        }
        self.chrome_alpha = 1.0;
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

    /// Perform an editor action, applying smart punctuation to plain char
    /// inserts (DESIGN.md: applied at input time so the file carries the real
    /// characters). Never inside code spans/fences; one backspace right after
    /// a substitution restores the literal keystrokes.
    fn perform_with_typography(&mut self, action: text_editor::Action) {
        match &action {
            text_editor::Action::Edit(Edit::Insert(c)) if self.content.selection().is_none() => {
                let caret = char_index_of(self.doc.buffer(), self.content.cursor().position);
                let before = self.doc.buffer().slice(0..caret);
                if !in_code_context(&before) {
                    if let Some(sub) = typography::substitute(&before, *c) {
                        let mut literal: String = before
                            .chars()
                            .rev()
                            .take(sub.delete_before)
                            .collect::<Vec<_>>()
                            .into_iter()
                            .rev()
                            .collect();
                        literal.push(*c);
                        for _ in 0..sub.delete_before {
                            self.content
                                .perform(text_editor::Action::Edit(Edit::Backspace));
                        }
                        for ch in sub.insert.chars() {
                            self.content
                                .perform(text_editor::Action::Edit(Edit::Insert(ch)));
                        }
                        self.pending_revert = Some(Revert {
                            inserted: sub.insert.chars().count(),
                            literal,
                        });
                        return;
                    }
                }
                self.pending_revert = None;
                self.content.perform(action);
            }
            text_editor::Action::Edit(Edit::Backspace) => {
                if let Some(revert) = self.pending_revert.take() {
                    for _ in 0..revert.inserted {
                        self.content
                            .perform(text_editor::Action::Edit(Edit::Backspace));
                    }
                    for ch in revert.literal.chars() {
                        self.content
                            .perform(text_editor::Action::Edit(Edit::Insert(ch)));
                    }
                    return;
                }
                self.content.perform(action);
            }
            _ => {
                self.pending_revert = None;
                self.content.perform(action);
            }
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
        let chrome_color = iced::Color {
            a: t.quiet.a * self.chrome_alpha,
            ..t.quiet
        };
        let quiet_text = |s: String| text(s).font(fonts::MONO).size(13).color(chrome_color);

        let chrome: Element<'_, Message> = match self.overlay {
            Overlay::None => {
                let words = self.doc.word_count();
                let right = match &self.status {
                    Some(status) => status.clone(),
                    None => {
                        let reading = format!(" · {} min", (words as f32 / 220.0).ceil().max(1.0));
                        format!(
                            "{} words{}{}{}",
                            words,
                            if words > 0 { &reading } else { "" },
                            if self.view_mode == ViewMode::Preview {
                                " · preview"
                            } else {
                                ""
                            },
                            if self.doc.is_dirty() {
                                ""
                            } else {
                                " · ● saved"
                            }
                        )
                    }
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
            Overlay::Deploy => {
                let page = self.deploy_page.as_deref().unwrap_or("?");
                let short: String = page.chars().take(8).collect();
                row![
                    text("deploy to notion")
                        .font(fonts::MONO)
                        .size(13)
                        .color(t.star),
                    quiet_text(format!("append → {short}…")),
                    space().width(Fill),
                    quiet_text("Enter confirm · Esc cancel".to_string()),
                ]
                .spacing(12)
                .into()
            }
        };

        let body: Element<'_, Message> = match self.view_mode {
            ViewMode::Write => text_editor(&self.content)
                .on_action(Message::Edit)
                .key_binding(key_binding)
                .font(fonts::WRITING)
                .size(17.5)
                .line_height(text::LineHeight::Relative(1.62))
                .height(Fill)
                .padding(Padding {
                    top: 4.0,
                    right: 2.0,
                    // Breathing room so the caret never writes at the window
                    // edge (typewriter scrolling proper lands in Phase 2).
                    bottom: 220.0,
                    left: 2.0,
                })
                .style(move |_theme, _status| text_editor::Style {
                    background: Background::Color(t.bg),
                    border: Border::default(),
                    placeholder: t.quiet,
                    value: t.ink,
                    selection: iced::Color { a: 0.35, ..t.star },
                })
                .into(),
            ViewMode::Preview => scrollable(container(preview::view(&self.last_text, t)).padding(
                Padding {
                    top: 4.0,
                    right: 2.0,
                    bottom: 220.0,
                    left: 2.0,
                },
            ))
            .id(PREVIEW_SCROLL_ID)
            .height(Fill)
            .width(Fill)
            .into(),
        };

        // ~62ch at 17.5px
        let page = container(column![chrome, body].spacing(26))
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
                "p" => return Some(Binding::Custom(Message::TogglePreview)),
                "d" => return Some(Binding::Custom(Message::DeployOpen)),
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

/// Cmd/Ctrl+P or Esc leaves preview; Cmd/Ctrl+S still saves. Subscribed only
/// while previewing (the editor and its bindings aren't mounted then).
fn preview_key_events(
    event: iced::Event,
    _status: event::Status,
    _window: iced::window::Id,
) -> Option<Message> {
    if let iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event {
        match key.as_ref() {
            keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::TogglePreview),
            keyboard::Key::Character("p") if modifiers.command() => Some(Message::TogglePreview),
            keyboard::Key::Character("s") if modifiers.command() => Some(Message::Save),
            _ => None,
        }
    } else {
        None
    }
}

/// Markdown context guard for smart punctuation: inside a fenced code block
/// (odd number of ``` fence lines so far) or an inline code span (odd number
/// of backticks on the current line).
fn in_code_context(before: &str) -> bool {
    let fences = before
        .lines()
        .filter(|l| l.trim_start().starts_with("```"))
        .count();
    if fences % 2 == 1 {
        return true;
    }
    let line = before.rsplit('\n').next().unwrap_or(before);
    line.chars().filter(|&c| c == '`').count() % 2 == 1
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
    use super::{apply_diff, char_index_of, position_of, App, Message, Overlay, ViewMode};
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
    fn smart_punctuation_applies_on_input() {
        let (mut app, _) = App::boot(None);
        type_into(&mut app, "wait -- \"really\" it's...");
        assert_eq!(
            app.doc.text().trim_end(),
            "wait \u{2014} \u{201C}really\u{201D} it\u{2019}s\u{2026}"
        );
    }

    #[test]
    fn smart_punctuation_skipped_in_code_contexts() {
        let (mut app, _) = App::boot(None);
        type_into(
            &mut app,
            "```\n--verbose \"flag\"\n```\nand `--inline \"x\"` here",
        );
        let text = app.doc.text();
        assert!(text.contains("--verbose \"flag\""), "fence stays literal");
        assert!(
            text.contains("`--inline \"x\"`"),
            "inline span stays literal"
        );
    }

    #[test]
    fn backspace_right_after_substitution_reverts_to_literal() {
        let (mut app, _) = App::boot(None);
        type_into(&mut app, "a--");
        assert!(app.doc.text().starts_with("a\u{2014}"));
        let _ = app.update(Message::Edit(Action::Edit(Edit::Backspace)));
        assert!(app.doc.text().starts_with("a--"), "literal restored");
        // A second backspace is a plain backspace again.
        let _ = app.update(Message::Edit(Action::Edit(Edit::Backspace)));
        assert!(app.doc.text().starts_with("a-"));
    }

    #[test]
    fn markdown_rule_stays_typeable() {
        let (mut app, _) = App::boot(None);
        type_into(&mut app, "text\n\n---");
        assert!(
            app.doc.text().contains("\n---"),
            "hr not turned into a dash"
        );
    }

    #[test]
    fn preview_toggles_and_chrome_returns() {
        let (mut app, _) = App::boot(None);
        type_into(&mut app, "# Title\n\nsome *styled* text");
        assert_eq!(app.view_mode, ViewMode::Write);
        let _ = app.update(Message::TogglePreview);
        assert_eq!(app.view_mode, ViewMode::Preview);
        // Fade target is 1.0 in preview even right after typing.
        app.chrome_alpha = 0.2;
        let _ = app.update(Message::FadeTick);
        assert!(app.chrome_alpha > 0.2);
        let _ = app.update(Message::TogglePreview);
        assert_eq!(app.view_mode, ViewMode::Write);
    }

    #[test]
    fn typing_fades_chrome_and_rest_restores_it() {
        let (mut app, _) = App::boot(None);
        type_into(&mut app, "x");
        for _ in 0..30 {
            let _ = app.update(Message::FadeTick);
        }
        assert_eq!(app.chrome_alpha, 0.0, "faded out while typing recently");
        std::thread::sleep(std::time::Duration::from_millis(1250));
        for _ in 0..30 {
            let _ = app.update(Message::FadeTick);
        }
        assert_eq!(app.chrome_alpha, 1.0, "returned after rest");
    }

    #[test]
    fn deploy_requires_a_saved_file() {
        let (mut app, _) = App::boot(None);
        let _ = app.update(Message::DeployOpen);
        assert_eq!(app.overlay, Overlay::None);
        assert!(app.status.as_deref().unwrap_or("").contains("save"));
    }

    #[test]
    fn deploy_confirm_saves_and_starts_exactly_one_deploy() {
        let dir = std::env::temp_dir().join("polaris-gui-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("deploy.md");
        std::fs::write(&path, "content\n").unwrap();

        let (mut app, _) = App::boot(Some(path.clone()));
        type_into(&mut app, "更 ");
        // Simulate a configured deploy confirmation (bypasses Config::load).
        app.deploy_token = Some("secret".into());
        app.deploy_page = Some("abc123def456".into());
        app.overlay = Overlay::Deploy;

        let _ = app.update(Message::OverlaySubmit { backwards: false });
        assert!(app.deploying);
        assert_eq!(app.overlay, Overlay::None);
        assert!(!app.doc.is_dirty(), "saved before deploying");
        assert!(app.status.as_deref().unwrap_or("").contains("deploying"));

        // Re-triggering while in flight is a no-op.
        let _ = app.update(Message::DeployOpen);
        assert_eq!(app.overlay, Overlay::None);

        let _ = app.update(Message::DeployDone(Ok("https://notion.so/x".into())));
        assert!(!app.deploying);
        assert!(app.status.as_deref().unwrap_or("").contains("deployed"));

        let _ = app.update(Message::DeployDone(Err("401".into())));
        assert!(app
            .status
            .as_deref()
            .unwrap_or("")
            .contains("deploy failed"));
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
