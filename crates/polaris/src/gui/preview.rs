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

/// Width of the left gutter that holds the reading-pointer marker. Reserved
/// on every block so the prose never shifts as the pointer moves.
const GUTTER_W: f32 = 12.0;

/// One rendered block plus the source byte offset where it begins — the
/// offset lets the reading pointer map a block back to a caret position.
struct Block<'a, M> {
    element: Element<'a, M>,
    source: usize,
}

/// Render the markdown, drawing the reading-pointer marker beside the block
/// at `pointer` (if any). A mode switch, not a split.
pub fn view<'a, M: 'a>(source: &str, t: theme::Tokens, pointer: Option<usize>) -> Element<'a, M> {
    let rows = render_blocks::<M>(source, t)
        .into_iter()
        .enumerate()
        .map(|(i, block)| gutter_row(block.element, pointer == Some(i), t))
        .collect::<Vec<_>>();
    column(rows).spacing(16).width(Fill).into()
}

/// The source byte offset of each rendered block, in view order. Shares the
/// exact block segmentation of [`view`] (both drive [`render_blocks`]), so a
/// pointer index means the same block in both.
pub fn block_offsets(source: &str, t: theme::Tokens) -> Vec<usize> {
    render_blocks::<()>(source, t)
        .into_iter()
        .map(|block| block.source)
        .collect()
}

/// Wrap a block with its left gutter. The pointer block gets a slim accent
/// rule; every other block gets an equal, empty gutter so text stays put.
fn gutter_row<'a, M: 'a>(block: Element<'a, M>, marked: bool, t: theme::Tokens) -> Element<'a, M> {
    let marker: Element<'a, M> = if marked {
        // A quiet 2px rule at the far left edge, the caret's stand-in.
        container(space().width(2).height(Fill))
            .width(2)
            .style(move |_| container::Style {
                background: Some(Background::Color(iced::Color { a: 0.75, ..t.star })),
                border: iced::Border {
                    radius: 1.0.into(),
                    ..iced::Border::default()
                },
                ..container::Style::default()
            })
            .into()
    } else {
        space().width(2).into()
    };
    row![
        container(marker).width(GUTTER_W),
        container(block).width(Fill),
    ]
    .width(Fill)
    .into()
}

fn render_blocks<'a, M: 'a>(source: &str, t: theme::Tokens) -> Vec<Block<'a, M>> {
    let mut blocks: Vec<Block<'a, M>> = Vec::new();
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
    // The source offset where the block currently being built begins. Each
    // Start event that opens a block records its own; a pending paragraph
    // keeps `para_start` so a later flush tags it correctly.
    let mut para_start = 0usize;
    let mut heading_start = 0usize;
    let mut item_start = 0usize;
    let mut code_start = 0usize;
    let mut table_start = 0usize;

    let current_font = |bold: usize, emphasis: usize, in_quote: bool| -> Option<Font> {
        let base = fonts::WRITING;
        match (bold > 0, emphasis > 0 || in_quote) {
            (true, _) => Some(semibold(base)),
            (false, true) => Some(italic(base)),
            (false, false) => None,
        }
    };

    let flush_paragraph = |spans: &mut Vec<Span<'a>>,
                           blocks: &mut Vec<Block<'a, M>>,
                           t: theme::Tokens,
                           in_quote: bool,
                           start: usize| {
        if spans.is_empty() {
            return;
        }
        let body = rich_text(std::mem::take(spans))
            .font(fonts::WRITING)
            .size(BODY_SIZE)
            .line_height(text::LineHeight::Relative(1.6))
            .color(t.ink);
        let element: Element<'a, M> = if in_quote {
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
            .into()
        } else {
            body.into()
        };
        blocks.push(Block {
            element,
            source: start,
        });
    };

    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    for (event, range) in Parser::new_ext(source, options).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                flush_paragraph(&mut spans, &mut blocks, t, in_quote, para_start);
                heading = Some(level);
                heading_start = range.start;
            }
            Event::End(TagEnd::Heading(_)) => {
                let level = heading.take().unwrap_or(HeadingLevel::H3);
                let size = match level {
                    HeadingLevel::H1 => 27.0,
                    HeadingLevel::H2 => 22.5,
                    _ => 19.5,
                };
                if !spans.is_empty() {
                    blocks.push(Block {
                        element: rich_text(std::mem::take(&mut spans))
                            .font(semibold(fonts::WRITING))
                            .size(size)
                            .line_height(text::LineHeight::Relative(1.3))
                            .color(t.ink)
                            .into(),
                        source: heading_start,
                    });
                }
            }
            Event::Start(Tag::Paragraph) => {
                if item_spans.is_none() {
                    flush_paragraph(&mut spans, &mut blocks, t, in_quote, para_start);
                    para_start = range.start;
                }
            }
            Event::End(TagEnd::Paragraph) => {
                if item_spans.is_none() {
                    flush_paragraph(&mut spans, &mut blocks, t, in_quote, para_start);
                }
            }
            Event::Start(Tag::BlockQuote(_)) => {
                flush_paragraph(&mut spans, &mut blocks, t, in_quote, para_start);
                in_quote = true;
            }
            Event::End(TagEnd::BlockQuote) => {
                flush_paragraph(&mut spans, &mut blocks, t, in_quote, para_start);
                in_quote = false;
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                flush_paragraph(&mut spans, &mut blocks, t, in_quote, para_start);
                let lang = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(lang) => lang.to_string(),
                    pulldown_cmark::CodeBlockKind::Indented => String::new(),
                };
                code_block = Some((lang, String::new()));
                code_start = range.start;
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some((lang, code)) = code_block.take() {
                    blocks.push(Block {
                        element: code_block_element(&lang, code.trim_end(), t),
                        source: code_start,
                    });
                }
            }
            Event::Start(Tag::List(start)) => {
                flush_paragraph(&mut spans, &mut blocks, t, in_quote, para_start);
                list_stack.push(start);
            }
            Event::End(TagEnd::List(_)) => {
                list_stack.pop();
            }
            Event::Start(Tag::Item) => {
                item_spans = Some(Vec::new());
                item_start = range.start;
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
                    blocks.push(Block {
                        element: row![
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
                        source: item_start,
                    });
                }
            }
            Event::Rule => {
                flush_paragraph(&mut spans, &mut blocks, t, in_quote, para_start);
                blocks.push(Block {
                    element: container(space().width(Fill).height(1))
                        .width(Fill)
                        .height(1)
                        .style(move |_| container::Style {
                            background: Some(iced::Background::Color(t.whisper)),
                            ..container::Style::default()
                        })
                        .into(),
                    source: range.start,
                });
            }
            Event::Start(Tag::Table(_)) => {
                flush_paragraph(&mut spans, &mut blocks, t, in_quote, para_start);
                table = Some(Vec::new());
                table_start = range.start;
            }
            Event::End(TagEnd::Table) => {
                if let Some(rows) = table.take() {
                    blocks.push(Block {
                        element: table_element(rows, t),
                        source: table_start,
                    });
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
    flush_paragraph(&mut spans, &mut blocks, t, in_quote, para_start);

    blocks
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_offsets_are_ordered_and_land_on_each_block() {
        let src = "# Title\n\nFirst paragraph.\n\nSecond paragraph.\n\n- one\n- two\n";
        let offsets = block_offsets(src, theme::tokens(false));
        // heading, para, para, item, item = 5 blocks.
        assert_eq!(offsets.len(), 5);
        // Non-decreasing, and each points at where its block starts.
        assert!(offsets.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(offsets[0], src.find("# Title").unwrap());
        assert_eq!(offsets[1], src.find("First").unwrap());
        assert_eq!(offsets[2], src.find("Second").unwrap());
        assert_eq!(offsets[3], src.find("- one").unwrap());
        assert_eq!(offsets[4], src.find("- two").unwrap());
    }

    #[test]
    fn block_offsets_is_empty_for_blank_source() {
        assert!(block_offsets("", theme::tokens(false)).is_empty());
    }
}
