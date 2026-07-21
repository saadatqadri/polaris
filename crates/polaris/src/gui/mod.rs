//! The iced GUI shell (Phase 1 M2–M5, Phase 2 editor widget).
//!
//! Since the Phase 2 promotion, the editor surface is our own widget
//! ([`editor::EditorView`]) rendering `polaris-core::Document` directly:
//! the Document is the single source of truth, the widget emits
//! [`editor::Action`]s, and there is no sync layer. Typewriter scrolling
//! (Cmd+Y) and focus dimming (Cmd+G) are widget flags.

mod editor;
pub(crate) mod fonts;
mod preview;
mod theme;
mod welcome;

use std::ops::Range;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use iced::widget::{column, container, row, scrollable, space, text, text_input};
use iced::{
    event, keyboard, Background, Border, Element, Fill, Padding, Subscription, Task, Theme,
};

use polaris_core::{typography, AutosaveTimer, Document};
use polaris_drafts::{
    word_diff, Decision, DiffKind, DraftStore, Kind as DraftKind, NoteStore, Review, Segment,
};

const CHROME_INPUT_ID: &str = "chrome-input";
const PREVIEW_SCROLL_ID: &str = "preview-scroll";
const DRAFT_VIEW_SCROLL_ID: &str = "draft-view-scroll";
const DRAFTS_LIST_SCROLL_ID: &str = "drafts-list-scroll";
const REVIEW_SCROLL_ID: &str = "review-scroll";

/// DESIGN.md chrome fade: 0.6s out on keystroke, back 1.2s after rest.
const FADE_OUT_SECS: f32 = 0.6;
const FADE_IN_SECS: f32 = 0.3;
const FADE_REST_MS: u64 = 1200;

pub fn run(path: Option<PathBuf>) -> iced::Result {
    run_with(path, false)
}

/// `polaris welcome`: reopen the tour regardless of the first-run flag.
pub fn run_welcome() -> iced::Result {
    run_with(None, true)
}

fn run_with(path: Option<PathBuf>, force_welcome: bool) -> iced::Result {
    iced::application(
        move || App::boot(path.clone(), force_welcome),
        App::update,
        App::view,
    )
    .title(App::title)
    .theme(App::theme)
    .subscription(App::subscription)
    .font(fonts::WRITING_REGULAR_BYTES)
    .font(fonts::WRITING_ITALIC_BYTES)
    .font(fonts::WRITING_SEMIBOLD_BYTES)
    .font(fonts::MONO_REGULAR_BYTES)
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
    /// Cmd+D target picker (shown only with ≥2 targets): Up/Down select,
    /// Enter publishes, Esc cancels. With one target Cmd+D fires straight
    /// through without this overlay.
    Publish,
    /// Cmd+L: session word goal — a number sets it, empty clears it.
    Goal,
    /// Cmd+M: name and mark a draft (docs/DRAFTS.md).
    Mark,
    /// N in preview: write (or edit) an inline note on the current block.
    Note,
    /// Cmd+Shift+I: path of an edited copy to import for accept/reject review.
    Import,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Write,
    Preview,
    /// The drafts browser (Cmd+Shift+M): named versions + autos.
    Drafts,
    /// Viewing one draft with a word-level diff against current.
    DraftView,
    /// Accept/reject review of an imported edited copy (docs/PHASE4.md P3).
    Review,
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
    /// Preview reading pointer: the block it marks and the source byte offset
    /// of each block (in view order), captured on entering preview so Up/Down
    /// can walk the doc and Cmd+P can round-trip the caret.
    preview_pointer: usize,
    preview_offsets: Vec<usize>,
    /// Inline notes for this document (None while untitled). Notes render in
    /// preview beneath their block; `notes_visible` toggles them (Cmd+Shift+N);
    /// `note_edit_id` is set while editing an existing note rather than adding.
    note_store: Option<NoteStore>,
    notes_visible: bool,
    note_edit_id: Option<String>,
    /// The active accept/reject review (Some only in ViewMode::Review) and the
    /// change the cursor is on.
    review: Option<Review>,
    review_index: usize,
    /// Chrome opacity: fades toward 0 while typing, back to 1 at rest.
    chrome_alpha: f32,
    last_key_ms: Option<u64>,
    pending_revert: Option<Revert>,
    /// The live target list built on Cmd+D (empty when the picker is closed).
    /// Boxed trait objects — consumed when a publish fires.
    publish_targets: Vec<Box<dyn polaris_publish::Target>>,
    publish_selected: usize,
    publishing: bool,
    /// A Hugo overwrite was refused; the next Cmd+D to that file forces it.
    overwrite_armed: bool,
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
    /// The document's snapshot history (None while untitled).
    store: Option<DraftStore>,
    /// Selection in the drafts browser (0 = newest).
    drafts_selected: usize,
    draft_view: Option<DraftView>,
}

/// State for viewing one draft against the current text.
struct DraftView {
    name: String,
    text: String,
    /// false: show the draft (current-only words hidden, draft-only struck);
    /// true: show current (draft restore would strike the current-only words).
    flipped: bool,
}

#[derive(Debug, Clone)]
enum Message {
    Editor(editor::Action),
    Save,
    AutosaveTick,
    FadeTick,
    TogglePreview,
    /// Move the preview reading pointer by ±1 block (Up/Down in preview).
    PreviewPointer(i32),
    /// N in preview: open the note input for the current block (edit if it
    /// already has one).
    NoteOpen,
    /// [ / ] — move the pointer to the previous/next block that has a note.
    NoteJump(i32),
    /// x — resolve/unresolve the current block's note.
    NoteResolve,
    /// Shift+X — delete the current block's note.
    NoteDelete,
    /// Cmd+Shift+N — show/hide notes in preview.
    ToggleNotes,
    ToggleTheme,
    PublishOpen,
    /// Move the target-picker selection (only meaningful while the Publish
    /// overlay is open).
    PublishNav(i32),
    PublishDone(Result<polaris_publish::Outcome, String>),
    FindOpen,
    RenameOpen,
    OverlayInput(String),
    OverlaySubmit {
        backwards: bool,
    },
    OverlayClose,
    CloseRequested(iced::window::Id),
    DraftsNav(i32),
    DraftsOpenSelected,
    DraftsFlip,
    DraftsRestore,
    DraftsBack,
    /// Cmd+Shift+I: open the import-path overlay.
    ImportOpen,
    /// Accept/reject review: move between changes (±1).
    ReviewNav(i32),
    /// Decide the current change: accept, reject, or clear back to pending.
    ReviewAccept,
    ReviewReject,
    ReviewUndo,
    /// Decide every change at once.
    ReviewAcceptAll,
    ReviewRejectAll,
    /// Apply decisions to the buffer (one undo group) / abandon the review.
    ReviewApply,
    ReviewCancel,
    /// Keyboard scrolling for read-only views: (scrollable id, signed px).
    ScrollBy(&'static str, f32),
    /// Snap a read-only view to a relative position (0.0 top, 1.0 bottom).
    Snap(&'static str, f32),
}

impl App {
    fn boot(path: Option<PathBuf>, force_welcome: bool) -> (Self, Task<Message>) {
        // First-ever bare launch (or `polaris welcome`): open the tour as
        // the untitled buffer. Tests never see it (cfg gate).
        let first_run = !cfg!(test)
            && path.is_none()
            && (force_welcome
                || !crate::config::Config::load()
                    .map(|c| c.onboarded)
                    .unwrap_or(true));
        let doc = match &path {
            // Readability is pre-checked in the CLI before `run`.
            Some(p) if p.exists() => Document::open(p).expect("file readable"),
            Some(p) => {
                let mut doc = Document::from_str("");
                doc.save_as(p).expect("file creatable");
                doc
            }
            None if first_run => Document::from_str(welcome::WELCOME),
            None => Document::new(),
        };

        let mut app = Self {
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
            preview_pointer: 0,
            preview_offsets: Vec::new(),
            note_store: None,
            notes_visible: true,
            note_edit_id: None,
            review: None,
            review_index: 0,
            chrome_alpha: 1.0,
            last_key_ms: None,
            pending_revert: None,
            publish_targets: Vec::new(),
            publish_selected: 0,
            publishing: false,
            overwrite_armed: false,
            close_pending: false,
            typewriter: false,
            focus_dim: false,
            hemingway: false,
            zen: false,
            goal: None,
            store: None,
            drafts_selected: 0,
            draft_view: None,
        };
        app.open_store();
        if first_run {
            // The tour is a sample, not user content: one close discards it.
            app.close_pending = true;
            if !force_welcome && !cfg!(test) {
                let mut config = crate::config::Config::load().unwrap_or_default();
                config.onboarded = true;
                let _ = config.save();
            }
        }

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
        // Preview's own keys yield to an open overlay (e.g. the note input),
        // so typing a note isn't swallowed as note commands.
        if self.view_mode == ViewMode::Preview && self.overlay == Overlay::None {
            subs.push(event::listen_with(preview_key_events));
        }
        if self.view_mode == ViewMode::Drafts {
            subs.push(event::listen_with(drafts_list_key_events));
        }
        if self.view_mode == ViewMode::DraftView {
            subs.push(event::listen_with(draft_view_key_events));
        }
        if self.view_mode == ViewMode::Review {
            subs.push(event::listen_with(review_key_events));
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
        let summoned = self.overlay != Overlay::None
            || self.view_mode != ViewMode::Write
            || self.status.is_some();
        let hiding = self.zen
            || self
                .last_key_ms
                .is_some_and(|t| self.now_ms().saturating_sub(t) < FADE_REST_MS);
        if !summoned && hiding {
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

    /// Wall-clock unix millis, for the drafts store (display needs real dates).
    fn unix_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// (Re)open the snapshot store for the current path: prune, then take
    /// the file-open baseline auto (skipped if nothing changed).
    fn open_store(&mut self) {
        self.store = None;
        self.note_store = None;
        let Some(path) = self.doc.path() else { return };
        let path = path.to_path_buf();
        if let Ok(mut store) = DraftStore::for_document(&path) {
            let now = Self::unix_ms();
            let _ = store.prune(now);
            let _ = store.snapshot(&self.doc.text(), DraftKind::Auto, None, now);
            self.store = Some(store);
        }
        if let Ok(mut notes) = NoteStore::for_document(&path) {
            if notes.reanchor(&self.doc.text()) {
                let _ = notes.save();
            }
            self.note_store = Some(notes);
        }
    }

    /// The block index (in preview order) that a source byte offset falls in.
    fn block_of(&self, byte: usize) -> usize {
        self.preview_offsets
            .iter()
            .rposition(|&start| start <= byte)
            .unwrap_or(0)
    }

    /// The `[start, end)` source byte span of preview block `i`.
    fn block_span(&self, i: usize, source: &str) -> (usize, usize) {
        let start = self.preview_offsets.get(i).copied().unwrap_or(0);
        let end = self
            .preview_offsets
            .get(i + 1)
            .copied()
            .unwrap_or(source.len());
        (start, end)
    }

    /// The first note anchored to block `block`, as (id, body).
    fn note_at_block(&self, block: usize) -> Option<(String, String)> {
        let notes = self.note_store.as_ref()?;
        notes
            .notes()
            .iter()
            .find(|n| self.block_of(n.start) == block)
            .map(|n| (n.id.clone(), n.body.clone()))
    }

    /// Sorted, de-duplicated block indices that carry a note — for [/] jumps.
    fn noted_blocks(&self) -> Vec<usize> {
        let mut blocks: Vec<usize> = self
            .note_store
            .as_ref()
            .map(|ns| ns.notes().iter().map(|n| self.block_of(n.start)).collect())
            .unwrap_or_default();
        blocks.sort_unstable();
        blocks.dedup();
        blocks
    }

    /// Notes to draw in preview, each resolved to its block. Empty when notes
    /// are hidden (Cmd+Shift+N) or the document has no note store yet.
    fn note_marks(&self) -> Vec<preview::NoteMark> {
        if !self.notes_visible {
            return Vec::new();
        }
        let Some(notes) = &self.note_store else {
            return Vec::new();
        };
        notes
            .notes()
            .iter()
            .map(|n| preview::NoteMark {
                block: self.block_of(n.start),
                body: n.body.clone(),
                resolved: !n.is_open(),
                detached: n.detached,
            })
            .collect()
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
                    // Map the caret to a block so the pointer starts where you
                    // were writing, and remember every block's source offset.
                    let source = self.doc.text();
                    self.preview_offsets =
                        preview::block_offsets(&source, theme::tokens(self.dark));
                    // Refresh note anchors against the current text before they
                    // render — preview is where notes are seen.
                    if let Some(notes) = self.note_store.as_mut() {
                        if notes.reanchor(&source) {
                            let _ = notes.save();
                        }
                    }
                    let caret_byte = self.doc.buffer().char_to_byte(self.doc.cursor().pos);
                    self.preview_pointer = self
                        .preview_offsets
                        .iter()
                        .rposition(|&start| start <= caret_byte)
                        .unwrap_or(0);
                    self.preview_snap_task()
                }
                ViewMode::Preview => {
                    // Leaving lands the caret where you were reading.
                    if let Some(&byte) = self.preview_offsets.get(self.preview_pointer) {
                        let pos = self.doc.buffer().byte_to_char(byte);
                        self.doc.set_cursor_pos(pos, false);
                    }
                    self.view_mode = ViewMode::Write;
                    Task::none()
                }
                // Preview toggle is inert in the drafts and review views.
                ViewMode::Drafts | ViewMode::DraftView | ViewMode::Review => Task::none(),
            },
            Message::PreviewPointer(delta) => {
                if self.view_mode == ViewMode::Preview && !self.preview_offsets.is_empty() {
                    let last = self.preview_offsets.len() as i32 - 1;
                    self.preview_pointer =
                        (self.preview_pointer as i32 + delta).clamp(0, last) as usize;
                    return self.preview_snap_task();
                }
                Task::none()
            }
            Message::NoteOpen => {
                if self.note_store.is_none() {
                    self.status = Some("save the file to add notes".to_string());
                    return Task::none();
                }
                if self.preview_offsets.is_empty() {
                    return Task::none();
                }
                // Editing the block's note if it has one, else adding.
                match self.note_at_block(self.preview_pointer) {
                    Some((id, body)) => {
                        self.note_edit_id = Some(id);
                        self.input = body;
                    }
                    None => {
                        self.note_edit_id = None;
                        self.input.clear();
                    }
                }
                self.open_overlay(Overlay::Note)
            }
            Message::NoteJump(delta) => {
                let blocks = self.noted_blocks();
                if blocks.is_empty() {
                    return Task::none();
                }
                let cur = self.preview_pointer;
                let target = if delta > 0 {
                    blocks
                        .iter()
                        .copied()
                        .find(|&b| b > cur)
                        .or_else(|| blocks.first().copied())
                } else {
                    blocks
                        .iter()
                        .copied()
                        .rev()
                        .find(|&b| b < cur)
                        .or_else(|| blocks.last().copied())
                };
                if let Some(block) = target {
                    self.preview_pointer = block;
                    return self.preview_snap_task();
                }
                Task::none()
            }
            Message::NoteResolve => {
                if let Some((id, _)) = self.note_at_block(self.preview_pointer) {
                    if let Some(notes) = self.note_store.as_mut() {
                        let _ = notes.toggle_resolved(&id);
                    }
                }
                Task::none()
            }
            Message::NoteDelete => {
                if let Some((id, _)) = self.note_at_block(self.preview_pointer) {
                    if let Some(notes) = self.note_store.as_mut() {
                        let _ = notes.remove(&id);
                    }
                    self.status = Some("· note deleted".to_string());
                }
                Task::none()
            }
            Message::ToggleNotes => {
                self.notes_visible = !self.notes_visible;
                self.status = Some(
                    if self.notes_visible {
                        "· notes shown"
                    } else {
                        "· notes hidden"
                    }
                    .to_string(),
                );
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
            Message::AutosaveTick => {
                if self.doc.is_dirty()
                    && self.doc.path().is_some()
                    && self.autosave.should_save(self.now_ms())
                {
                    self.save_now();
                    let now = Self::unix_ms();
                    if let Some(store) = &mut self.store {
                        if store.should_auto_snapshot(now) {
                            let _ = store.snapshot(&self.doc.text(), DraftKind::Auto, None, now);
                        }
                    }
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
            Message::PublishOpen => {
                if self.publishing {
                    return Task::none();
                }
                if self.doc.path().is_none() {
                    self.status = Some("save before publishing (Cmd+S)".to_string());
                    return Task::none();
                }
                let config = match crate::config::Config::load() {
                    Ok(c) => c,
                    Err(e) => {
                        self.status = Some(format!("config error: {e}"));
                        return Task::none();
                    }
                };
                self.publish_targets = crate::publish::available(&config, self.overwrite_armed);
                match self.publish_targets.len() {
                    0 => {
                        self.status = Some(
                            "no publish targets — configure Notion or add [hugo] (docs/PHASE4.md)"
                                .to_string(),
                        );
                        Task::none()
                    }
                    // One target: one keystroke stays one keystroke.
                    1 => self.start_publish(0),
                    _ => {
                        self.publish_selected =
                            crate::publish::default_id(&config, &self.publish_targets)
                                .and_then(|id| {
                                    self.publish_targets.iter().position(|t| t.id() == id)
                                })
                                .unwrap_or(0);
                        self.open_overlay(Overlay::Publish)
                    }
                }
            }
            Message::PublishNav(delta) => {
                if self.overlay == Overlay::Publish && !self.publish_targets.is_empty() {
                    let last = self.publish_targets.len() as i32 - 1;
                    let next = (self.publish_selected as i32 + delta).clamp(0, last);
                    self.publish_selected = next as usize;
                }
                Task::none()
            }
            Message::PublishDone(result) => {
                self.publishing = false;
                self.status = Some(match result {
                    Ok(polaris_publish::Outcome::Url(url)) => {
                        self.overwrite_armed = false;
                        format!(
                            "✓ published {} → {}",
                            chrono::Local::now().format("%H:%M"),
                            url
                        )
                    }
                    Ok(polaris_publish::Outcome::Path(path)) => {
                        self.overwrite_armed = false;
                        let name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_else(|| path.display().to_string());
                        format!("✓ wrote {} {}", chrono::Local::now().format("%H:%M"), name)
                    }
                    Ok(polaris_publish::Outcome::Clipboard { hint, html, text }) => {
                        self.overwrite_armed = false;
                        match crate::publish::copy_to_clipboard(html.as_deref(), &text) {
                            Ok(()) => format!("✓ copied — {hint}"),
                            Err(e) => format!("clipboard failed: {e}"),
                        }
                    }
                    // Hugo's overwrite guard: arm a forced retry on the next Cmd+D.
                    Err(e) if e.contains("already exists") => {
                        self.overwrite_armed = true;
                        "file exists — Cmd+D again to overwrite".to_string()
                    }
                    Err(e) => {
                        self.overwrite_armed = false;
                        format!("publish failed: {e}")
                    }
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
                            self.open_store();
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
                    let old_path = self.doc.path().map(|p| p.to_path_buf());
                    match self.doc.rename(target) {
                        Ok(()) => {
                            if let (Some(old), Some(new)) = (old_path, self.doc.path()) {
                                let _ = DraftStore::migrate(&old, new);
                            }
                            self.open_store();
                            self.status = None;
                            self.close_overlay()
                        }
                        Err(e) => {
                            self.status = Some(format!("rename failed: {e}"));
                            Task::none()
                        }
                    }
                }
                Overlay::Mark => {
                    let name = self.input.trim().to_string();
                    if name.is_empty() {
                        return Task::none();
                    }
                    self.save_now();
                    if let Some(store) = &mut self.store {
                        match store.snapshot(
                            &self.doc.text(),
                            DraftKind::Marked,
                            Some(name),
                            Self::unix_ms(),
                        ) {
                            Ok(entry) => {
                                // Freeze the live notes with the marked draft so
                                // it carries the critique that was live (never
                                // drifts — the draft's text is frozen).
                                if let (Some(entry), Some(notes)) = (&entry, &self.note_store) {
                                    let _ = notes.freeze_to(&entry.id);
                                }
                                let task = self.close_overlay();
                                self.status = Some("· draft marked".to_string());
                                return task;
                            }
                            Err(e) => {
                                self.status = Some(format!("mark failed: {e}"));
                                return Task::none();
                            }
                        }
                    }
                    Task::none()
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
                Overlay::Publish => self.start_publish(self.publish_selected),
                Overlay::Note => {
                    let body = self.input.trim().to_string();
                    let edit_id = self.note_edit_id.take();
                    // Empty body: delete the note being edited, or cancel a new one.
                    if body.is_empty() {
                        if let (Some(id), Some(notes)) = (edit_id, self.note_store.as_mut()) {
                            let _ = notes.remove(&id);
                        }
                        return self.close_overlay();
                    }
                    let source = self.doc.text();
                    let (start, end) = self.block_span(self.preview_pointer, &source);
                    let quote = anchor_quote(source.get(start..end).unwrap_or(""));
                    if let Some(notes) = self.note_store.as_mut() {
                        let result = match edit_id {
                            Some(id) => notes.edit(&id, body),
                            None => notes
                                .add(start, end, quote, body, Self::unix_ms())
                                .map(|_| ()),
                        };
                        if let Err(e) = result {
                            self.status = Some(format!("note failed: {e}"));
                            return self.close_overlay();
                        }
                    }
                    let task = self.close_overlay();
                    self.status = Some("· note saved".to_string());
                    task
                }
                Overlay::Import => {
                    let raw = self.input.trim();
                    if raw.is_empty() {
                        return self.close_overlay();
                    }
                    let path = expand_tilde_path(raw);
                    match std::fs::read_to_string(&path) {
                        Ok(incoming) => {
                            let review = Review::from_texts(&self.doc.text(), &incoming);
                            if review.is_empty() {
                                let task = self.close_overlay();
                                self.status = Some("no changes to review".to_string());
                                task
                            } else {
                                let task = self.close_overlay();
                                self.review = Some(review);
                                self.review_index = 0;
                                self.view_mode = ViewMode::Review;
                                self.review_status();
                                task
                            }
                        }
                        Err(e) => {
                            self.status = Some(format!("can't read {raw}: {e}"));
                            self.close_overlay()
                        }
                    }
                }
                Overlay::None => Task::none(),
            },
            Message::OverlayClose => self.close_overlay(),
            Message::DraftsNav(delta) => {
                let len = self.store.as_ref().map(|s| s.entries().len()).unwrap_or(0);
                if len > 1 {
                    let cur = self.drafts_selected as i32 + delta;
                    self.drafts_selected = cur.clamp(0, len as i32 - 1) as usize;
                    // Keep the selection in view (relative position is a
                    // good approximation for evenly sized rows).
                    return iced::widget::operation::snap_to(
                        DRAFTS_LIST_SCROLL_ID,
                        scrollable::RelativeOffset {
                            x: 0.0,
                            y: self.drafts_selected as f32 / (len - 1) as f32,
                        },
                    );
                }
                Task::none()
            }
            Message::DraftsOpenSelected => {
                if let Some(store) = &self.store {
                    let entries: Vec<_> = store.entries().iter().rev().collect();
                    if let Some(entry) = entries.get(self.drafts_selected) {
                        if let Ok(text) = store.load(entry) {
                            self.draft_view = Some(DraftView {
                                name: entry
                                    .name
                                    .clone()
                                    .unwrap_or_else(|| "auto snapshot".to_string()),
                                text,
                                flipped: false,
                            });
                            self.view_mode = ViewMode::DraftView;
                        }
                    }
                }
                Task::none()
            }
            Message::DraftsFlip => {
                if let Some(view) = &mut self.draft_view {
                    view.flipped = !view.flipped;
                }
                Task::none()
            }
            Message::DraftsRestore => {
                if let Some(view) = self.draft_view.take() {
                    // Restore can never lose words: snapshot current first.
                    if let Some(store) = &mut self.store {
                        let _ = store.snapshot(
                            &self.doc.text(),
                            DraftKind::Auto,
                            None,
                            Self::unix_ms(),
                        );
                    }
                    // One atomic undo group: select-all + insert.
                    self.doc.select_all();
                    self.doc.insert_str(&view.text);
                    self.pending_revert = None;
                    self.note_edit();
                    self.view_mode = ViewMode::Write;
                    self.status = Some("restored — Cmd+Z undoes".to_string());
                }
                Task::none()
            }
            Message::DraftsBack => {
                self.view_mode = match self.view_mode {
                    ViewMode::DraftView => {
                        self.draft_view = None;
                        ViewMode::Drafts
                    }
                    _ => ViewMode::Write,
                };
                Task::none()
            }
            Message::ImportOpen => self.open_overlay(Overlay::Import),
            Message::ReviewNav(delta) => {
                let Some(review) = &self.review else {
                    return Task::none();
                };
                let last = review.change_count().saturating_sub(1) as i32;
                self.review_index = (self.review_index as i32 + delta).clamp(0, last) as usize;
                self.review_status();
                self.review_snap_task()
            }
            Message::ReviewAccept => self.decide_current(Decision::Accepted),
            Message::ReviewReject => self.decide_current(Decision::Rejected),
            Message::ReviewUndo => self.decide_current(Decision::Pending),
            Message::ReviewAcceptAll => {
                if let Some(review) = &mut self.review {
                    review.set_all(Decision::Accepted);
                    self.review_status();
                }
                Task::none()
            }
            Message::ReviewRejectAll => {
                if let Some(review) = &mut self.review {
                    review.set_all(Decision::Rejected);
                    self.review_status();
                }
                Task::none()
            }
            Message::ReviewApply => {
                if let Some(review) = self.review.take() {
                    // Recoverable: snapshot current before replacing (as restore does).
                    if let Some(store) = &mut self.store {
                        let _ = store.snapshot(
                            &self.doc.text(),
                            DraftKind::Auto,
                            None,
                            Self::unix_ms(),
                        );
                    }
                    let accepted = review.accepted_count();
                    // One atomic undo group: select-all + insert the result.
                    self.doc.select_all();
                    self.doc.insert_str(&review.applied());
                    self.pending_revert = None;
                    self.note_edit();
                    self.view_mode = ViewMode::Write;
                    self.status = Some(format!(
                        "applied {accepted} change{} — Cmd+Z undoes",
                        if accepted == 1 { "" } else { "s" }
                    ));
                }
                Task::none()
            }
            Message::ReviewCancel => {
                self.review = None;
                self.view_mode = ViewMode::Write;
                self.status = Some("review cancelled".to_string());
                Task::none()
            }
            Message::ScrollBy(id, dy) => {
                iced::widget::operation::scroll_by(id, scrollable::AbsoluteOffset { x: 0.0, y: dy })
            }
            Message::Snap(id, y) => {
                iced::widget::operation::snap_to(id, scrollable::RelativeOffset { x: 0.0, y })
            }
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
            A::Backspace
            | A::Delete
            | A::DeleteWordBack
            | A::DeleteWordForward
            | A::DeleteToLineStart
            | A::DeleteToLineEnd
            | A::Cut
                if self.hemingway => {}
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
            // Range deletions: select, then delete the selection (one undo
            // group, and a no-op when the range is empty).
            A::DeleteWordBack => {
                self.pending_revert = None;
                if self.doc.selection().is_none() {
                    self.doc.move_word_left(true);
                }
                self.doc.backspace();
                self.note_edit();
            }
            A::DeleteWordForward => {
                self.pending_revert = None;
                if self.doc.selection().is_none() {
                    self.doc.move_word_right(true);
                }
                self.doc.backspace();
                self.note_edit();
            }
            A::DeleteToLineStart => {
                self.pending_revert = None;
                if self.doc.selection().is_none() {
                    self.doc.move_line_start(true);
                }
                self.doc.backspace();
                self.note_edit();
            }
            A::DeleteToLineEnd => {
                self.pending_revert = None;
                if self.doc.selection().is_none() {
                    self.doc.move_line_end(true);
                }
                self.doc.backspace();
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
                    M::DocStart => self.doc.set_cursor_pos(0, extend),
                    M::DocEnd => {
                        let end = self.doc.buffer().len_chars();
                        self.doc.set_cursor_pos(end, extend);
                    }
                }
            }
            A::VerticalMove { target, extend } => {
                self.pending_revert = None;
                self.doc.set_cursor_pos(target, extend);
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
            A::Command { key, shift } => return self.on_command(&key, shift),
        }
        Task::none()
    }

    /// The app-level keymap, reached through the editor widget's unclaimed
    /// Cmd/Ctrl shortcuts.
    fn on_command(&mut self, key: &str, shift: bool) -> Task<Message> {
        match key {
            "m" if shift => {
                if self.store.is_some() {
                    self.drafts_selected = 0;
                    self.view_mode = ViewMode::Drafts;
                } else {
                    self.status = Some("save the file to start keeping drafts".to_string());
                }
                Task::none()
            }
            "m" => {
                if self.store.is_some() {
                    self.open_overlay(Overlay::Mark)
                } else {
                    self.status = Some("save the file to start keeping drafts".to_string());
                    Task::none()
                }
            }
            "s" => self.update(Message::Save),
            "f" => self.update(Message::FindOpen),
            "r" => self.update(Message::RenameOpen),
            "p" => self.update(Message::TogglePreview),
            "t" => self.update(Message::ToggleTheme),
            "d" => self.update(Message::PublishOpen),
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
            "i" if shift => self.update(Message::ImportOpen),
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
            {
                if let Some(sub) = typography::substitute_in_context(&before, c) {
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
            Overlay::Mark => {
                let n = self.store.as_ref().map(|s| s.marked_count()).unwrap_or(0);
                self.input = format!("Draft {}", n + 1);
            }
            Overlay::Import => self.input.clear(),
            // The note input's text is set by NoteOpen (edit prefill or empty);
            // leave it and let the focus below land in it.
            Overlay::Note => {}
            // The publish picker has no text input; Up/Down/Enter/Esc arrive
            // via the overlay subscription, so the focus below is a no-op.
            Overlay::Publish | Overlay::None => {}
        }
        self.chrome_alpha = 1.0;
        iced::widget::operation::focus(CHROME_INPUT_ID)
    }

    fn close_overlay(&mut self) -> Task<Message> {
        self.overlay = Overlay::None;
        self.note_edit_id = None;
        Task::none()
    }

    /// Scroll preview so the pointed block is roughly in view — the same
    /// caret-ratio approximation preview has always used, keyed to the
    /// pointer's block index rather than a raw line.
    fn preview_snap_task(&self) -> Task<Message> {
        let len = self.preview_offsets.len();
        let y = if len > 1 {
            (self.preview_pointer as f32 / (len - 1) as f32).clamp(0.0, 1.0)
        } else {
            0.0
        };
        iced::widget::operation::snap_to(
            PREVIEW_SCROLL_ID,
            scrollable::RelativeOffset { x: 0.0, y },
        )
    }

    /// Decide the current change and advance to the next one.
    fn decide_current(&mut self, decision: Decision) -> Task<Message> {
        let Some(review) = &mut self.review else {
            return Task::none();
        };
        review.set_decision(self.review_index, decision);
        let last = review.change_count().saturating_sub(1);
        if self.review_index < last {
            self.review_index += 1;
        }
        self.review_status();
        self.review_snap_task()
    }

    /// The chrome counter shown throughout a review.
    fn review_status(&mut self) {
        if let Some(review) = &self.review {
            let count = review.change_count();
            self.status = Some(format!(
                "change {}/{} · {} accepted — A accept · R reject · U undo · Enter apply · Esc cancel",
                (self.review_index + 1).min(count),
                count,
                review.accepted_count(),
            ));
        }
    }

    /// Keep the current change roughly in view (caret-ratio approximation).
    fn review_snap_task(&self) -> Task<Message> {
        let count = self.review.as_ref().map(|r| r.change_count()).unwrap_or(0);
        let y = if count > 1 {
            (self.review_index as f32 / (count - 1) as f32).clamp(0.0, 1.0)
        } else {
            0.0
        };
        iced::widget::operation::snap_to(REVIEW_SCROLL_ID, scrollable::RelativeOffset { x: 0.0, y })
    }

    /// Save, then publish the target at `index` in `publish_targets`,
    /// consuming the built list. Reached by Cmd+D fire-through (one target)
    /// or the picker's Enter.
    fn start_publish(&mut self, index: usize) -> Task<Message> {
        let targets = std::mem::take(&mut self.publish_targets);
        let Some(target) = targets.into_iter().nth(index) else {
            return self.close_overlay();
        };
        self.save_now();
        let markdown = self.doc.text();
        let path = self.doc.path().map(|p| p.to_path_buf());
        self.publishing = true;
        self.status = Some(format!("publishing to {}…", target.label()));
        let close = self.close_overlay();
        Task::batch([
            close,
            Task::perform(
                async move {
                    target
                        .publish(polaris_publish::Doc::new(&markdown, path.as_deref()))
                        .await
                        .map_err(|e| e.to_string())
                },
                Message::PublishDone,
            ),
        ])
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
                        if self.view_mode == ViewMode::Drafts {
                            parts.push("drafts · Enter view · Esc back".to_string());
                        }
                        if self.view_mode == ViewMode::DraftView {
                            if let Some(view) = &self.draft_view {
                                parts.push(format!(
                                    "{} · R restore · Tab flip · Esc back",
                                    view.name
                                ));
                            }
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
            Overlay::Mark => row![
                quiet_text("mark draft".to_string()),
                self.chrome_input("a name for this version"),
            ]
            .spacing(12)
            .into(),
            Overlay::Note => row![
                quiet_text(
                    if self.note_edit_id.is_some() {
                        "edit note"
                    } else {
                        "note"
                    }
                    .to_string()
                ),
                self.chrome_input("a note to yourself (empty deletes)"),
            ]
            .spacing(12)
            .into(),
            Overlay::Import => row![
                quiet_text("import edited copy".to_string()),
                self.chrome_input("path to the edited .md"),
            ]
            .spacing(12)
            .into(),
            Overlay::Publish => {
                let mut list =
                    column![text("publish to").font(fonts::MONO).size(13).color(t.star)].spacing(6);
                for (i, target) in self.publish_targets.iter().enumerate() {
                    let label = target.label();
                    if i == self.publish_selected {
                        list = list.push(
                            text(format!("✧ {label}"))
                                .font(fonts::MONO)
                                .size(13)
                                .color(t.star),
                        );
                    } else {
                        list = list.push(quiet_text(format!("   {label}")));
                    }
                }
                list.push(quiet_text(
                    "↑↓ select · Enter publish · Esc cancel".to_string(),
                ))
                .into()
            }
        };

        let outer_style = move |_: &Theme| container::Style {
            background: Some(Background::Color(t.bg)),
            ..container::Style::default()
        };

        match self.view_mode {
            ViewMode::Drafts => {
                let list = self.drafts_list_view(t);
                let top = container(container(chrome).max_width(600)).center_x(Fill);
                container(column![top, list].spacing(26))
                    .style(outer_style)
                    .width(Fill)
                    .height(Fill)
                    .padding(Padding {
                        top: 76.0,
                        right: 8.0,
                        bottom: 0.0,
                        left: 8.0,
                    })
                    .into()
            }
            ViewMode::DraftView => {
                let body = self.draft_view_body(t);
                let top = container(container(chrome).max_width(600)).center_x(Fill);
                container(column![top, body].spacing(26))
                    .style(outer_style)
                    .width(Fill)
                    .height(Fill)
                    .padding(Padding {
                        top: 76.0,
                        right: 8.0,
                        bottom: 0.0,
                        left: 8.0,
                    })
                    .into()
            }
            ViewMode::Review => {
                let body = self.review_body(t);
                let top = container(container(chrome).max_width(600)).center_x(Fill);
                container(column![top, body].spacing(26))
                    .style(outer_style)
                    .width(Fill)
                    .height(Fill)
                    .padding(Padding {
                        top: 76.0,
                        right: 8.0,
                        bottom: 0.0,
                        left: 8.0,
                    })
                    .into()
            }
            ViewMode::Write => {
                let body: Element<'_, Message> = editor::EditorView::new(
                    &self.doc,
                    self.text_version,
                    self.overlay == Overlay::None,
                    self.typewriter,
                    self.focus_dim,
                    t,
                    Message::Editor,
                )
                .into();

                // ~62ch at 19px
                let page = container(column![chrome, body].spacing(26))
                    .max_width(600)
                    .height(Fill);

                container(page)
                    .style(outer_style)
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
            ViewMode::Preview => {
                // The scrollable spans the window (scrollbar at the window
                // edge, clear of the text); the column centers inside it.
                let source = self.doc.text();
                let notes = self.note_marks();
                let base = self.doc.path().and_then(|p| p.parent());
                let column_content = container(preview::view(
                    &source,
                    t,
                    Some(self.preview_pointer),
                    &notes,
                    base,
                ))
                .max_width(600)
                .padding(Padding {
                    top: 4.0,
                    right: 2.0,
                    bottom: 220.0,
                    left: 2.0,
                });
                let scroll = scrollable(container(column_content).center_x(Fill))
                    .id(PREVIEW_SCROLL_ID)
                    .width(Fill)
                    .height(Fill)
                    .direction(scrollable::Direction::Vertical(
                        scrollable::Scrollbar::new()
                            .width(6)
                            .margin(4)
                            .scroller_width(6),
                    ))
                    .style(move |theme: &Theme, status| {
                        let mut style = scrollable::default(theme, status);
                        style.container = container::Style::default();
                        style.vertical_rail.background = None;
                        style.vertical_rail.border = iced::Border::default();
                        style.vertical_rail.scroller.background = Background::Color(
                            if matches!(status, scrollable::Status::Active { .. }) {
                                t.whisper
                            } else {
                                t.quiet
                            },
                        );
                        style.vertical_rail.scroller.border = iced::Border {
                            radius: 3.0.into(),
                            ..iced::Border::default()
                        };
                        style
                    });

                let top = container(container(chrome).max_width(600)).center_x(Fill);
                container(column![top, scroll].spacing(26))
                    .style(outer_style)
                    .width(Fill)
                    .height(Fill)
                    .padding(Padding {
                        top: 76.0,
                        right: 8.0,
                        bottom: 0.0,
                        left: 8.0,
                    })
                    .into()
            }
        }
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

impl App {
    /// The drafts browser: newest first, marked drafts prominent.
    fn drafts_list_view(&self, t: theme::Tokens) -> Element<'_, Message> {
        let now = Self::unix_ms();
        let Some(store) = &self.store else {
            return container(text("no drafts yet").color(t.quiet)).into();
        };
        let entries: Vec<_> = store.entries().iter().rev().collect();
        if entries.is_empty() {
            return container(
                text("no drafts yet — Cmd+M marks one")
                    .font(fonts::MONO)
                    .size(13)
                    .color(t.quiet),
            )
            .center_x(Fill)
            .into();
        }

        let mut rows: Vec<Element<'_, Message>> = Vec::new();
        for (i, entry) in entries.iter().enumerate() {
            let selected = i == self.drafts_selected;
            let marker: Element<'_, Message> = if selected {
                text("▍").color(t.star).size(16).into()
            } else {
                space().width(12).into()
            };
            let name: Element<'_, Message> = match entry.kind {
                polaris_drafts::Kind::Marked => {
                    text(entry.name.clone().unwrap_or_else(|| "draft".to_string()))
                        .font(iced::Font {
                            weight: iced::font::Weight::Semibold,
                            ..fonts::WRITING
                        })
                        .size(17)
                        .color(t.ink)
                        .into()
                }
                polaris_drafts::Kind::Auto => text("auto snapshot").size(15).color(t.quiet).into(),
            };
            // Delta vs the previous entry (chronological), per DRAFTS.md.
            let idx = entries.len() - 1 - i;
            let delta = if idx > 0 {
                let prev = store.entries()[idx - 1].words as i64;
                let d = entry.words as i64 - prev;
                if d != 0 {
                    format!(" · {}{d}", if d > 0 { "+" } else { "" })
                } else {
                    String::new()
                }
            } else {
                String::new()
            };
            let meta = text(format!(
                "{} · {} words{delta}",
                ago(now, entry.created_ms),
                entry.words
            ))
            .font(fonts::MONO)
            .size(12)
            .color(t.quiet);

            rows.push(
                row![marker, name, space().width(Fill), meta]
                    .spacing(10)
                    .into(),
            );
        }

        let list = container(column(rows).spacing(14)).max_width(600);
        scrollable(container(list).center_x(Fill))
            .id(DRAFTS_LIST_SCROLL_ID)
            .width(Fill)
            .height(Fill)
            .into()
    }

    /// One draft, word-diffed against the current text.
    fn draft_view_body(&self, t: theme::Tokens) -> Element<'_, Message> {
        let Some(view) = &self.draft_view else {
            return space().into();
        };
        let current = self.doc.text();
        let spans = word_diff(&view.text, &current);
        let mut rich: Vec<iced::widget::text::Span<'_>> = Vec::new();
        for span in &spans {
            match (span.kind, view.flipped) {
                (DiffKind::Equal, _) => {
                    rich.push(iced::widget::text::Span::new(span.text.clone()));
                }
                // Viewing the draft: draft-only words struck in quiet.
                (DiffKind::Removed, false) => rich.push(
                    iced::widget::text::Span::new(span.text.clone())
                        .color(t.quiet)
                        .strikethrough(true),
                ),
                // Flipped (viewing current): what a restore would remove.
                (DiffKind::Added, true) => rich.push(
                    iced::widget::text::Span::new(span.text.clone())
                        .color(t.quiet)
                        .strikethrough(true),
                ),
                _ => {}
            }
        }
        let body = iced::widget::rich_text(rich)
            .font(fonts::WRITING)
            .size(19)
            .line_height(text::LineHeight::Relative(1.56))
            .color(t.ink);
        let column_content = container(body).max_width(600).padding(Padding {
            top: 4.0,
            right: 2.0,
            bottom: 220.0,
            left: 2.0,
        });
        scrollable(container(column_content).center_x(Fill))
            .id(DRAFT_VIEW_SCROLL_ID)
            .width(Fill)
            .height(Fill)
            .into()
    }

    /// The imported edited copy as an inline diff: deletions struck through,
    /// insertions in the accent, rejected changes shown as kept-current /
    /// discarded-incoming, and the change under the cursor in semibold.
    fn review_body(&self, t: theme::Tokens) -> Element<'_, Message> {
        use iced::widget::text::Span;
        let Some(review) = &self.review else {
            return space().into();
        };
        let bold = iced::Font {
            weight: iced::font::Weight::Semibold,
            ..fonts::WRITING
        };
        let mut rich: Vec<Span<'_>> = Vec::new();
        for segment in review.segments() {
            match segment {
                Segment::Context(text) => rich.push(Span::new(text.to_string()).color(t.ink)),
                Segment::Change { index, change } => {
                    let current = index == self.review_index;
                    // Deletion side (current document's words).
                    if !change.old.is_empty() {
                        let mut span = Span::new(change.old.clone());
                        span = if change.decision == Decision::Rejected {
                            span.color(t.ink) // kept
                        } else {
                            span.color(if current { t.quiet } else { t.whisper })
                                .strikethrough(true)
                        };
                        if current {
                            span = span.font(bold);
                        }
                        rich.push(span);
                    }
                    // Insertion side (incoming words).
                    if !change.new.is_empty() {
                        let mut span = Span::new(change.new.clone());
                        span = if change.decision == Decision::Rejected {
                            span.color(t.whisper).strikethrough(true) // discarded
                        } else {
                            span.color(t.star) // proposed or accepted
                        };
                        if current {
                            span = span.font(bold);
                        }
                        rich.push(span);
                    }
                }
            }
        }
        let body = iced::widget::rich_text(rich)
            .font(fonts::WRITING)
            .size(19)
            .line_height(text::LineHeight::Relative(1.56))
            .color(t.ink);
        let column_content = container(body).max_width(600).padding(Padding {
            top: 4.0,
            right: 2.0,
            bottom: 220.0,
            left: 2.0,
        });
        scrollable(container(column_content).center_x(Fill))
            .id(REVIEW_SCROLL_ID)
            .width(Fill)
            .height(Fill)
            .into()
    }
}

/// Expand a leading `~/` in a user-typed path against the home directory.
fn expand_tilde_path(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

/// The anchor quote for a note on a block: its first non-empty source line,
/// trimmed and capped. A prefix of the block, so it re-anchors by exact match
/// yet is resilient to edits later in the block. Design: PHASE4.md #7.
fn anchor_quote(block_src: &str) -> String {
    block_src
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("")
        .trim()
        .chars()
        .take(120)
        .collect()
}

/// "just now" / "5 min ago" / "3 h ago" / "2 days ago".
fn ago(now_ms: u64, then_ms: u64) -> String {
    let secs = now_ms.saturating_sub(then_ms) / 1000;
    match secs {
        0..=59 => "just now".to_string(),
        60..=3599 => format!("{} min ago", secs / 60),
        3600..=86_399 => format!("{} h ago", secs / 3600),
        _ => format!("{} days ago", secs / 86_400),
    }
}

/// Up/Down/Enter/Esc in the drafts browser.
fn drafts_list_key_events(
    event: iced::Event,
    _status: event::Status,
    _window: iced::window::Id,
) -> Option<Message> {
    if let iced::Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) = event {
        match key.as_ref() {
            keyboard::Key::Named(keyboard::key::Named::ArrowUp) => Some(Message::DraftsNav(-1)),
            keyboard::Key::Named(keyboard::key::Named::ArrowDown) => Some(Message::DraftsNav(1)),
            keyboard::Key::Named(keyboard::key::Named::Enter) => Some(Message::DraftsOpenSelected),
            keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::DraftsBack),
            _ => None,
        }
    } else {
        None
    }
}

/// R restore / Tab flip / Esc back while viewing a draft.
fn draft_view_key_events(
    event: iced::Event,
    _status: event::Status,
    _window: iced::window::Id,
) -> Option<Message> {
    if let iced::Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) = event {
        match key.as_ref() {
            keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::DraftsBack),
            keyboard::Key::Named(keyboard::key::Named::Tab) => Some(Message::DraftsFlip),
            keyboard::Key::Character("r") => Some(Message::DraftsRestore),
            keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                Some(Message::ScrollBy(DRAFT_VIEW_SCROLL_ID, -60.0))
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                Some(Message::ScrollBy(DRAFT_VIEW_SCROLL_ID, 60.0))
            }
            keyboard::Key::Named(keyboard::key::Named::PageUp) => {
                Some(Message::ScrollBy(DRAFT_VIEW_SCROLL_ID, -600.0))
            }
            keyboard::Key::Named(keyboard::key::Named::PageDown) => {
                Some(Message::ScrollBy(DRAFT_VIEW_SCROLL_ID, 600.0))
            }
            keyboard::Key::Named(keyboard::key::Named::Home) => {
                Some(Message::Snap(DRAFT_VIEW_SCROLL_ID, 0.0))
            }
            keyboard::Key::Named(keyboard::key::Named::End) => {
                Some(Message::Snap(DRAFT_VIEW_SCROLL_ID, 1.0))
            }
            _ => None,
        }
    } else {
        None
    }
}

/// Accept/reject review: J/K (or arrows) move between changes, A/R decide, U
/// clears to pending, Shift+A/Shift+R decide all, Enter applies, Esc cancels.
fn review_key_events(
    event: iced::Event,
    _status: event::Status,
    _window: iced::window::Id,
) -> Option<Message> {
    if let iced::Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) = event {
        match key.as_ref() {
            keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::ReviewCancel),
            keyboard::Key::Named(keyboard::key::Named::Enter) => Some(Message::ReviewApply),
            keyboard::Key::Named(keyboard::key::Named::ArrowDown)
            | keyboard::Key::Character("j") => Some(Message::ReviewNav(1)),
            keyboard::Key::Named(keyboard::key::Named::ArrowUp) | keyboard::Key::Character("k") => {
                Some(Message::ReviewNav(-1))
            }
            keyboard::Key::Character("a") => Some(Message::ReviewAccept),
            keyboard::Key::Character("r") => Some(Message::ReviewReject),
            keyboard::Key::Character("u") => Some(Message::ReviewUndo),
            keyboard::Key::Character("A") => Some(Message::ReviewAcceptAll),
            keyboard::Key::Character("R") => Some(Message::ReviewRejectAll),
            _ => None,
        }
    } else {
        None
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
            // Only the publish picker acts on these; other overlays ignore them.
            keyboard::Key::Named(keyboard::key::Named::ArrowUp) => Some(Message::PublishNav(-1)),
            keyboard::Key::Named(keyboard::key::Named::ArrowDown) => Some(Message::PublishNav(1)),
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
            // Cmd+Shift+N shows/hides notes (must precede the bare-n note key).
            keyboard::Key::Character("n" | "N") if modifiers.command() => {
                Some(Message::ToggleNotes)
            }
            // Notes: n adds/edits on the current block, [/] jump between notes,
            // x resolves, Shift+X deletes.
            keyboard::Key::Character("n") => Some(Message::NoteOpen),
            keyboard::Key::Character("[") => Some(Message::NoteJump(-1)),
            keyboard::Key::Character("]") => Some(Message::NoteJump(1)),
            keyboard::Key::Character("x") => Some(Message::NoteResolve),
            keyboard::Key::Character("X") => Some(Message::NoteDelete),
            // Up/Down walk the reading pointer; PageUp/Down still free-scroll.
            keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                Some(Message::PreviewPointer(-1))
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                Some(Message::PreviewPointer(1))
            }
            keyboard::Key::Named(keyboard::key::Named::PageUp) => {
                Some(Message::ScrollBy(PREVIEW_SCROLL_ID, -600.0))
            }
            keyboard::Key::Named(keyboard::key::Named::PageDown) => {
                Some(Message::ScrollBy(PREVIEW_SCROLL_ID, 600.0))
            }
            keyboard::Key::Named(keyboard::key::Named::Home) => {
                Some(Message::Snap(PREVIEW_SCROLL_ID, 0.0))
            }
            keyboard::Key::Named(keyboard::key::Named::End) => {
                Some(Message::Snap(PREVIEW_SCROLL_ID, 1.0))
            }
            _ => None,
        }
    } else {
        None
    }
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

        let (mut app, _) = App::boot(Some(path.clone()), false);
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

        let (mut app, _) = App::boot(None, false);
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
        let (mut app, _) = App::boot(None, false);
        type_into(&mut app, "wait -- \"really\" it's...");
        assert_eq!(
            app.doc.text(),
            "wait \u{2014} \u{201C}really\u{201D} it\u{2019}s\u{2026}"
        );
    }

    #[test]
    fn smart_punctuation_skipped_in_code_contexts() {
        let (mut app, _) = App::boot(None, false);
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
        let (mut app, _) = App::boot(None, false);
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
        let (mut app, _) = App::boot(None, false);
        type_into(&mut app, "text\n\n---");
        assert!(
            app.doc.text().contains("\n---"),
            "hr not turned into a dash"
        );
    }

    #[test]
    fn paste_and_cut_roundtrip_through_core() {
        let (mut app, _) = App::boot(None, false);
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
        let (mut app, _) = App::boot(None, false);
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
    fn navigation_actions_route_through_core() {
        let (mut app, _) = App::boot(None, false);
        type_into(&mut app, "hello brave world");

        // Word jump (Option+arrow) advances by a word.
        act(
            &mut app,
            editor::Action::Move(editor::Motion::DocStart, false),
        );
        assert_eq!(app.doc.cursor().pos, 0);
        act(
            &mut app,
            editor::Action::Move(editor::Motion::WordRight, false),
        );
        assert_eq!(app.doc.cursor().pos, 5, "end of 'hello'");

        // Cmd+arrow extremes.
        act(
            &mut app,
            editor::Action::Move(editor::Motion::DocEnd, false),
        );
        assert_eq!(app.doc.cursor().pos, app.doc.text().chars().count());
        act(
            &mut app,
            editor::Action::Move(editor::Motion::DocStart, true),
        );
        assert_eq!(
            app.doc.selected_text().as_deref(),
            Some("hello brave world")
        );

        // VerticalMove is a widget-resolved position; the app just applies it.
        act(
            &mut app,
            editor::Action::VerticalMove {
                target: 6,
                extend: false,
            },
        );
        assert_eq!(app.doc.cursor().pos, 6);
        assert_eq!(app.doc.selection(), None);
    }

    #[test]
    fn command_routing_reaches_app_keymap() {
        let (mut app, _) = App::boot(None, false);
        let _ = app.update(Message::Editor(editor::Action::Command {
            key: "y".to_string(),
            shift: false,
        }));
        assert!(app.typewriter);
        let _ = app.update(Message::Editor(editor::Action::Command {
            key: "g".to_string(),
            shift: false,
        }));
        assert!(app.focus_dim);
        let _ = app.update(Message::Editor(editor::Action::Command {
            key: "f".to_string(),
            shift: false,
        }));
        assert_eq!(app.overlay, Overlay::Find);
    }

    #[test]
    fn word_and_line_deletions() {
        let (mut app, _) = App::boot(None, false);
        type_into(&mut app, "hello brave new world");
        // Caret at end; delete a word back.
        act(&mut app, editor::Action::DeleteWordBack);
        assert_eq!(app.doc.text(), "hello brave new ");
        act(&mut app, editor::Action::Undo);
        assert_eq!(app.doc.text(), "hello brave new world", "one undo");

        // Delete to line start from the end wipes the line.
        act(
            &mut app,
            editor::Action::Move(editor::Motion::DocEnd, false),
        );
        act(&mut app, editor::Action::DeleteToLineStart);
        assert_eq!(app.doc.text(), "");

        // Forward word delete from the start.
        type_into(&mut app, "one two three");
        act(
            &mut app,
            editor::Action::Move(editor::Motion::DocStart, false),
        );
        act(&mut app, editor::Action::DeleteWordForward);
        assert_eq!(app.doc.text(), " two three");
    }

    #[test]
    fn hemingway_mode_is_forward_only() {
        let (mut app, _) = App::boot(None, false);
        type_into(&mut app, "first draft");
        let _ = app.update(Message::Editor(editor::Action::Command {
            key: "e".to_string(),
            shift: false,
        }));
        assert!(app.hemingway);

        act(&mut app, editor::Action::Backspace);
        act(&mut app, editor::Action::Delete);
        act(&mut app, editor::Action::DeleteWordBack);
        act(&mut app, editor::Action::DeleteToLineStart);
        act(&mut app, editor::Action::SelectAll);
        act(&mut app, editor::Action::Cut);
        assert_eq!(app.doc.text(), "first draft", "nothing deletes");

        type_into(&mut app, " continues");
        assert!(app.doc.text().contains("continues"), "typing still works");

        let _ = app.update(Message::Editor(editor::Action::Command {
            key: "e".to_string(),
            shift: false,
        }));
        act(&mut app, editor::Action::Backspace);
        assert!(!app.doc.text().contains("continues s"), "deletion restored");
    }

    #[test]
    fn zen_hides_chrome_but_status_and_overlays_summon_it() {
        let (mut app, _) = App::boot(None, false);
        let _ = app.update(Message::Editor(editor::Action::Command {
            key: "k".to_string(),
            shift: false,
        }));
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
        let (mut app, _) = App::boot(None, false);
        type_into(&mut app, "one two three");
        let _ = app.update(Message::Editor(editor::Action::Command {
            key: "l".to_string(),
            shift: false,
        }));
        assert_eq!(app.overlay, Overlay::Goal);
        let _ = app.update(Message::OverlayInput("5".to_string()));
        let _ = app.update(Message::OverlaySubmit { backwards: false });
        let goal = app.goal.expect("goal set");
        assert_eq!((goal.target, goal.baseline), (5, 3));

        // Non-numeric input keeps the overlay open with a hint.
        let _ = app.update(Message::Editor(editor::Action::Command {
            key: "l".to_string(),
            shift: false,
        }));
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
        let (mut app, _) = App::boot(None, false);
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
    fn preview_pointer_maps_caret_to_block_and_round_trips_it() {
        let (mut app, _) = App::boot(None, false);
        let text = "# Title\n\nAlpha para.\n\nBeta para.\n\nGamma para.\n";
        type_into(&mut app, text);
        // Caret into "Beta para." (ASCII here, so byte == char index).
        let beta = text.find("Beta").unwrap();
        app.doc.set_cursor_pos(beta, false);

        // Entering preview starts the pointer on the caret's block: blocks are
        // heading(0), Alpha(1), Beta(2), Gamma(3).
        let _ = app.update(Message::TogglePreview);
        assert_eq!(app.view_mode, ViewMode::Preview);
        assert_eq!(app.preview_pointer, 2);

        // Up/Down clamp within the block list.
        let _ = app.update(Message::PreviewPointer(1));
        assert_eq!(app.preview_pointer, 3);
        let _ = app.update(Message::PreviewPointer(1));
        assert_eq!(app.preview_pointer, 3, "clamps at the last block");

        // Leaving lands the caret where the pointer was reading.
        let _ = app.update(Message::TogglePreview);
        assert_eq!(app.view_mode, ViewMode::Write);
        assert_eq!(app.doc.cursor().pos, text.find("Gamma").unwrap());
    }

    #[test]
    fn notes_add_render_resolve_jump_and_delete_in_preview() {
        let dir = std::env::temp_dir().join("polaris-gui-notes");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("notes.md");
        std::fs::write(&path, "# Title\n\nAlpha para.\n\nBeta para.\n").unwrap();

        let (mut app, _) = App::boot(Some(path.clone()), false);
        assert!(app.note_store.is_some(), "notes open with a saved doc");

        // Preview, pointer to the "Beta para." block: heading(0), Alpha(1), Beta(2).
        app.doc.set_cursor_pos(0, false);
        let _ = app.update(Message::TogglePreview);
        let _ = app.update(Message::PreviewPointer(1));
        let _ = app.update(Message::PreviewPointer(1));
        assert_eq!(app.preview_pointer, 2);

        // Add a note on that block.
        let _ = app.update(Message::NoteOpen);
        assert_eq!(app.overlay, Overlay::Note);
        app.input = "check this claim".to_string();
        let _ = app.update(Message::OverlaySubmit { backwards: false });
        assert_eq!(app.overlay, Overlay::None);
        let notes = app.note_store.as_ref().unwrap().notes();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].body, "check this claim");
        assert_eq!(
            app.block_of(notes[0].start),
            2,
            "anchored to the Beta block"
        );

        // Re-opening on the same block edits (prefilled), Esc clears edit state.
        let _ = app.update(Message::NoteOpen);
        assert!(app.note_edit_id.is_some());
        assert_eq!(app.input, "check this claim");
        let _ = app.update(Message::OverlayClose);
        assert!(app.note_edit_id.is_none());

        // Marks render when visible, vanish when hidden.
        assert_eq!(app.note_marks().len(), 1);
        let _ = app.update(Message::ToggleNotes);
        assert!(app.note_marks().is_empty());
        let _ = app.update(Message::ToggleNotes);

        // Resolve flips the mark; jump from the top lands on the noted block.
        let _ = app.update(Message::NoteResolve);
        assert!(app.note_marks()[0].resolved);
        app.preview_pointer = 0;
        let _ = app.update(Message::NoteJump(1));
        assert_eq!(app.preview_pointer, 2);

        // Delete removes it, and the removal persists.
        let _ = app.update(Message::NoteDelete);
        assert!(app.note_store.as_ref().unwrap().is_empty());
        assert!(polaris_drafts::NoteStore::for_document(&path)
            .unwrap()
            .is_empty());

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn review_import_decide_apply_and_undo() {
        let (mut app, _) = App::boot(None, false);
        type_into(&mut app, "one two three four");

        let dir = std::env::temp_dir().join("polaris-gui-review");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("edited.md");
        std::fs::write(&path, "one TWO three FOUR").unwrap();

        // Import the edited copy through the overlay.
        app.input = path.to_string_lossy().into_owned();
        app.overlay = Overlay::Import;
        let _ = app.update(Message::OverlaySubmit { backwards: false });
        assert_eq!(app.view_mode, ViewMode::Review);
        assert_eq!(app.review.as_ref().unwrap().change_count(), 2);

        // Accept the first change (auto-advances), reject the second.
        let _ = app.update(Message::ReviewAccept);
        assert_eq!(app.review_index, 1);
        let _ = app.update(Message::ReviewReject);

        // Apply: buffer takes the accepted change, keeps the rejected one.
        let _ = app.update(Message::ReviewApply);
        assert_eq!(app.view_mode, ViewMode::Write);
        assert!(app.review.is_none());
        assert_eq!(app.doc.text(), "one TWO three four");

        // One undo group: Cmd+Z reverts the whole apply.
        assert!(app.doc.undo());
        assert_eq!(app.doc.text(), "one two three four");

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn review_cancel_leaves_the_buffer_untouched() {
        let (mut app, _) = App::boot(None, false);
        type_into(&mut app, "hello world");
        let dir = std::env::temp_dir().join("polaris-gui-review-cancel");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("edited.md");
        std::fs::write(&path, "hello brave world").unwrap();

        app.input = path.to_string_lossy().into_owned();
        app.overlay = Overlay::Import;
        let _ = app.update(Message::OverlaySubmit { backwards: false });
        assert_eq!(app.view_mode, ViewMode::Review);

        let _ = app.update(Message::ReviewAcceptAll);
        let _ = app.update(Message::ReviewCancel);
        assert_eq!(app.view_mode, ViewMode::Write);
        assert!(app.review.is_none());
        assert_eq!(app.doc.text(), "hello world", "cancel touches nothing");

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn typing_fades_chrome_and_rest_restores_it() {
        let (mut app, _) = App::boot(None, false);
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

        let (mut app, _) = App::boot(Some(path.clone()), false);
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
        let (mut app, _) = App::boot(None, false);
        type_into(&mut app, "not yet saved");
        let id = iced::window::Id::unique();
        let _ = app.update(Message::CloseRequested(id));
        assert_eq!(app.overlay, Overlay::SaveAs, "one chance to name it");
        assert!(app.close_pending);
        // Typing re-arms the warning.
        type_into(&mut app, " more");
        assert!(!app.close_pending);
        // Empty untitled buffers close without ceremony.
        let (mut empty, _) = App::boot(None, false);
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

        let (mut app, _) = App::boot(Some(old.clone()), false);
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

        let (mut app, _) = App::boot(Some(a.clone()), false);
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
        let (mut app, _) = App::boot(None, false);
        let _ = app.update(Message::RenameOpen);
        assert_eq!(app.overlay, Overlay::SaveAs);
    }

    #[test]
    fn theme_toggle_flips_and_works_in_preview_too() {
        let (mut app, _) = App::boot(None, false);
        let initial = app.dark;
        let _ = app.update(Message::ToggleTheme);
        assert_eq!(app.dark, !initial);
        let _ = app.update(Message::TogglePreview);
        let _ = app.update(Message::ToggleTheme);
        assert_eq!(app.dark, initial);
    }

    #[test]
    fn publish_requires_a_saved_file() {
        let (mut app, _) = App::boot(None, false);
        let _ = app.update(Message::PublishOpen);
        assert_eq!(app.overlay, Overlay::None);
        assert!(app.status.as_deref().unwrap_or("").contains("save"));
    }

    fn fake_targets(n: usize, dir: &std::path::Path) -> Vec<Box<dyn polaris_publish::Target>> {
        (0..n)
            .map(|_| {
                Box::new(polaris_publish::HugoTarget::new(
                    dir.to_path_buf(),
                    toml::Table::new(),
                )) as Box<dyn polaris_publish::Target>
            })
            .collect()
    }

    #[test]
    fn publish_confirm_saves_and_starts_exactly_one_publish() {
        let dir = std::env::temp_dir().join("polaris-gui-publish");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("publish.md");
        std::fs::write(&path, "content\n").unwrap();

        let (mut app, _) = App::boot(Some(path.clone()), false);
        type_into(&mut app, "更 ");
        // Seed the picker directly (bypasses Config::load); two targets means
        // the overlay path, not fire-through, is exercised.
        app.publish_targets = fake_targets(2, &dir);
        app.publish_selected = 0;
        app.overlay = Overlay::Publish;

        let _ = app.update(Message::OverlaySubmit { backwards: false });
        assert!(app.publishing);
        assert_eq!(app.overlay, Overlay::None);
        assert!(!app.doc.is_dirty(), "saved before publishing");
        assert!(app.status.as_deref().unwrap_or("").contains("publishing"));
        assert!(app.publish_targets.is_empty(), "target list consumed");

        // Re-triggering while in flight is a no-op.
        let _ = app.update(Message::PublishOpen);
        assert_eq!(app.overlay, Overlay::None);

        let _ = app.update(Message::PublishDone(Ok(polaris_publish::Outcome::Url(
            "https://notion.so/x".into(),
        ))));
        assert!(!app.publishing);
        assert!(app.status.as_deref().unwrap_or("").contains("published"));

        // Hugo's overwrite guard arms a forced retry; a generic error clears it.
        let _ = app.update(Message::PublishDone(Err("x already exists y".into())));
        assert!(app.overwrite_armed);
        assert!(app.status.as_deref().unwrap_or("").contains("overwrite"));
        let _ = app.update(Message::PublishDone(Err("401".into())));
        assert!(!app.overwrite_armed);
        assert!(app
            .status
            .as_deref()
            .unwrap_or("")
            .contains("publish failed"));

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn publish_picker_nav_clamps_within_the_list() {
        let (mut app, _) = App::boot(None, false);
        app.publish_targets = fake_targets(2, &std::env::temp_dir());
        app.overlay = Overlay::Publish;
        app.publish_selected = 0;

        let _ = app.update(Message::PublishNav(-1));
        assert_eq!(app.publish_selected, 0, "clamps at the top");
        let _ = app.update(Message::PublishNav(1));
        assert_eq!(app.publish_selected, 1);
        let _ = app.update(Message::PublishNav(1));
        assert_eq!(app.publish_selected, 1, "clamps at the bottom");
    }

    #[test]
    fn mark_browse_and_restore_a_draft() {
        let dir = std::env::temp_dir().join("polaris-gui-drafts");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("novel.md");
        std::fs::write(&path, "chapter one\n").unwrap();

        let (mut app, _) = App::boot(Some(path.clone()), false);
        assert!(app.store.is_some(), "store opens with the document");

        // Mark a draft (Cmd+M -> prefilled overlay -> Enter).
        let _ = app.update(Message::Editor(editor::Action::Command {
            key: "m".to_string(),
            shift: false,
        }));
        assert_eq!(app.overlay, Overlay::Mark);
        assert_eq!(app.input, "Draft 1", "prefilled");
        let _ = app.update(Message::OverlaySubmit { backwards: false });
        assert_eq!(app.overlay, Overlay::None);
        assert!(app.status.as_deref().unwrap_or("").contains("draft marked"));

        // Write more, then open the browser (Cmd+Shift+M).
        type_into(&mut app, "and then some more words ");
        let _ = app.update(Message::Editor(editor::Action::Command {
            key: "m".to_string(),
            shift: true,
        }));
        assert_eq!(app.view_mode, ViewMode::Drafts);

        // Newest first; navigate to the marked draft and view it.
        let entries = app.store.as_ref().unwrap().entries().len();
        assert!(entries >= 2, "baseline auto + marked draft");
        let _ = app.update(Message::DraftsNav(1));
        let _ = app.update(Message::DraftsOpenSelected);
        assert_eq!(app.view_mode, ViewMode::DraftView);

        // Restore: current text snapshotted first, replace is undoable.
        let before_restore = app.doc.text();
        let _ = app.update(Message::DraftsRestore);
        assert_eq!(app.view_mode, ViewMode::Write);
        assert_ne!(app.doc.text(), before_restore, "content replaced");
        act(&mut app, editor::Action::Undo);
        assert_eq!(app.doc.text(), before_restore, "restore is one undo");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn marking_untitled_hints_instead() {
        let (mut app, _) = App::boot(None, false);
        let _ = app.update(Message::Editor(editor::Action::Command {
            key: "m".to_string(),
            shift: false,
        }));
        assert_eq!(app.overlay, Overlay::None);
        assert!(app.status.as_deref().unwrap_or("").contains("save"));
    }

    #[test]
    fn rename_migrates_draft_history() {
        let dir = std::env::temp_dir().join("polaris-gui-migrate");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let old = dir.join("before.md");
        std::fs::write(&old, "words\n").unwrap();

        let (mut app, _) = App::boot(Some(old.clone()), false);
        let _ = app.update(Message::Editor(editor::Action::Command {
            key: "m".to_string(),
            shift: false,
        }));
        let _ = app.update(Message::OverlaySubmit { backwards: false });

        let _ = app.update(Message::RenameOpen);
        let _ = app.update(Message::OverlayInput("after.md".to_string()));
        let _ = app.update(Message::OverlaySubmit { backwards: false });

        let store = app.store.as_ref().unwrap();
        assert!(
            store
                .entries()
                .iter()
                .any(|e| e.name.as_deref() == Some("Draft 1")),
            "history followed the rename"
        );
        assert!(dir.join(".polaris/after.md").exists());
        assert!(!dir.join(".polaris/before.md").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn welcome_tour_mentions_every_binding() {
        for key in [
            "Cmd+P",
            "Cmd+F",
            "Cmd+S",
            "Cmd+R",
            "Cmd+T",
            "Cmd+Z",
            "Cmd+Y",
            "Cmd+G",
            "Cmd+E",
            "Cmd+K",
            "Cmd+L",
            "Cmd+M",
            "Cmd+Shift+M",
            "Cmd+D",
        ] {
            assert!(
                super::welcome::WELCOME.contains(key),
                "welcome tour is missing {key}"
            );
        }
    }

    #[test]
    fn find_overlay_matches_and_cycles() {
        let (mut app, _) = App::boot(None, false);
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
