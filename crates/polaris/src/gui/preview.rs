//! Preview mode: the same column, markdown rendered — one voice (Newsreader)
//! to iced widgets via pulldown-cmark. A mode switch, not a split.

use iced::widget::text::Span;
use iced::widget::{column, container, rich_text, row, scrollable, space, text};
use iced::{font, Background, Element, Fill, Font};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use super::{fonts, theme};

const BODY_SIZE: f32 = 19.0;

fn italic(base: Font) -> Font {
    Font {
        style: font::Style::Italic,
        ..base
    }
}

fn semibold(base: Font) -> Font {
    Font {
        weight: font::Weight::Semibold,
        ..base
    }
}

pub fn view<'a, M: 'a>(source: &str, t: theme::Tokens) -> Element<'a, M> {
    let mut blocks: Vec<Element<'a, M>> = Vec::new();
    let mut spans: Vec<Span<'a>> = Vec::new();
    let mut bold = 0usize;
    let mut emphasis = 0usize;
    let mut in_quote = false;
    let mut heading: Option<HeadingLevel> = None;
    let mut code_block: Option<(String, String)> = None; // (language, body)
    let mut list_stack: Vec<Option<u64>> = Vec::new();
    let mut item_spans: Option<Vec<Span<'a>>> = None;
    // Table state: rows of cells of spans; the first row is the header.
    let mut table: Option<Vec<Vec<Vec<Span<'a>>>>> = None;
    let mut table_cell: Option<Vec<Span<'a>>> = None;

    let current_font = |bold: usize, emphasis: usize, in_quote: bool| -> Option<Font> {
        let base = fonts::WRITING;
        match (bold > 0, emphasis > 0 || in_quote) {
            (true, _) => Some(semibold(base)),
            (false, true) => Some(italic(base)),
            (false, false) => None,
        }
    };

    let flush_paragraph = |spans: &mut Vec<Span<'a>>,
                           blocks: &mut Vec<Element<'a, M>>,
                           t: theme::Tokens,
                           in_quote: bool| {
        if spans.is_empty() {
            return;
        }
        let body = rich_text(std::mem::take(spans))
            .font(fonts::WRITING)
            .size(BODY_SIZE)
            .line_height(text::LineHeight::Relative(1.6))
            .color(t.ink);
        if in_quote {
            blocks.push(
                row![
                    container(space().width(3).height(Fill))
                        .width(3)
                        .style(move |_| container::Style {
                            background: Some(iced::Background::Color(t.whisper)),
                            ..container::Style::default()
                        }),
                    container(body).width(Fill),
                ]
                .spacing(16)
                .into(),
            );
        } else {
            blocks.push(body.into());
        }
    };

    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    for event in Parser::new_ext(source, options) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                flush_paragraph(&mut spans, &mut blocks, t, in_quote);
                heading = Some(level);
            }
            Event::End(TagEnd::Heading(_)) => {
                let level = heading.take().unwrap_or(HeadingLevel::H3);
                let size = match level {
                    HeadingLevel::H1 => 27.0,
                    HeadingLevel::H2 => 22.5,
                    _ => 19.5,
                };
                if !spans.is_empty() {
                    blocks.push(
                        rich_text(std::mem::take(&mut spans))
                            .font(semibold(fonts::WRITING))
                            .size(size)
                            .line_height(text::LineHeight::Relative(1.3))
                            .color(t.ink)
                            .into(),
                    );
                }
            }
            Event::Start(Tag::Paragraph) => {
                if item_spans.is_none() {
                    flush_paragraph(&mut spans, &mut blocks, t, in_quote);
                }
            }
            Event::End(TagEnd::Paragraph) => {
                if item_spans.is_none() {
                    flush_paragraph(&mut spans, &mut blocks, t, in_quote);
                }
            }
            Event::Start(Tag::BlockQuote(_)) => {
                flush_paragraph(&mut spans, &mut blocks, t, in_quote);
                in_quote = true;
            }
            Event::End(TagEnd::BlockQuote) => {
                flush_paragraph(&mut spans, &mut blocks, t, in_quote);
                in_quote = false;
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                flush_paragraph(&mut spans, &mut blocks, t, in_quote);
                let lang = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(lang) => lang.to_string(),
                    pulldown_cmark::CodeBlockKind::Indented => String::new(),
                };
                code_block = Some((lang, String::new()));
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some((lang, code)) = code_block.take() {
                    blocks.push(code_block_element(&lang, code.trim_end(), t));
                }
            }
            Event::Start(Tag::List(start)) => {
                flush_paragraph(&mut spans, &mut blocks, t, in_quote);
                list_stack.push(start);
            }
            Event::End(TagEnd::List(_)) => {
                list_stack.pop();
            }
            Event::Start(Tag::Item) => {
                item_spans = Some(Vec::new());
            }
            Event::End(TagEnd::Item) => {
                if let Some(item) = item_spans.take() {
                    let marker = match list_stack.last_mut() {
                        Some(Some(n)) => {
                            let m = format!("{n}.");
                            *n += 1;
                            m
                        }
                        _ => "–".to_string(),
                    };
                    blocks.push(
                        row![
                            container(
                                text(marker)
                                    .font(fonts::WRITING)
                                    .size(BODY_SIZE)
                                    .line_height(text::LineHeight::Relative(1.6))
                                    .color(t.quiet)
                            )
                            .width(30),
                            container(
                                rich_text(item)
                                    .font(fonts::WRITING)
                                    .size(BODY_SIZE)
                                    .line_height(text::LineHeight::Relative(1.6))
                                    .color(t.ink)
                            )
                            .width(Fill),
                        ]
                        .into(),
                    );
                }
            }
            Event::Rule => {
                flush_paragraph(&mut spans, &mut blocks, t, in_quote);
                blocks.push(
                    container(space().width(Fill).height(1))
                        .width(Fill)
                        .height(1)
                        .style(move |_| container::Style {
                            background: Some(iced::Background::Color(t.whisper)),
                            ..container::Style::default()
                        })
                        .into(),
                );
            }
            Event::Start(Tag::Table(_)) => {
                flush_paragraph(&mut spans, &mut blocks, t, in_quote);
                table = Some(Vec::new());
            }
            Event::End(TagEnd::Table) => {
                if let Some(rows) = table.take() {
                    blocks.push(table_element(rows, t));
                }
            }
            Event::Start(Tag::TableHead) | Event::Start(Tag::TableRow) => {
                if let Some(rows) = table.as_mut() {
                    rows.push(Vec::new());
                }
            }
            Event::End(TagEnd::TableHead) | Event::End(TagEnd::TableRow) => {}
            Event::Start(Tag::TableCell) => {
                table_cell = Some(Vec::new());
            }
            Event::End(TagEnd::TableCell) => {
                if let (Some(cell), Some(rows)) = (table_cell.take(), table.as_mut()) {
                    if let Some(row) = rows.last_mut() {
                        row.push(cell);
                    }
                }
            }
            Event::Start(Tag::Strong) => bold += 1,
            Event::End(TagEnd::Strong) => bold = bold.saturating_sub(1),
            Event::Start(Tag::Emphasis) => emphasis += 1,
            Event::End(TagEnd::Emphasis) => emphasis = emphasis.saturating_sub(1),
            Event::Text(chunk) => {
                if let Some((_, code)) = code_block.as_mut() {
                    code.push_str(&chunk);
                } else if let Some(cell) = table_cell.as_mut() {
                    let mut s = Span::new(chunk.to_string());
                    if let Some(f) = current_font(bold, emphasis, false) {
                        s = s.font(f);
                    }
                    cell.push(s);
                } else {
                    let mut s = Span::new(chunk.to_string());
                    if let Some(f) = current_font(bold, emphasis, in_quote) {
                        s = s.font(f);
                    }
                    match item_spans.as_mut() {
                        Some(item) => item.push(s),
                        None => spans.push(s),
                    }
                }
            }
            Event::Code(code) => {
                let s = Span::new(code.to_string()).font(fonts::MONO).size(15.0);
                if let Some(cell) = table_cell.as_mut() {
                    cell.push(s);
                } else {
                    match item_spans.as_mut() {
                        Some(item) => item.push(s),
                        None => spans.push(s),
                    }
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some((_, code)) = code_block.as_mut() {
                    code.push('\n');
                } else {
                    let s = Span::new(" ");
                    match item_spans.as_mut() {
                        Some(item) => item.push(s),
                        None => spans.push(s),
                    }
                }
            }
            _ => {}
        }
    }
    flush_paragraph(&mut spans, &mut blocks, t, in_quote);

    column(blocks).spacing(16).width(Fill).into()
}

/// A fenced code block: never wrapped (ASCII art and diagram source stay
/// intact), horizontally scrollable, with a quiet language label.
fn code_block_element<'a, M: 'a>(lang: &str, code: &str, t: theme::Tokens) -> Element<'a, M> {
    let body = scrollable(
        text(code.to_string())
            .font(fonts::MONO)
            .size(14)
            .line_height(text::LineHeight::Relative(1.55))
            .color(t.ink)
            .wrapping(text::Wrapping::None),
    )
    .direction(scrollable::Direction::Horizontal(
        scrollable::Scrollbar::new()
            .width(4)
            .margin(2)
            .scroller_width(4),
    ))
    .width(Fill)
    .style(move |theme: &iced::Theme, status| {
        let mut style = scrollable::default(theme, status);
        style.container = container::Style::default();
        style.horizontal_rail.background = None;
        style.horizontal_rail.border = iced::Border::default();
        style.horizontal_rail.scroller.background = Background::Color(t.quiet);
        style.horizontal_rail.scroller.border = iced::Border {
            radius: 2.0.into(),
            ..iced::Border::default()
        };
        style
    });

    let block = container(body)
        .width(Fill)
        .padding(14)
        .style(move |_| container::Style {
            background: Some(Background::Color(iced::Color {
                a: 0.5,
                ..t.whisper
            })),
            border: iced::Border {
                radius: 4.0.into(),
                ..iced::Border::default()
            },
            ..container::Style::default()
        });

    if lang.is_empty() {
        block.into()
    } else {
        column![
            text(lang.to_string())
                .font(fonts::MONO)
                .size(11)
                .color(t.quiet),
            block
        ]
        .spacing(4)
        .into()
    }
}

/// A table: header row semibold over a whisper hairline, equal-width
/// columns, prose wrapping inside cells.
fn table_element<'a, M: 'a>(rows: Vec<Vec<Vec<Span<'a>>>>, t: theme::Tokens) -> Element<'a, M> {
    let mut out: Vec<Element<'a, M>> = Vec::new();
    let header = !rows.is_empty();
    for (i, cells) in rows.into_iter().enumerate() {
        let is_header = header && i == 0;
        let mut line: Vec<Element<'a, M>> = Vec::new();
        for cell in cells {
            let mut body = rich_text(cell)
                .size(15)
                .line_height(text::LineHeight::Relative(1.5))
                .color(t.ink);
            if is_header {
                body = body.font(semibold(fonts::WRITING));
            } else {
                body = body.font(fonts::WRITING);
            }
            line.push(
                container(body)
                    .width(iced::Length::FillPortion(1))
                    .padding(iced::Padding {
                        top: 4.0,
                        right: 10.0,
                        bottom: 4.0,
                        left: 0.0,
                    })
                    .into(),
            );
        }
        out.push(iced::widget::Row::from_vec(line).into());
        if is_header {
            out.push(
                container(space().width(Fill).height(1))
                    .width(Fill)
                    .height(1)
                    .style(move |_| container::Style {
                        background: Some(Background::Color(t.whisper)),
                        ..container::Style::default()
                    })
                    .into(),
            );
        }
    }
    column(out).spacing(4).width(Fill).into()
}
