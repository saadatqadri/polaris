//! Phase 2 spike: a custom editor widget over `polaris-core`, bypassing
//! iced's `text_editor`. Run with `polaris spike [file]`.
//!
//! Proves the four capabilities the shim can't do:
//!   1. per-span styling (markdown marks in `quiet`)  — heading/quote/list
//!      prefixes here; the same machinery extends to inline marks
//!   2. typewriter scrolling (Cmd+Y): caret line held vertically steady
//!   3. focus dimming (Cmd+G): only the caret's paragraph at full ink
//!   4. caret/selection fully driven by `polaris-core::Document`
//!
//! Deliberately NOT in the spike: selection rendering, IME, clipboard,
//! overlays, autosave. Findings in docs/SPIKE-editor-widget.md.

use std::path::PathBuf;

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer::{self, Quad};
use iced::advanced::text::{self as advanced_text, Paragraph as _, Text};
use iced::advanced::widget::{tree, Tree, Widget};
use iced::advanced::{Clipboard, Shell};
use iced::widget::text::Span;
use iced::widget::{column, container, text};
use iced::{
    keyboard, mouse, Background, Color, Element, Event, Fill, Font, Length, Padding, Pixels, Point,
    Rectangle, Size, Theme,
};

use polaris_core::Document;

use super::{fonts, theme};

type Renderer = iced::Renderer;
type ParagraphOf = <Renderer as advanced_text::Renderer>::Paragraph;

const BODY_SIZE: f32 = 19.0;
const LINE_HEIGHT: f32 = 1.56;
const MEASURE: f32 = 600.0;

pub fn run(path: Option<PathBuf>) -> iced::Result {
    iced::application(move || App::boot(path.clone()), App::update, App::view)
        .title(|_: &App| "Polaris — editor widget spike".to_string())
        .theme(|app: &App| theme::theme(app.dark))
        .font(fonts::WRITING_REGULAR_BYTES)
        .font(fonts::WRITING_ITALIC_BYTES)
        .font(fonts::WRITING_SEMIBOLD_BYTES)
        .font(fonts::MONO_REGULAR_BYTES)
        .default_font(fonts::WRITING)
        .window_size(iced::Size::new(760.0, 940.0))
        .run()
}

struct App {
    doc: Document,
    lines: Vec<String>,
    text_version: u64,
    dark: bool,
    typewriter: bool,
    focus_dim: bool,
}

#[derive(Debug, Clone)]
enum Message {
    Insert(String),
    Enter,
    Backspace,
    Delete,
    Move(Motion, bool),
    Click { line: usize, byte: usize },
    ToggleTypewriter,
    ToggleFocus,
    Save,
    Undo,
    Redo,
}

#[derive(Debug, Clone, Copy)]
enum Motion {
    Left,
    Right,
    Up,
    Down,
    WordLeft,
    WordRight,
    Home,
    End,
}

impl App {
    fn boot(path: Option<PathBuf>) -> (Self, iced::Task<Message>) {
        let doc = match &path {
            Some(p) if p.exists() => Document::open(p).expect("file readable"),
            Some(p) => {
                let mut doc = Document::from_str("");
                doc.save_as(p).expect("file creatable");
                doc
            }
            None => Document::from_str(SAMPLE),
        };
        let mut app = Self {
            lines: Vec::new(),
            text_version: 0,
            doc,
            dark: matches!(dark_light::detect(), Ok(dark_light::Mode::Dark)),
            typewriter: true,
            focus_dim: false,
        };
        app.refresh_lines();
        (app, iced::Task::none())
    }

    fn refresh_lines(&mut self) {
        let text = self.doc.text();
        self.lines = text.split('\n').map(str::to_string).collect();
        self.text_version += 1;
    }

    fn caret(&self) -> (usize, usize) {
        let buffer = self.doc.buffer();
        let pos = self.doc.cursor().pos;
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

    fn update(&mut self, message: Message) {
        match message {
            Message::Insert(s) => {
                for c in s.chars() {
                    self.doc.insert_char(c);
                }
                self.refresh_lines();
            }
            Message::Enter => {
                self.doc.insert_newline();
                self.refresh_lines();
            }
            Message::Backspace => {
                self.doc.backspace();
                self.refresh_lines();
            }
            Message::Delete => {
                self.doc.delete_forward();
                self.refresh_lines();
            }
            Message::Move(motion, extend) => match motion {
                Motion::Left => self.doc.move_left(extend),
                Motion::Right => self.doc.move_right(extend),
                Motion::Up => self.doc.move_up(extend),
                Motion::Down => self.doc.move_down(extend),
                Motion::WordLeft => self.doc.move_word_left(extend),
                Motion::WordRight => self.doc.move_word_right(extend),
                Motion::Home => self.doc.move_line_start(extend),
                Motion::End => self.doc.move_line_end(extend),
            },
            Message::Click { line, byte } => {
                let buffer = self.doc.buffer();
                let line = line.min(buffer.len_lines().saturating_sub(1));
                let line_start = buffer.line_to_char(line);
                let text = buffer.line(line);
                let mut chars = 0;
                for (b, _) in text.char_indices() {
                    if b >= byte {
                        break;
                    }
                    chars += 1;
                }
                self.doc.set_cursor_pos(line_start + chars, false);
            }
            Message::ToggleTypewriter => self.typewriter = !self.typewriter,
            Message::ToggleFocus => self.focus_dim = !self.focus_dim,
            Message::Save => {
                let _ = self.doc.save();
            }
            Message::Undo => {
                if self.doc.undo() {
                    self.refresh_lines();
                }
            }
            Message::Redo => {
                if self.doc.redo() {
                    self.refresh_lines();
                }
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let t = theme::tokens(self.dark);
        let hint = format!(
            "spike · cmd+Y typewriter {} · cmd+G focus {} · cmd+S save · {} words",
            if self.typewriter { "on" } else { "off" },
            if self.focus_dim { "on" } else { "off" },
            self.doc.word_count(),
        );
        let chrome = text(hint).font(fonts::MONO).size(13).color(t.quiet);

        let canvas = Element::new(EditorCanvas {
            lines: &self.lines,
            caret: self.caret(),
            text_version: self.text_version,
            typewriter: self.typewriter,
            focus_dim: self.focus_dim,
            tokens: t,
        });

        let page = container(column![chrome, canvas].spacing(20))
            .max_width(MEASURE)
            .height(Fill);
        container(page)
            .style(move |_| container::Style {
                background: Some(Background::Color(t.bg)),
                ..container::Style::default()
            })
            .center_x(Fill)
            .height(Fill)
            .padding(Padding {
                top: 48.0,
                right: 32.0,
                bottom: 0.0,
                left: 32.0,
            })
            .into()
    }
}

// ---------------------------------------------------------------------------
// The widget
// ---------------------------------------------------------------------------

struct EditorCanvas<'a> {
    lines: &'a [String],
    /// (buffer line, byte column within line)
    caret: (usize, usize),
    text_version: u64,
    typewriter: bool,
    focus_dim: bool,
    tokens: theme::Tokens,
}

#[derive(Default)]
struct State {
    paragraphs: Vec<ParagraphOf>,
    heights: Vec<f32>,
    built_version: u64,
    built_width: f32,
    scroll: f32,
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

/// Markdown-quiet spans for one source line: prefix marks (`#`, `>`, `-`,
/// `1.`) in `quiet`, content styled. Inline marks would use the same
/// machinery (this is the capability proof, not full coverage).
fn line_spans(line: &str, t: theme::Tokens) -> Vec<Span<'static>> {
    let heading = line.len() - line.trim_start_matches('#').len();
    if (1..=6).contains(&heading) && line[heading..].starts_with(' ') {
        return vec![
            Span::new(line[..heading + 1].to_string()).color(t.quiet),
            Span::new(line[heading + 1..].to_string()).font(semibold()),
        ];
    }
    if let Some(rest) = line.strip_prefix("> ") {
        return vec![
            Span::new("> ".to_string()).color(t.quiet),
            Span::new(rest.to_string()).font(Font {
                style: iced::font::Style::Italic,
                ..fonts::WRITING
            }),
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

fn layout_paragraph(spans: &[Span<'static>], width: f32) -> ParagraphOf {
    ParagraphOf::with_spans(Text {
        content: spans,
        bounds: Size::new(width, f32::INFINITY),
        size: Pixels(BODY_SIZE),
        line_height: advanced_text::LineHeight::Relative(LINE_HEIGHT),
        font: fonts::WRITING,
        align_x: advanced_text::Alignment::Left,
        align_y: iced::alignment::Vertical::Top,
        shaping: advanced_text::Shaping::Advanced,
        wrapping: advanced_text::Wrapping::Word,
    })
}

/// Caret geometry inside its paragraph, via a probe layout of the text up
/// to the caret at the same width: (x, y of the caret's visual row).
fn caret_in_paragraph(line: &str, byte: usize, width: f32) -> (f32, f32) {
    let lh = line_height_px();
    let prefix = &line[..byte.min(line.len())];
    if prefix.is_empty() {
        return (0.0, 0.0);
    }
    let probe = ParagraphOf::with_text(Text {
        content: prefix,
        bounds: Size::new(width, f32::INFINITY),
        size: Pixels(BODY_SIZE),
        line_height: advanced_text::LineHeight::Relative(LINE_HEIGHT),
        font: fonts::WRITING,
        align_x: advanced_text::Alignment::Left,
        align_y: iced::alignment::Vertical::Top,
        shaping: advanced_text::Shaping::Advanced,
        wrapping: advanced_text::Wrapping::Word,
    });
    let rows = (probe.min_bounds().height / lh).round().max(1.0) as usize;
    let x = probe
        .grapheme_position(rows - 1, usize::MAX / 2)
        .map(|p| p.x)
        .unwrap_or(0.0);
    (x, (rows - 1) as f32 * lh)
}

impl State {
    fn rebuild(&mut self, canvas: &EditorCanvas<'_>, width: f32) {
        self.paragraphs = canvas
            .lines
            .iter()
            .map(|line| layout_paragraph(&line_spans(line, canvas.tokens), width))
            .collect();
        let lh = line_height_px();
        self.heights = self
            .paragraphs
            .iter()
            .map(|p| p.min_bounds().height.max(lh))
            .collect();
        self.built_version = canvas.text_version;
        self.built_width = width;
    }

    fn content_height(&self) -> f32 {
        self.heights.iter().sum()
    }
}

impl<Message: Clone + 'static> Widget<Message, Theme, Renderer> for EditorCanvas<'_>
where
    Message: From<SpikeEvent>,
{
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
            state.rebuild(self, size.width);
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

        let (caret_line, caret_byte) = self.caret;
        let y_above: f32 = state.heights[..caret_line.min(state.heights.len())]
            .iter()
            .sum();
        let empty = String::new();
        let caret_line_text = self.lines.get(caret_line).unwrap_or(&empty);
        let (cx, cy) = caret_in_paragraph(caret_line_text, caret_byte, state.built_width);

        let offset = if self.typewriter {
            // Hold the caret's row at 45% of the viewport, always.
            y_above + cy - bounds.height * 0.45
        } else {
            state
                .scroll
                .min((state.content_height() - bounds.height).max(0.0))
                .max(0.0)
        };

        renderer.with_layer(bounds, |renderer| {
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

            // The caret: a 2px bar in `star` — finally ours to draw.
            renderer.fill_quad(
                Quad {
                    bounds: Rectangle {
                        x: bounds.x + cx,
                        y: bounds.y - offset + y_above + cy + (lh - BODY_SIZE * 1.2) / 2.0,
                        width: 2.0,
                        height: BODY_SIZE * 1.2,
                    },
                    ..Quad::default()
                },
                t.star,
            );
        });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
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
            }) => {
                use keyboard::key::Named;
                use keyboard::Key;
                let word = modifiers.alt();
                let extend = modifiers.shift();
                let ev = match key.as_ref() {
                    Key::Named(Named::Backspace) => Some(SpikeEvent::Backspace),
                    Key::Named(Named::Delete) => Some(SpikeEvent::Delete),
                    Key::Named(Named::Enter) => Some(SpikeEvent::Enter),
                    Key::Named(Named::ArrowLeft) => Some(SpikeEvent::Move(
                        if word { Motion::WordLeft } else { Motion::Left },
                        extend,
                    )),
                    Key::Named(Named::ArrowRight) => Some(SpikeEvent::Move(
                        if word {
                            Motion::WordRight
                        } else {
                            Motion::Right
                        },
                        extend,
                    )),
                    Key::Named(Named::ArrowUp) => Some(SpikeEvent::Move(Motion::Up, extend)),
                    Key::Named(Named::ArrowDown) => Some(SpikeEvent::Move(Motion::Down, extend)),
                    Key::Named(Named::Home) => Some(SpikeEvent::Move(Motion::Home, extend)),
                    Key::Named(Named::End) => Some(SpikeEvent::Move(Motion::End, extend)),
                    Key::Character("y") if modifiers.command() => {
                        Some(SpikeEvent::ToggleTypewriter)
                    }
                    Key::Character("g") if modifiers.command() => Some(SpikeEvent::ToggleFocus),
                    Key::Character("s") if modifiers.command() => Some(SpikeEvent::Save),
                    Key::Character("z") if modifiers.command() && modifiers.shift() => {
                        Some(SpikeEvent::Redo)
                    }
                    Key::Character("z") if modifiers.command() => Some(SpikeEvent::Undo),
                    _ => text
                        .as_ref()
                        .filter(|_| !modifiers.command() && !modifiers.control())
                        .map(|t| t.to_string())
                        .filter(|t| t.chars().all(|c| !c.is_control()))
                        .map(SpikeEvent::Insert),
                };
                if let Some(ev) = ev {
                    shell.publish(Message::from(ev));
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(position) = cursor.position_in(bounds) {
                    // Which paragraph did we hit?
                    let (caret_line, caret_byte) = self.caret;
                    let empty = String::new();
                    let caret_text = self.lines.get(caret_line).unwrap_or(&empty);
                    let (_, cy) = caret_in_paragraph(caret_text, caret_byte, state.built_width);
                    let y_above: f32 = state.heights[..caret_line.min(state.heights.len())]
                        .iter()
                        .sum();
                    let offset = if self.typewriter {
                        y_above + cy - bounds.height * 0.45
                    } else {
                        state
                            .scroll
                            .min((state.content_height() - bounds.height).max(0.0))
                            .max(0.0)
                    };
                    let mut y = -offset;
                    for (i, paragraph) in state.paragraphs.iter().enumerate() {
                        let h = state.heights[i];
                        if position.y >= y && position.y < y + h {
                            let hit = paragraph
                                .hit_test(Point::new(position.x, position.y - y))
                                .map(|h| h.cursor())
                                .unwrap_or(self.lines[i].len());
                            shell.publish(Message::from(SpikeEvent::Click { line: i, byte: hit }));
                            shell.capture_event();
                            break;
                        }
                        y += h;
                    }
                }
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                if cursor.position_in(bounds).is_some() && !self.typewriter {
                    let dy = match delta {
                        mouse::ScrollDelta::Lines { y, .. } => y * line_height_px(),
                        mouse::ScrollDelta::Pixels { y, .. } => *y,
                    };
                    state.scroll = (state.scroll - dy)
                        .min((state.content_height() - bounds.height).max(0.0))
                        .max(0.0);
                    shell.request_redraw();
                }
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

/// Widget-to-app events; the app's Message implements `From<SpikeEvent>`.
#[derive(Debug, Clone)]
enum SpikeEvent {
    Insert(String),
    Enter,
    Backspace,
    Delete,
    Move(Motion, bool),
    Click { line: usize, byte: usize },
    ToggleTypewriter,
    ToggleFocus,
    Save,
    Undo,
    Redo,
}

impl From<SpikeEvent> for Message {
    fn from(ev: SpikeEvent) -> Self {
        match ev {
            SpikeEvent::Insert(s) => Message::Insert(s),
            SpikeEvent::Enter => Message::Enter,
            SpikeEvent::Backspace => Message::Backspace,
            SpikeEvent::Delete => Message::Delete,
            SpikeEvent::Move(m, e) => Message::Move(m, e),
            SpikeEvent::Click { line, byte } => Message::Click { line, byte },
            SpikeEvent::ToggleTypewriter => Message::ToggleTypewriter,
            SpikeEvent::ToggleFocus => Message::ToggleFocus,
            SpikeEvent::Save => Message::Save,
            SpikeEvent::Undo => Message::Undo,
            SpikeEvent::Redo => Message::Redo,
        }
    }
}

const SAMPLE: &str = "# The custom widget\n\nThis window is the Phase 2 spike: every glyph here is laid out by our own widget over polaris-core's rope — no text_editor in sight.\n\n## What to try\n\n- Type anywhere; click to place the caret\n- Cmd+Y toggles typewriter scrolling — the line you're writing holds still\n- Cmd+G dims everything but the paragraph you're in\n- The # marks and this list's dashes render quiet, like the mock\n\n> Markdown stays visible, but quiet.\n\nKeep typing past the bottom of the window to feel the typewriter hold the line steady.\n";
