//! Markdown → a simple block model the native front-ends render. Parsing
//! lives here (pulldown-cmark); presentation is the platform's. The iOS
//! preview view walks these blocks.

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// An inline run with its emphasis. `code` spans render monospaced.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct PreviewSpan {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
}

/// A rendered block. Headings carry a level (1–6); list items carry their
/// marker; code blocks carry raw text + language.
#[derive(uniffi::Enum, Debug, PartialEq, Eq)]
pub enum PreviewBlock {
    Heading {
        level: u8,
        spans: Vec<PreviewSpan>,
    },
    Paragraph {
        spans: Vec<PreviewSpan>,
    },
    ListItem {
        ordered: bool,
        marker: String,
        spans: Vec<PreviewSpan>,
    },
    Quote {
        spans: Vec<PreviewSpan>,
    },
    Code {
        language: String,
        text: String,
    },
    Rule,
}

/// Parse `markdown` into preview blocks.
pub fn render(markdown: &str) -> Vec<PreviewBlock> {
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    let mut blocks: Vec<PreviewBlock> = Vec::new();
    let mut spans: Vec<PreviewSpan> = Vec::new();
    let mut bold = 0usize;
    let mut italic = 0usize;
    let mut heading: Option<HeadingLevel> = None;
    let mut in_quote = false;
    let mut code: Option<(String, String)> = None;
    let mut list_stack: Vec<Option<u64>> = Vec::new();
    let mut in_item = false;

    let push_text = |spans: &mut Vec<PreviewSpan>, text: &str, bold, italic, code| {
        spans.push(PreviewSpan {
            text: text.to_string(),
            bold,
            italic,
            code,
        });
    };

    for event in Parser::new_ext(markdown, options) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => heading = Some(level),
            Event::End(TagEnd::Heading(_)) => {
                let level = heading.take().unwrap_or(HeadingLevel::H3);
                blocks.push(PreviewBlock::Heading {
                    level: level as u8,
                    spans: std::mem::take(&mut spans),
                });
            }
            Event::Start(Tag::Paragraph) => {}
            Event::End(TagEnd::Paragraph) => {
                if in_item {
                    // handled at item end
                } else if in_quote {
                    blocks.push(PreviewBlock::Quote {
                        spans: std::mem::take(&mut spans),
                    });
                } else if !spans.is_empty() {
                    blocks.push(PreviewBlock::Paragraph {
                        spans: std::mem::take(&mut spans),
                    });
                }
            }
            Event::Start(Tag::BlockQuote(_)) => in_quote = true,
            Event::End(TagEnd::BlockQuote) => in_quote = false,
            Event::Start(Tag::CodeBlock(kind)) => {
                let lang = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(l) => l.to_string(),
                    pulldown_cmark::CodeBlockKind::Indented => String::new(),
                };
                code = Some((lang, String::new()));
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some((language, text)) = code.take() {
                    blocks.push(PreviewBlock::Code {
                        language,
                        text: text.trim_end().to_string(),
                    });
                }
            }
            Event::Start(Tag::List(start)) => list_stack.push(start),
            Event::End(TagEnd::List(_)) => {
                list_stack.pop();
            }
            Event::Start(Tag::Item) => {
                in_item = true;
                spans.clear();
            }
            Event::End(TagEnd::Item) => {
                in_item = false;
                let (ordered, marker) = match list_stack.last_mut() {
                    Some(Some(n)) => {
                        let m = format!("{n}.");
                        *n += 1;
                        (true, m)
                    }
                    _ => (false, "–".to_string()),
                };
                blocks.push(PreviewBlock::ListItem {
                    ordered,
                    marker,
                    spans: std::mem::take(&mut spans),
                });
            }
            Event::Rule => blocks.push(PreviewBlock::Rule),
            Event::Start(Tag::Strong) => bold += 1,
            Event::End(TagEnd::Strong) => bold = bold.saturating_sub(1),
            Event::Start(Tag::Emphasis) => italic += 1,
            Event::End(TagEnd::Emphasis) => italic = italic.saturating_sub(1),
            Event::Text(t) => {
                if let Some((_, body)) = code.as_mut() {
                    body.push_str(&t);
                } else {
                    push_text(&mut spans, &t, bold > 0, italic > 0, false);
                }
            }
            Event::Code(c) => push_text(&mut spans, &c, false, false, true),
            Event::SoftBreak | Event::HardBreak => {
                if let Some((_, body)) = code.as_mut() {
                    body.push('\n');
                } else {
                    push_text(&mut spans, " ", false, false, false);
                }
            }
            _ => {}
        }
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_paragraphs_and_emphasis() {
        let b = render("# Title\n\nplain **bold** and *italic*.");
        assert_eq!(
            b[0],
            PreviewBlock::Heading {
                level: 1,
                spans: vec![PreviewSpan {
                    text: "Title".into(),
                    bold: false,
                    italic: false,
                    code: false
                }]
            }
        );
        let PreviewBlock::Paragraph { spans } = &b[1] else {
            panic!("expected paragraph")
        };
        assert!(spans.iter().any(|s| s.text == "bold" && s.bold));
        assert!(spans.iter().any(|s| s.text == "italic" && s.italic));
    }

    #[test]
    fn lists_quotes_code_rules() {
        let b = render("- one\n- two\n\n> quote\n\n```rust\nfn x() {}\n```\n\n---");
        assert!(matches!(
            b[0],
            PreviewBlock::ListItem { ordered: false, .. }
        ));
        assert!(matches!(&b[2], PreviewBlock::Quote { .. }));
        assert!(
            matches!(&b[3], PreviewBlock::Code { language, text } if language == "rust" && text == "fn x() {}")
        );
        assert_eq!(b[4], PreviewBlock::Rule);

        let ol = render("1. a\n2. b");
        assert!(matches!(&ol[0], PreviewBlock::ListItem { marker, .. } if marker == "1."));
        assert!(matches!(&ol[1], PreviewBlock::ListItem { marker, .. } if marker == "2."));
    }
}
