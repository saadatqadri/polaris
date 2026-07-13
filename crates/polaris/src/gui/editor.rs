//! The Polaris editor widget (Phase 2): a custom iced widget rendering
//! `polaris-core::Document` directly. The Document is the single source of
//! truth — the widget draws `doc.cursor()`/`doc.selection()` and emits
//! [`Action`]s; it never owns text state. Promoted from the spike
//! (docs/SPIKE-editor-widget.md).
//!
//! The caret is steady, not blinking — DESIGN.md: "Nothing animates,
//! blinks, or badges while words are arriving."

use std::cell::Cell;
use std::time::{Duration, Instant};

use iced::advanced::input_method;
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer::{self, Quad};
use iced::advanced::text::{self as advanced_text, Paragraph as _, Text};
use iced::advanced::widget::{tree, Tree, Widget};
use iced::advanced::{clipboard, Clipboard, Shell};
use iced::widget::text::Span;
use iced::{
    keyboard, mouse, window, Color, Element, Event, Font, Length, Pixels, Point, Rectangle, Size,
    Theme,
};

use polaris_core::buffer::Buffer;
use polaris_core::{cursor as core_cursor, Document};

use super::{fonts, theme};

type Renderer = iced::Renderer;
type ParagraphOf = <Renderer as advanced_text::Renderer>::Paragraph;

const BODY_SIZE: f32 = 19.0;
const LINE_HEIGHT: f32 = 1.56;
/// Kept visible around the caret when auto-scrolling (non-typewriter).
const SCROLL_MARGIN: f32 = 72.0;
/// Typewriter mode holds the caret's row at this fraction of the viewport.
const TYPEWRITER_HOLD: f32 = 0.45;
const DOUBLE_CLICK_MS: u64 = 400;

/// What the widget asks of the application. Editing goes through core.
#[derive(Debug, Clone)]
pub enum Action {
    Insert(String),
    Enter,
    Backspace,
    Delete,
    DeleteWordBack,
    DeleteWordForward,
    DeleteToLineStart,
    DeleteToLineEnd,
    Move(Motion, bool),
    /// Up/Down resolved to a char position by the widget's wrap layout, so
    /// navigation follows visual (soft-wrapped) rows, not buffer lines.
    VerticalMove {
        target: usize,
        extend: bool,
    },
    SelectAll,
    Click {
        position: usize,
        extend: bool,
    },
    DragTo {
        position: usize,
    },
    SelectWord {
        position: usize,
    },
    Cut,
    Paste(String),
    Undo,
    Redo,
    /// An unclaimed Cmd/Ctrl+<char> shortcut, for the app's keymap.
    Command {
        key: String,
        shift: bool,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum Motion {
    Left,
    Right,
    Up,
    Down,
    WordLeft,
    WordRight,
    Home,
    End,
    DocStart,
    DocEnd,
}

pub struct EditorView<'a, Message> {
    doc: &'a Document,
    text_version: u64,
    /// False while a chrome overlay owns the keyboard.
    active: bool,
    typewriter: bool,
    focus_dim: bool,
    tokens: theme::Tokens,
    on_action: Box<dyn Fn(Action) -> Message + 'a>,
}

impl<'a, Message> EditorView<'a, Message> {
    pub fn new(
        doc: &'a Document,
        text_version: u64,
        active: bool,
        typewriter: bool,
        focus_dim: bool,
        tokens: theme::Tokens,
        on_action: impl Fn(Action) -> Message + 'a,
    ) -> Self {
        Self {
            doc,
            text_version,
            active,
            typewriter,
            focus_dim,
            tokens,
            on_action: Box::new(on_action),
        }
    }

    /// Caret as (buffer line, byte column within the line).
    fn caret(&self) -> (usize, usize) {
        let buffer = self.doc.buffer();
        char_pos_to_line_byte(buffer, self.doc.cursor().pos)
    }
}

struct State {
    paragraphs: Vec<ParagraphOf>,
    heights: Vec<f32>,
    built_version: u64,
    built_width: f32,
    /// Interior-mutable: draw() keeps the caret in view.
    scroll: Cell<f32>,
    dragging: bool,
    last_click: Option<(Instant, usize)>,
    preedit: Option<String>,
    /// Preferred visual x (pixels) held across a run of Up/Down, so the
    /// caret keeps its column over short rows. Cleared by any other key.
    sticky_x: Option<f32>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            paragraphs: Vec::new(),
            heights: Vec::new(),
            built_version: u64::MAX,
            built_width: 0.0,
            scroll: Cell::new(0.0),
            dragging: false,
            last_click: None,
            preedit: None,
            sticky_x: None,
        }
    }
}

fn line_height_px() -> f32 {
    advanced_text::LineHeight::Relative(LINE_HEIGHT)
        .to_absolute(Pixels(BODY_SIZE))
        .0
}

fn semibold() -> Font {
    Font {
        weight: iced::font::Weight::Semibold,
        ..fonts::WRITING
    }
}

fn italic() -> Font {
    Font {
        style: iced::font::Style::Italic,
        ..fonts::WRITING
    }
}

/// Markdown source, visible but quiet: prefix marks in `quiet`, heading
/// content semibold, quote content italic.
fn line_spans(line: &str, t: theme::Tokens) -> Vec<Span<'static>> {
    let hashes = line.len() - line.trim_start_matches('#').len();
    if (1..=6).contains(&hashes) && line[hashes..].starts_with(' ') {
        return vec![
            Span::new(line[..hashes + 1].to_string()).color(t.quiet),
            Span::new(line[hashes + 1..].to_string()).font(semibold()),
        ];
    }
    if let Some(rest) = line.strip_prefix("> ") {
        return vec![
            Span::new("> ".to_string()).color(t.quiet),
            Span::new(rest.to_string()).font(italic()),
        ];
    }
    for marker in ["- ", "* "] {
        if let Some(rest) = line.strip_prefix(marker) {
            return vec![
                Span::new(marker.to_string()).color(t.quiet),
                Span::new(rest.to_string()),
            ];
        }
    }
    vec![Span::new(line.to_string())]
}

fn text_defaults<C>(content: C, width: f32) -> Text<C> {
    Text {
        content,
        bounds: Size::new(width, f32::INFINITY),
        size: Pixels(BODY_SIZE),
        line_height: advanced_text::LineHeight::Relative(LINE_HEIGHT),
        font: fonts::WRITING,
        align_x: advanced_text::Alignment::Left,
        align_y: iced::alignment::Vertical::Top,
        shaping: advanced_text::Shaping::Advanced,
        wrapping: advanced_text::Wrapping::Word,
    }
}

/// Position of a byte offset inside a (possibly soft-wrapped) line, via a
/// probe layout of the text up to that offset: (x, y of the visual row).
fn position_in_line(line: &str, byte: usize, width: f32) -> (f32, f32) {
    let lh = line_height_px();
    let prefix = &line[..byte.min(line.len())];
    if prefix.is_empty() {
        return (0.0, 0.0);
    }
    let probe = ParagraphOf::with_text(text_defaults(prefix, width));
    let rows = (probe.min_bounds().height / lh).round().max(1.0) as usize;
    let x = probe
        .grapheme_position(rows - 1, usize::MAX / 2)
        .map(|p| p.x)
        .unwrap_or(0.0);
    (x, (rows - 1) as f32 * lh)
}

/// End x of each visual row of a laid-out paragraph.
fn row_end_x(paragraph: &ParagraphOf, row: usize) -> f32 {
    paragraph
        .grapheme_position(row, usize::MAX / 2)
        .map(|p| p.x)
        .unwrap_or(0.0)
}

/// A one-buffer-line step used as the anti-stuck fallback for visual moves:
/// down lands at the start of the next line (end of doc if none), up at the
/// start of the previous line (0 if none). Guarantees forward/back progress.
fn buffer_line_step(buffer: &Buffer, pos: usize, up: bool) -> usize {
    let line = buffer.char_to_line(pos);
    if up {
        if line == 0 {
            0
        } else {
            buffer.line_to_char(line - 1)
        }
    } else if line + 1 >= buffer.len_lines() {
        buffer.len_chars()
    } else {
        buffer.line_to_char(line + 1)
    }
}

fn char_pos_to_line_byte(buffer: &Buffer, pos: usize) -> (usize, usize) {
    let line = buffer.char_to_line(pos);
    let line_start = buffer.line_to_char(line);
    let text = buffer.line(line);
    let byte = text
        .char_indices()
        .nth(pos - line_start)
        .map(|(b, _)| b)
        .unwrap_or_else(|| text.strip_suffix('\n').unwrap_or(&text).len());
    (line, byte)
}

fn line_byte_to_char_pos(buffer: &Buffer, line: usize, byte: usize) -> usize {
    let line = line.min(buffer.len_lines().saturating_sub(1));
    let line_start = buffer.line_to_char(line);
    let text = buffer.line(line);
    let content = text.strip_suffix('\n').unwrap_or(&text);
    let byte = byte.min(content.len());
    let mut chars = 0;
    for (b, _) in content.char_indices() {
        if b >= byte {
            break;
        }
        chars += 1;
    }
    line_start + chars
}

impl State {
    fn rebuild(&mut self, doc: &Document, tokens: theme::Tokens, version: u64, width: f32) {
        let text = doc.text();
        self.paragraphs = text
            .split('\n')
            .map(|line| ParagraphOf::with_spans(text_defaults(&line_spans(line, tokens), width)))
            .collect();
        let lh = line_height_px();
        self.heights = self
            .paragraphs
            .iter()
            .map(|p| p.min_bounds().height.max(lh))
            .collect();
        self.built_version = version;
        self.built_width = width;
    }

    fn content_height(&self) -> f32 {
        self.heights.iter().sum()
    }

    fn y_of_line(&self, line: usize) -> f32 {
        self.heights[..line.min(self.heights.len())].iter().sum()
    }

    /// The caret's visual x within its paragraph (pixels).
    fn caret_x(&self, doc: &Document, width: f32) -> f32 {
        let buffer = doc.buffer();
        let (line, byte) = char_pos_to_line_byte(buffer, doc.cursor().pos);
        let text = buffer.line(line);
        let content = text.strip_suffix('\n').unwrap_or(&text);
        position_in_line(content, byte, width).0
    }

    /// Move one visual row up or down at the preferred x, following
    /// soft-wrapped rows and crossing paragraph boundaries. Returns the new
    /// char position (clamped to document start/end at the edges).
    fn visual_move(&self, doc: &Document, up: bool, sticky_x: f32, width: f32) -> usize {
        let buffer = doc.buffer();
        let (line, byte) = char_pos_to_line_byte(buffer, doc.cursor().pos);
        let text = buffer.line(line);
        let content = text.strip_suffix('\n').unwrap_or(&text);
        let (_, cy) = position_in_line(content, byte, width);
        let lh = line_height_px();
        let row_top = self.y_of_line(line) + cy;
        // Aim at the middle of the neighbouring row.
        let target_y = if up {
            row_top - lh * 0.5
        } else {
            row_top + lh * 1.5
        };
        if target_y < 0.0 {
            return 0;
        }
        let start = doc.cursor().pos;
        let mut hit_pos = None;
        let mut y = 0.0;
        for (i, paragraph) in self.paragraphs.iter().enumerate() {
            let h = self.heights[i];
            if target_y < y + h {
                let local = Point::new(sticky_x.max(0.0), (target_y - y).max(0.0));
                let hit = paragraph
                    .hit_test(local)
                    .map(|hit| hit.cursor())
                    .unwrap_or(usize::MAX);
                hit_pos = Some(line_byte_to_char_pos(buffer, i, hit));
                break;
            }
            y += h;
        }
        let target = hit_pos.unwrap_or_else(|| buffer.len_chars());
        // A glyph outside the writing font (emoji, ●) lays out with fallback
        // metrics, so the geometric hit can land on the same spot — never let
        // Up/Down stick. Fall back to a plain buffer-line step.
        if target == start {
            buffer_line_step(buffer, start, up)
        } else {
            target
        }
    }

    /// The scroll offset for this frame; non-typewriter keeps the caret in
    /// view and remembers the result.
    fn offset(&self, doc: &Document, typewriter: bool, viewport_height: f32, width: f32) -> f32 {
        let buffer = doc.buffer();
        let (line, byte) = char_pos_to_line_byte(buffer, doc.cursor().pos);
        let line_text = buffer.line(line);
        let content = line_text.strip_suffix('\n').unwrap_or(&line_text);
        let (_, cy) = position_in_line(content, byte, width);
        let caret_top = self.y_of_line(line) + cy;
        let lh = line_height_px();

        if typewriter {
            return caret_top - viewport_height * TYPEWRITER_HOLD;
        }

        let mut scroll = self
            .scroll
            .get()
            .min((self.content_height() - viewport_height).max(0.0))
            .max(0.0);
        if caret_top < scroll + SCROLL_MARGIN {
            scroll = (caret_top - SCROLL_MARGIN).max(0.0);
        } else if caret_top + lh > scroll + viewport_height - SCROLL_MARGIN {
            scroll = caret_top + lh - viewport_height + SCROLL_MARGIN;
        }
        self.scroll.set(scroll);
        scroll
    }

    /// Map a point in widget space to a char position in the document.
    fn hit_char(&self, doc: &Document, point: Point, offset: f32) -> usize {
        let buffer = doc.buffer();
        let mut y = -offset;
        for (i, paragraph) in self.paragraphs.iter().enumerate() {
            let h = self.heights[i];
            if point.y < y + h {
                let byte = paragraph
                    .hit_test(Point::new(point.x.max(0.0), (point.y - y).max(0.0)))
                    .map(|hit| hit.cursor())
                    .unwrap_or(usize::MAX);
                return line_byte_to_char_pos(buffer, i, byte);
            }
            y += h;
        }
        buffer.len_chars()
    }
}

impl<Message> Widget<Message, Theme, Renderer> for EditorView<'_, Message> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let size = limits.max();
        let state = tree.state.downcast_mut::<State>();
        if state.built_version != self.text_version || state.built_width != size.width {
            state.rebuild(self.doc, self.tokens, self.text_version, size.width);
        }
        layout::Node::new(size)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        use iced::advanced::text::Renderer as _;
        use renderer::Renderer as _;

        let bounds = layout.bounds();
        let state = tree.state.downcast_ref::<State>();
        let t = self.tokens;
        let lh = line_height_px();
        let buffer = self.doc.buffer();

        let offset = state.offset(self.doc, self.typewriter, bounds.height, state.built_width);
        let (caret_line, caret_byte) = self.caret();
        let caret_text = buffer.line(caret_line);
        let caret_content = caret_text.strip_suffix('\n').unwrap_or(&caret_text);
        let (cx, cy) = position_in_line(caret_content, caret_byte, state.built_width);
        let caret_top = state.y_of_line(caret_line) + cy;

        renderer.with_layer(bounds, |renderer| {
            // Selection, under the text, in translucent star.
            if let Some(selection) = self.doc.selection() {
                let star_soft = Color { a: 0.18, ..t.star };
                let (start_line, start_byte) = char_pos_to_line_byte(buffer, selection.start);
                let (end_line, end_byte) = char_pos_to_line_byte(buffer, selection.end);
                for line in start_line..=end_line.min(state.paragraphs.len().saturating_sub(1)) {
                    let text = buffer.line(line);
                    let content = text.strip_suffix('\n').unwrap_or(&text);
                    let from = if line == start_line { start_byte } else { 0 };
                    let to = if line == end_line {
                        end_byte
                    } else {
                        content.len()
                    };
                    let (x0, y0) = position_in_line(content, from, state.built_width);
                    let (x1, y1) = position_in_line(content, to, state.built_width);
                    let row0 = (y0 / lh).round() as usize;
                    let row1 = (y1 / lh).round() as usize;
                    let line_top = state.y_of_line(line);
                    for row in row0..=row1 {
                        let sx = if row == row0 { x0 } else { 0.0 };
                        let ex = if row == row1 {
                            x1
                        } else {
                            row_end_x(&state.paragraphs[line], row)
                        };
                        let width = (ex - sx).max(4.0);
                        let y = bounds.y - offset + line_top + row as f32 * lh;
                        if y + lh < bounds.y || y > bounds.y + bounds.height {
                            continue;
                        }
                        renderer.fill_quad(
                            Quad {
                                bounds: Rectangle {
                                    x: bounds.x + sx,
                                    y,
                                    width,
                                    height: lh,
                                },
                                ..Quad::default()
                            },
                            star_soft,
                        );
                    }
                }
            }

            // The text.
            let mut y = bounds.y - offset;
            for (i, paragraph) in state.paragraphs.iter().enumerate() {
                let h = state.heights[i];
                if y + h >= bounds.y && y <= bounds.y + bounds.height {
                    let ink = if self.focus_dim && i != caret_line {
                        Color { a: 0.30, ..t.ink }
                    } else {
                        t.ink
                    };
                    renderer.fill_paragraph(
                        paragraph,
                        Point::new(bounds.x, y),
                        ink,
                        Rectangle::with_size(Size::INFINITE),
                    );
                }
                y += h;
            }

            // IME preedit: inline at the caret, underlined.
            let mut preedit_width = 0.0;
            if let Some(preedit) = state.preedit.as_ref().filter(|s| !s.is_empty()) {
                let paragraph =
                    ParagraphOf::with_text(text_defaults(preedit.as_str(), f32::INFINITY));
                let size = paragraph.min_bounds();
                preedit_width = size.width;
                let x = bounds.x + cx;
                let y = bounds.y - offset + caret_top;
                renderer.fill_paragraph(
                    &paragraph,
                    Point::new(x, y),
                    t.ink,
                    Rectangle::with_size(Size::INFINITE),
                );
                renderer.fill_quad(
                    Quad {
                        bounds: Rectangle {
                            x,
                            y: y + lh - 2.0,
                            width: size.width,
                            height: 1.0,
                        },
                        ..Quad::default()
                    },
                    t.star,
                );
            }

            // The caret: steady, star, ours.
            if self.active {
                renderer.fill_quad(
                    Quad {
                        bounds: Rectangle {
                            x: bounds.x + cx + preedit_width,
                            y: bounds.y - offset + caret_top + (lh - BODY_SIZE * 1.2) / 2.0,
                            width: 2.0,
                            height: BODY_SIZE * 1.2,
                        },
                        ..Quad::default()
                    },
                    t.star,
                );
            }
        });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let state = tree.state.downcast_mut::<State>();

        match event {
            Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                modifiers,
                text,
                ..
            }) if self.active => {
                use keyboard::key::Named;
                use keyboard::Key;
                let word = modifiers.alt();
                let extend = modifiers.shift();
                let action = if modifiers.command() {
                    match key.as_ref() {
                        Key::Character("z") if modifiers.shift() => Some(Action::Redo),
                        Key::Character("z") => Some(Action::Undo),
                        Key::Character("a") => Some(Action::SelectAll),
                        Key::Character("c") | Key::Character("x") => {
                            if let Some(selected) = self.doc.selected_text() {
                                clipboard.write(clipboard::Kind::Standard, selected);
                            }
                            if matches!(key.as_ref(), Key::Character("x")) {
                                Some(Action::Cut)
                            } else {
                                shell.capture_event();
                                None
                            }
                        }
                        Key::Character("v") => {
                            clipboard.read(clipboard::Kind::Standard).map(Action::Paste)
                        }
                        // macOS: Cmd+Left/Right = line start/end,
                        // Cmd+Up/Down = document start/end.
                        Key::Named(Named::ArrowLeft) => Some(Action::Move(Motion::Home, extend)),
                        Key::Named(Named::ArrowRight) => Some(Action::Move(Motion::End, extend)),
                        Key::Named(Named::ArrowUp) => Some(Action::Move(Motion::DocStart, extend)),
                        Key::Named(Named::ArrowDown) => Some(Action::Move(Motion::DocEnd, extend)),
                        // Cmd+Delete = delete to line start / end (macOS).
                        Key::Named(Named::Backspace) => Some(Action::DeleteToLineStart),
                        Key::Named(Named::Delete) => Some(Action::DeleteToLineEnd),
                        Key::Character(c) => Some(Action::Command {
                            key: c.to_string(),
                            shift: modifiers.shift(),
                        }),
                        _ => None,
                    }
                } else {
                    match key.as_ref() {
                        // Option+Delete = delete a word (macOS).
                        Key::Named(Named::Backspace) if word => Some(Action::DeleteWordBack),
                        Key::Named(Named::Delete) if word => Some(Action::DeleteWordForward),
                        Key::Named(Named::Backspace) => Some(Action::Backspace),
                        Key::Named(Named::Delete) => Some(Action::Delete),
                        Key::Named(Named::Enter) => Some(Action::Enter),
                        Key::Named(Named::ArrowLeft) => Some(Action::Move(
                            if word { Motion::WordLeft } else { Motion::Left },
                            extend,
                        )),
                        Key::Named(Named::ArrowRight) => Some(Action::Move(
                            if word {
                                Motion::WordRight
                            } else {
                                Motion::Right
                            },
                            extend,
                        )),
                        // Up/Down follow visual rows (soft wrap), resolved
                        // here where the wrap layout lives.
                        Key::Named(Named::ArrowUp) | Key::Named(Named::ArrowDown) => {
                            let up = matches!(key.as_ref(), Key::Named(Named::ArrowUp));
                            let width = state.built_width;
                            let sx = match state.sticky_x {
                                Some(x) => x,
                                None => {
                                    let x = state.caret_x(self.doc, width);
                                    state.sticky_x = Some(x);
                                    x
                                }
                            };
                            let target = state.visual_move(self.doc, up, sx, width);
                            Some(Action::VerticalMove { target, extend })
                        }
                        Key::Named(Named::Home) => Some(Action::Move(Motion::Home, extend)),
                        Key::Named(Named::End) => Some(Action::Move(Motion::End, extend)),
                        _ => text
                            .as_ref()
                            .map(|t| t.to_string())
                            .filter(|t| !t.is_empty() && t.chars().all(|c| !c.is_control()))
                            .map(Action::Insert),
                    }
                };
                // Vertical runs keep the preferred x; anything else drops it.
                if !matches!(action, Some(Action::VerticalMove { .. })) {
                    state.sticky_x = None;
                }
                if let Some(action) = action {
                    shell.publish((self.on_action)(action));
                    shell.capture_event();
                }
            }
            Event::InputMethod(ime_event) if self.active => match ime_event {
                input_method::Event::Opened => {
                    state.preedit = Some(String::new());
                }
                input_method::Event::Preedit(content, _selection) => {
                    state.preedit = Some(content.clone());
                    shell.request_redraw();
                }
                input_method::Event::Commit(text) => {
                    state.preedit = None;
                    shell.publish((self.on_action)(Action::Insert(text.clone())));
                }
                input_method::Event::Closed => {
                    state.preedit = None;
                    shell.request_redraw();
                }
            },
            Event::Window(window::Event::RedrawRequested(_)) if self.active => {
                // Position the IME candidate window at the caret.
                let buffer = self.doc.buffer();
                let (line, byte) = char_pos_to_line_byte(buffer, self.doc.cursor().pos);
                let text = buffer.line(line);
                let content = text.strip_suffix('\n').unwrap_or(&text);
                let width = if state.built_width > 0.0 {
                    state.built_width
                } else {
                    bounds.width
                };
                let (cx, cy) = position_in_line(content, byte, width);
                let offset = state.offset(self.doc, self.typewriter, bounds.height, width);
                let caret = Rectangle {
                    x: bounds.x + cx,
                    y: bounds.y - offset + state.y_of_line(line) + cy,
                    width: 2.0,
                    height: line_height_px(),
                };
                shell.request_input_method(&input_method::InputMethod::Enabled {
                    cursor: caret,
                    purpose: input_method::Purpose::Normal,
                    preedit: state.preedit.as_ref().map(|content| input_method::Preedit {
                        content: content.as_str(),
                        selection: None,
                        text_size: Some(Pixels(BODY_SIZE)),
                    }),
                });
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(position) = cursor.position_in(bounds) {
                    let offset =
                        state.offset(self.doc, self.typewriter, bounds.height, state.built_width);
                    let char_pos = state.hit_char(self.doc, position, offset);
                    let now = Instant::now();
                    let double = state.last_click.is_some_and(|(at, pos)| {
                        pos == char_pos
                            && now.duration_since(at) < Duration::from_millis(DOUBLE_CLICK_MS)
                    });
                    state.last_click = Some((now, char_pos));
                    state.dragging = true;
                    let action = if double {
                        Action::SelectWord { position: char_pos }
                    } else {
                        Action::Click {
                            position: char_pos,
                            extend: false,
                        }
                    };
                    shell.publish((self.on_action)(action));
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) if state.dragging => {
                if let Some(position) = cursor.position_in(bounds) {
                    let offset =
                        state.offset(self.doc, self.typewriter, bounds.height, state.built_width);
                    let char_pos = state.hit_char(self.doc, position, offset);
                    if char_pos != self.doc.cursor().pos {
                        shell.publish((self.on_action)(Action::DragTo { position: char_pos }));
                    }
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.dragging = false;
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta })
                if cursor.position_in(bounds).is_some() && !self.typewriter =>
            {
                let dy = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => y * line_height_px(),
                    mouse::ScrollDelta::Pixels { y, .. } => *y,
                };
                let max = (state.content_height() - bounds.height).max(0.0);
                state.scroll.set((state.scroll.get() - dy).clamp(0.0, max));
                shell.request_redraw();
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if cursor.position_in(layout.bounds()).is_some() {
            mouse::Interaction::Text
        } else {
            mouse::Interaction::None
        }
    }
}

impl<'a, Message: 'a> From<EditorView<'a, Message>> for Element<'a, Message> {
    fn from(editor: EditorView<'a, Message>) -> Self {
        Element::new(editor)
    }
}

/// Word selection bounds around a position, for double-click.
pub fn word_range_at(buffer: &Buffer, position: usize) -> std::ops::Range<usize> {
    let start = core_cursor::prev_word_boundary(buffer, position.min(buffer.len_chars()));
    let end = core_cursor::next_word_boundary(buffer, start);
    start..end.max(position)
}

#[cfg(test)]
mod tests {
    use super::buffer_line_step;
    use polaris_core::buffer::Buffer;

    #[test]
    fn buffer_line_step_always_progresses_and_clamps() {
        let b = Buffer::from_str("one\ntwo\nthree");
        // Down from line 0 → start of line 1; up → 0.
        assert_eq!(buffer_line_step(&b, 1, false), 4); // "two" starts at char 4
        assert_eq!(buffer_line_step(&b, 1, true), 0);
        // Down from the last line clamps to end; up from first clamps to 0.
        let end = b.len_chars();
        assert_eq!(buffer_line_step(&b, 9, false), end);
        assert_eq!(buffer_line_step(&b, 0, true), 0);
    }
}
