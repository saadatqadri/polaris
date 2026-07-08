//! The iced GUI shell (Phase 1 M2–M5, Phase 2 editor widget).
//!
//! Since the Phase 2 promotion, the editor surface is our own widget
//! ([`editor::EditorView`]) rendering `polaris-core::Document` directly:
//! the Document is the single source of truth, the widget emits
//! [`editor::Action`]s, and there is no sync layer. Typewriter scrolling
//! (Cmd+Y) and focus dimming (Cmd+G) are widget flags.

mod editor;
mod fonts;
mod preview;
mod theme;

use std::ops::Range;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use iced::widget::{column, container, row, scrollable, space, text, text_input};
use iced::{
    event, keyboard, Background, Border, Element, Fill, Padding, Subscription, Task, Theme,
};

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
        .font(fonts::WRITING_REGULAR_BYTES)
        .font(fonts::WRITING_ITALIC_BYTES)
        .font(fonts::WRITING_SEMIBOLD_BYTES)
        .font(fonts::MONO_REGULAR_BYTES)
        .font(fonts::READING_REGULAR_BYTES)
        .font(fonts::READING_ITALIC_BYTES)
        .font(fonts::READING_SEMIBOLD_BYTES)
        .default_font(fonts::WRITING)
        .window_size(iced::Size::new(760.0, 940.0))
        // Close requests route through update() so the buffer is flushed
        // before exit (see Message::CloseRequested).
        .exit_on_close_request(false)
        .run()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Overlay {
    None,
    Find,
    SaveAs,
    /// Cmd+R: input prefilled with the current name; Enter renames on disk.
    Rename,
    /// Cmd+D confirmation: page + mode shown, Enter deploys, Esc cancels.
    Deploy,
    /// Cmd+L: session word goal — a number sets it, empty clears it.
    Goal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Write,
    Preview,
}

/// A session word goal: progress counts words written since it was set.
#[derive(Debug, Clone, Copy)]
struct Goal {
    target: usize,
    baseline: usize,
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
    /// Bumped on every text change; keys the widget's paragraph cache.
    text_version: u64,
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
    /// An untitled buffer got one close warning; the next close discards.
    close_pending: bool,
    /// Phase 2 writing modes (session flags for now).
    typewriter: bool,
    focus_dim: bool,
    /// Hemingway mode: backspace/delete/cut disabled — forward only.
    hemingway: bool,
    /// Zen mode: chrome hidden until summoned (overlays and status still show).
    zen: bool,
    goal: Option<Goal>,
}

#[derive(Debug, Clone)]
enum Message {
    Editor(editor::Action),
    Save,
    AutosaveTick,
    FadeTick,
    TogglePreview,
    ToggleTheme,
    DeployOpen,
    DeployDone(Result<String, String>),
    FindOpen,
    RenameOpen,
    OverlayInput(String),
    OverlaySubmit { backwards: bool },
    OverlayClose,
    CloseRequested(iced::window::Id),
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

        let app = Self {
            doc,
            text_version: 0,
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
            close_pending: false,
            typewriter: false,
            focus_dim: false,
            hemingway: false,
            zen: false,
            goal: None,
        };

        (app, Task::none())
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
        subs.push(iced::window::close_requests().map(Message::CloseRequested));
        // Fade animation ticks: while not at the target or typing recently.
        let recently_typed = self
            .last_key_ms
            .is_some_and(|t| self.now_ms().saturating_sub(t) < FADE_REST_MS + 100);
        if (self.chrome_alpha - self.chrome_target()).abs() > f32::EPSILON || recently_typed {
            subs.push(iced::time::every(Duration::from_millis(40)).map(|_| Message::FadeTick));
        }
        Subscription::batch(subs)
    }

    /// Where the chrome fade is heading. Overlays, preview, and status
    /// messages always summon it; zen hides it; typing hides it briefly.
    fn chrome_target(&self) -> f32 {
        if self.overlay != Overlay::None
            || self.view_mode == ViewMode::Preview
            || self.status.is_some()
        {
            1.0
        } else if self.zen {
            0.0
        } else if self
            .last_key_ms
            .is_some_and(|t| self.now_ms().saturating_sub(t) < FADE_REST_MS)
        {
            0.0
        } else {
            1.0
        }
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

    /// Bookkeeping after any text mutation.
    fn note_edit(&mut self) {
        self.text_version += 1;
        self.status = None;
        self.close_pending = false; // new content re-arms the close warning
        self.last_key_ms = Some(self.now_ms());
        self.autosave.note_edit(self.now_ms());
        if self.overlay == Overlay::Find {
            self.refresh_matches();
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Editor(action) => self.on_editor_action(action),
            Message::FadeTick => {
                let target = self.chrome_target();
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
                    let line = self.doc.buffer().char_to_line(self.doc.cursor().pos) as f32;
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
                    Task::none()
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
            Message::AutosaveTick => {
                if self.doc.is_dirty()
                    && self.doc.path().is_some()
                    && self.autosave.should_save(self.now_ms())
                {
                    self.save_now();
                }
                Task::none()
            }
            Message::ToggleTheme => {
                self.dark = !self.dark;
                // Persist the choice (best-effort). Skipped under test so the
                // suite never writes the developer's real ~/.polaris.toml.
                if !cfg!(test) {
                    let mut config = crate::config::Config::load().unwrap_or_default();
                    config.theme = Some(if self.dark { "dark" } else { "light" }.to_string());
                    let _ = config.save();
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
            Message::RenameOpen => {
                if self.doc.path().is_none() {
                    // Nothing to rename yet — naming an untitled buffer is save-as.
                    self.open_overlay(Overlay::SaveAs)
                } else {
                    self.open_overlay(Overlay::Rename)
                }
            }
            Message::OverlayInput(value) => {
                self.input = value;
                if self.overlay == Overlay::Find {
                    self.refresh_matches();
                    // Jump to the first match at or after the caret.
                    let caret = self.doc.cursor().pos;
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
                Overlay::Rename => {
                    let name = self.input.trim().to_string();
                    if name.is_empty() {
                        return Task::none();
                    }
                    // Bare names rename within the file's own directory;
                    // paths with separators are taken as given.
                    let target = PathBuf::from(&name);
                    let target = if target.is_absolute() || name.contains(std::path::MAIN_SEPARATOR)
                    {
                        target
                    } else {
                        self.doc
                            .path()
                            .and_then(|p| p.parent())
                            .map(|dir| dir.join(&name))
                            .unwrap_or(target)
                    };
                    match self.doc.rename(target) {
                        Ok(()) => {
                            self.status = None;
                            self.close_overlay()
                        }
                        Err(e) => {
                            self.status = Some(format!("rename failed: {e}"));
                            Task::none()
                        }
                    }
                }
                Overlay::Goal => {
                    let value = self.input.trim().to_string();
                    if value.is_empty() {
                        self.goal = None;
                        return self.close_overlay();
                    }
                    match value.parse::<usize>() {
                        Ok(target) if target > 0 => {
                            self.goal = Some(Goal {
                                target,
                                baseline: self.doc.word_count(),
                            });
                            self.close_overlay()
                        }
                        _ => {
                            self.status = Some("goal: enter a word count, e.g. 500".to_string());
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
            Message::CloseRequested(id) => {
                if self.doc.path().is_some() {
                    if self.doc.is_dirty() {
                        self.save_now();
                        if self.doc.is_dirty() {
                            // Save failed; status has the reason — stay open.
                            return Task::none();
                        }
                    }
                    iced::window::close(id)
                } else if !self.doc.text().trim().is_empty() && !self.close_pending {
                    // Untitled with content: one chance to name it.
                    self.close_pending = true;
                    self.status =
                        Some("unsaved — name it (Enter), or close again to discard".to_string());
                    self.open_overlay(Overlay::SaveAs)
                } else {
                    iced::window::close(id)
                }
            }
        }
    }

    fn on_editor_action(&mut self, action: editor::Action) -> Task<Message> {
        use editor::Action as A;
        match action {
            // Hemingway mode: forward only — deletion waits for the edit pass.
            A::Backspace | A::Delete | A::Cut if self.hemingway => {}
            A::Insert(s) => {
                self.insert_with_typography(&s);
                self.note_edit();
            }
            A::Enter => {
                self.doc.insert_newline();
                self.pending_revert = None;
                self.note_edit();
            }
            A::Backspace => {
                if let Some(revert) = self.pending_revert.take() {
                    let pos = self.doc.cursor().pos;
                    self.doc
                        .replace_range(pos.saturating_sub(revert.inserted)..pos, &revert.literal);
                } else {
                    self.doc.backspace();
                }
                self.note_edit();
            }
            A::Delete => {
                self.doc.delete_forward();
                self.pending_revert = None;
                self.note_edit();
            }
            A::Move(motion, extend) => {
                use editor::Motion as M;
                self.pending_revert = None;
                match motion {
                    M::Left => self.doc.move_left(extend),
                    M::Right => self.doc.move_right(extend),
                    M::Up => self.doc.move_up(extend),
                    M::Down => self.doc.move_down(extend),
                    M::WordLeft => self.doc.move_word_left(extend),
                    M::WordRight => self.doc.move_word_right(extend),
                    M::Home => self.doc.move_line_start(extend),
                    M::End => self.doc.move_line_end(extend),
                }
            }
            A::SelectAll => self.doc.select_all(),
            A::Click { position, extend } => {
                self.pending_revert = None;
                self.doc.set_cursor_pos(position, extend);
            }
            A::DragTo { position } => self.doc.set_cursor_pos(position, true),
            A::SelectWord { position } => {
                let range = editor::word_range_at(self.doc.buffer(), position);
                self.doc.set_cursor_pos(range.start, false);
                self.doc.set_cursor_pos(range.end, true);
            }
            A::Cut => {
                if self.doc.selection().is_some() {
                    self.doc.backspace();
                    self.note_edit();
                }
            }
            A::Paste(s) => {
                if !s.is_empty() {
                    self.doc.insert_str(&s);
                    self.pending_revert = None;
                    self.note_edit();
                }
            }
            A::Undo => {
                if self.doc.undo() {
                    self.pending_revert = None;
                    self.note_edit();
                }
            }
            A::Redo => {
                if self.doc.redo() {
                    self.pending_revert = None;
                    self.note_edit();
                }
            }
            A::Command(key) => return self.on_command(&key),
        }
        Task::none()
    }

    /// The app-level keymap, reached through the editor widget's unclaimed
    /// Cmd/Ctrl shortcuts.
    fn on_command(&mut self, key: &str) -> Task<Message> {
        match key {
            "s" => self.update(Message::Save),
            "f" => self.update(Message::FindOpen),
            "r" => self.update(Message::RenameOpen),
            "p" => self.update(Message::TogglePreview),
            "t" => self.update(Message::ToggleTheme),
            "d" => self.update(Message::DeployOpen),
            "y" => {
                self.typewriter = !self.typewriter;
                Task::none()
            }
            "g" => {
                self.focus_dim = !self.focus_dim;
                Task::none()
            }
            "e" => {
                self.hemingway = !self.hemingway;
                Task::none()
            }
            "k" => {
                self.zen = !self.zen;
                Task::none()
            }
            "l" => self.open_overlay(Overlay::Goal),
            _ => Task::none(),
        }
    }

    /// Insert typed text, applying smart punctuation to single plain chars
    /// (DESIGN.md: applied at input time so the file carries the real
    /// characters). Never inside code spans/fences.
    fn insert_with_typography(&mut self, s: &str) {
        let mut chars = s.chars();
        let (Some(c), None) = (chars.next(), chars.next()) else {
            self.doc.insert_str(s);
            self.pending_revert = None;
            return;
        };
        if self.doc.selection().is_none() {
            let pos = self.doc.cursor().pos;
            let before = self.doc.buffer().slice(0..pos);
            if !in_code_context(&before) {
                if let Some(sub) = typography::substitute(&before, c) {
                    let mut literal: String = before
                        .chars()
                        .rev()
                        .take(sub.delete_before)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect();
                    literal.push(c);
                    self.doc
                        .replace_range(pos - sub.delete_before..pos, sub.insert);
                    self.pending_revert = Some(Revert {
                        inserted: sub.insert.chars().count(),
                        literal,
                    });
                    return;
                }
            }
        }
        self.doc.insert_char(c);
        self.pending_revert = None;
    }

    fn open_overlay(&mut self, overlay: Overlay) -> Task<Message> {
        self.overlay = overlay;
        match overlay {
            Overlay::Find => self.refresh_matches(),
            Overlay::SaveAs => self.input.clear(),
            Overlay::Rename => self.input = self.filename(),
            Overlay::Goal => {
                self.input = self.goal.map(|g| g.target.to_string()).unwrap_or_default();
            }
            // Deploy has no input; Enter/Esc arrive via the overlay
            // subscription, so the missing-id focus below is a no-op.
            Overlay::Deploy | Overlay::None => {}
        }
        self.chrome_alpha = 1.0;
        iced::widget::operation::focus(CHROME_INPUT_ID)
    }

    fn close_overlay(&mut self) -> Task<Message> {
        self.overlay = Overlay::None;
        Task::none()
    }

    fn refresh_matches(&mut self) {
        self.matches = self.doc.find(&self.input);
        if self.current_match >= self.matches.len() {
            self.current_match = 0;
        }
    }

    /// Select match `idx` in the document; the widget renders and reveals it.
    fn select_match(&mut self, idx: usize) {
        if let Some(range) = self.matches.get(idx).cloned() {
            self.current_match = idx;
            self.doc.set_cursor_pos(range.start, false);
            self.doc.set_cursor_pos(range.end, true);
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
                        let mut parts: Vec<String> = vec![format!("{words} words")];
                        if words > 0 {
                            parts.push(format!("{} min", (words as f32 / 220.0).ceil().max(1.0)));
                        }
                        if let Some(goal) = self.goal {
                            let written = words.saturating_sub(goal.baseline);
                            let done = if written >= goal.target { " ✓" } else { "" };
                            parts.push(format!("session {written}/{}{done}", goal.target));
                        }
                        if self.view_mode == ViewMode::Preview {
                            parts.push("preview".to_string());
                        }
                        if self.typewriter {
                            parts.push("typewriter".to_string());
                        }
                        if self.hemingway {
                            parts.push("hemingway".to_string());
                        }
                        if !self.doc.is_dirty() {
                            parts.push("● saved".to_string());
                        }
                        parts.join(" · ")
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
            Overlay::Rename => row![
                quiet_text("rename".to_string()),
                self.chrome_input("new-name.md"),
            ]
            .spacing(12)
            .into(),
            Overlay::Goal => row![
                quiet_text("session goal".to_string()),
                self.chrome_input("words (empty clears)"),
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
            ViewMode::Write => editor::EditorView::new(
                &self.doc,
                self.text_version,
                self.overlay == Overlay::None,
                self.typewriter,
                self.focus_dim,
                t,
                Message::Editor,
            )
            .into(),
            ViewMode::Preview => {
                let source = self.doc.text();
                scrollable(container(preview::view(&source, t)).padding(Padding {
                    top: 4.0,
                    right: 2.0,
                    bottom: 220.0,
                    left: 2.0,
                }))
                .id(PREVIEW_SCROLL_ID)
                .height(Fill)
                .width(Fill)
                .into()
            }
        };

        // ~62ch at 19px
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
/// while previewing (the editor widget isn't mounted then).
fn preview_key_events(
    event: iced::Event,
    _status: event::Status,
    _window: iced::window::Id,
) -> Option<Message> {
    if let iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event {
        match key.as_ref() {
            keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::TogglePreview),
            keyboard::Key::Character("p") if modifiers.command() => Some(Message::TogglePreview),
            keyboard::Key::Character("t") if modifiers.command() => Some(Message::ToggleTheme),
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

/// A persisted Cmd+T choice wins; otherwise follow the OS.
fn detect_dark() -> bool {
    match crate::config::Config::load().ok().and_then(|c| c.theme) {
        Some(theme) if theme == "dark" => true,
        Some(theme) if theme == "light" => false,
        _ => matches!(dark_light::detect(), Ok(dark_light::Mode::Dark)),
    }
}

#[cfg(test)]
mod tests {
    use super::{editor, App, Message, Overlay, ViewMode};

    fn act(app: &mut App, action: editor::Action) {
        let _ = app.update(Message::Editor(action));
    }

    fn type_into(app: &mut App, s: &str) {
        for c in s.chars() {
            if c == '\n' {
                act(app, editor::Action::Enter);
            } else {
                act(app, editor::Action::Insert(c.to_string()));
            }
        }
    }

    /// The full loop, headless: edit -> debounce -> autosave hits disk.
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
            app.doc.text(),
            "autosave wrote the document"
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
            app.doc.text(),
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
        act(&mut app, editor::Action::Backspace);
        assert!(app.doc.text().starts_with("a--"), "literal restored");
        // A second backspace is a plain backspace again.
        act(&mut app, editor::Action::Backspace);
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
    fn paste_and_cut_roundtrip_through_core() {
        let (mut app, _) = App::boot(None);
        act(
            &mut app,
            editor::Action::Paste("pasted words\n".to_string()),
        );
        assert_eq!(app.doc.text(), "pasted words\n");
        act(&mut app, editor::Action::SelectAll);
        act(&mut app, editor::Action::Cut);
        assert_eq!(app.doc.text(), "");
        act(&mut app, editor::Action::Undo);
        assert_eq!(app.doc.text(), "pasted words\n");
    }

    #[test]
    fn click_drag_and_double_click_select() {
        let (mut app, _) = App::boot(None);
        type_into(&mut app, "hello brave world");
        act(
            &mut app,
            editor::Action::Click {
                position: 0,
                extend: false,
            },
        );
        act(&mut app, editor::Action::DragTo { position: 5 });
        assert_eq!(app.doc.selected_text().as_deref(), Some("hello"));

        act(&mut app, editor::Action::SelectWord { position: 7 });
        assert_eq!(app.doc.selected_text().as_deref(), Some("brave"));
    }

    #[test]
    fn command_routing_reaches_app_keymap() {
        let (mut app, _) = App::boot(None);
        let _ = app.update(Message::Editor(editor::Action::Command("y".to_string())));
        assert!(app.typewriter);
        let _ = app.update(Message::Editor(editor::Action::Command("g".to_string())));
        assert!(app.focus_dim);
        let _ = app.update(Message::Editor(editor::Action::Command("f".to_string())));
        assert_eq!(app.overlay, Overlay::Find);
    }

    #[test]
    fn hemingway_mode_is_forward_only() {
        let (mut app, _) = App::boot(None);
        type_into(&mut app, "first draft");
        let _ = app.update(Message::Editor(editor::Action::Command("e".to_string())));
        assert!(app.hemingway);

        act(&mut app, editor::Action::Backspace);
        act(&mut app, editor::Action::Delete);
        act(&mut app, editor::Action::SelectAll);
        act(&mut app, editor::Action::Cut);
        assert_eq!(app.doc.text(), "first draft", "nothing deletes");

        type_into(&mut app, " continues");
        assert!(app.doc.text().contains("continues"), "typing still works");

        let _ = app.update(Message::Editor(editor::Action::Command("e".to_string())));
        act(&mut app, editor::Action::Backspace);
        assert!(!app.doc.text().contains("continues s"), "deletion restored");
    }

    #[test]
    fn zen_hides_chrome_but_status_and_overlays_summon_it() {
        let (mut app, _) = App::boot(None);
        let _ = app.update(Message::Editor(editor::Action::Command("k".to_string())));
        assert!(app.zen);
        for _ in 0..40 {
            let _ = app.update(Message::FadeTick);
        }
        assert_eq!(app.chrome_alpha, 0.0, "zen drains the chrome");

        app.status = Some("save failed: disk full".to_string());
        for _ in 0..40 {
            let _ = app.update(Message::FadeTick);
        }
        assert_eq!(app.chrome_alpha, 1.0, "status must be visible even in zen");

        app.status = None;
        let _ = app.update(Message::FindOpen);
        assert_eq!(app.chrome_alpha, 1.0, "overlays summon the chrome");
    }

    #[test]
    fn session_goal_sets_counts_and_clears() {
        let (mut app, _) = App::boot(None);
        type_into(&mut app, "one two three");
        let _ = app.update(Message::Editor(editor::Action::Command("l".to_string())));
        assert_eq!(app.overlay, Overlay::Goal);
        let _ = app.update(Message::OverlayInput("5".to_string()));
        let _ = app.update(Message::OverlaySubmit { backwards: false });
        let goal = app.goal.expect("goal set");
        assert_eq!((goal.target, goal.baseline), (5, 3));

        // Non-numeric input keeps the overlay open with a hint.
        let _ = app.update(Message::Editor(editor::Action::Command("l".to_string())));
        assert_eq!(app.input, "5", "prefilled with the current target");
        let _ = app.update(Message::OverlayInput("soon".to_string()));
        let _ = app.update(Message::OverlaySubmit { backwards: false });
        assert_eq!(app.overlay, Overlay::Goal);
        assert!(app.status.as_deref().unwrap_or("").contains("goal"));

        // Empty clears.
        let _ = app.update(Message::OverlayInput(String::new()));
        let _ = app.update(Message::OverlaySubmit { backwards: false });
        assert!(app.goal.is_none());
        assert_eq!(app.overlay, Overlay::None);
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
    fn close_request_flushes_dirty_named_buffer() {
        let dir = std::env::temp_dir().join("polaris-gui-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("close-flush.md");
        std::fs::write(&path, "start\n").unwrap();

        let (mut app, _) = App::boot(Some(path.clone()));
        type_into(&mut app, "last-second words ");
        assert!(app.doc.is_dirty());
        let _ = app.update(Message::CloseRequested(iced::window::Id::unique()));
        assert!(!app.doc.is_dirty(), "flushed before close");
        assert!(std::fs::read_to_string(&path)
            .unwrap()
            .starts_with("last-second words"));
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn close_request_on_untitled_content_warns_once() {
        let (mut app, _) = App::boot(None);
        type_into(&mut app, "not yet saved");
        let id = iced::window::Id::unique();
        let _ = app.update(Message::CloseRequested(id));
        assert_eq!(app.overlay, Overlay::SaveAs, "one chance to name it");
        assert!(app.close_pending);
        // Typing re-arms the warning.
        type_into(&mut app, " more");
        assert!(!app.close_pending);
        // Empty untitled buffers close without ceremony.
        let (mut empty, _) = App::boot(None);
        let _ = empty.update(Message::CloseRequested(id));
        assert_eq!(empty.overlay, Overlay::None);
        assert!(!empty.close_pending);
    }

    #[test]
    fn rename_overlay_prefills_and_renames_in_place() {
        let dir = std::env::temp_dir().join("polaris-gui-test");
        std::fs::create_dir_all(&dir).unwrap();
        let old = dir.join("chapter-one.md");
        let new = dir.join("chapter-uno.md");
        let _ = std::fs::remove_file(&new);
        std::fs::write(&old, "words\n").unwrap();

        let (mut app, _) = App::boot(Some(old.clone()));
        let _ = app.update(Message::RenameOpen);
        assert_eq!(app.overlay, Overlay::Rename);
        assert_eq!(app.input, "chapter-one.md", "prefilled with current name");

        // A bare name renames within the same directory, not the cwd.
        let _ = app.update(Message::OverlayInput("chapter-uno.md".to_string()));
        let _ = app.update(Message::OverlaySubmit { backwards: false });
        assert_eq!(app.overlay, Overlay::None);
        assert!(!old.exists());
        assert!(new.exists());
        assert_eq!(app.filename(), "chapter-uno.md");
        std::fs::remove_file(&new).unwrap();
    }

    #[test]
    fn rename_refuses_overwrite_and_stays_open() {
        let dir = std::env::temp_dir().join("polaris-gui-test");
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("gui-refuse-a.md");
        let b = dir.join("gui-refuse-b.md");
        std::fs::write(&a, "a\n").unwrap();
        std::fs::write(&b, "precious\n").unwrap();

        let (mut app, _) = App::boot(Some(a.clone()));
        let _ = app.update(Message::RenameOpen);
        let _ = app.update(Message::OverlayInput("gui-refuse-b.md".to_string()));
        let _ = app.update(Message::OverlaySubmit { backwards: false });
        assert_eq!(app.overlay, Overlay::Rename, "stays open on failure");
        assert!(app
            .status
            .as_deref()
            .unwrap_or("")
            .contains("rename failed"));
        assert_eq!(std::fs::read_to_string(&b).unwrap(), "precious\n");
        std::fs::remove_file(&a).unwrap();
        std::fs::remove_file(&b).unwrap();
    }

    #[test]
    fn rename_on_untitled_opens_save_as() {
        let (mut app, _) = App::boot(None);
        let _ = app.update(Message::RenameOpen);
        assert_eq!(app.overlay, Overlay::SaveAs);
    }

    #[test]
    fn theme_toggle_flips_and_works_in_preview_too() {
        let (mut app, _) = App::boot(None);
        let initial = app.dark;
        let _ = app.update(Message::ToggleTheme);
        assert_eq!(app.dark, !initial);
        let _ = app.update(Message::TogglePreview);
        let _ = app.update(Message::ToggleTheme);
        assert_eq!(app.dark, initial);
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

        // Selection follows the current match in the document.
        let selected = app.doc.selected_text().map(|s| s.to_lowercase());
        assert_eq!(selected.as_deref(), Some("alpha"));

        let _ = app.update(Message::OverlayClose);
        assert_eq!(app.overlay, Overlay::None);
    }
}
