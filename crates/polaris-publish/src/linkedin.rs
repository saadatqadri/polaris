//! LinkedIn — format-and-copy. Posting is partner-gated, so v1 flattens the
//! markdown to the plain text LinkedIn's composer accepts, using the Unicode
//! sans-serif-bold letters people reach for (LinkedIn strips real markup) and
//! `•` bullets. Copied to the clipboard as plain text; the writer pastes.

use crate::{Doc, Outcome, Target};
use anyhow::Result;
use async_trait::async_trait;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

pub struct LinkedinTarget;

#[async_trait]
impl Target for LinkedinTarget {
    fn id(&self) -> &'static str {
        "linkedin"
    }

    fn label(&self) -> String {
        "LinkedIn".to_string()
    }

    async fn publish(&self, doc: Doc<'_>) -> Result<Outcome> {
        Ok(Outcome::Clipboard {
            hint: "paste into a new LinkedIn post".to_string(),
            html: None,
            text: to_linkedin(doc.markdown),
        })
    }
}

/// Flatten markdown to LinkedIn-ready plain text: headings and `**bold**`
/// become Unicode sans-serif bold, list items get `•`, links read as
/// `text (url)`, and everything else is stripped to its words.
pub fn to_linkedin(markdown: &str) -> String {
    let options = Options::ENABLE_STRIKETHROUGH;
    let mut out = String::new();
    let mut bold = 0usize;
    let mut pending_link: Option<String> = None;

    for event in Parser::new_ext(markdown, options) {
        match event {
            Event::Start(Tag::Heading { .. }) | Event::Start(Tag::Strong) => bold += 1,
            Event::End(TagEnd::Heading(_)) => {
                bold = bold.saturating_sub(1);
                out.push_str("\n\n");
            }
            Event::End(TagEnd::Strong) => bold = bold.saturating_sub(1),
            Event::End(TagEnd::Paragraph) => out.push_str("\n\n"),
            Event::Start(Tag::Item) => out.push_str("• "),
            Event::End(TagEnd::Item) => out.push('\n'),
            Event::Start(Tag::Link { dest_url, .. }) => pending_link = Some(dest_url.to_string()),
            Event::End(TagEnd::Link) => {
                if let Some(url) = pending_link.take() {
                    out.push_str(&format!(" ({url})"));
                }
            }
            Event::Text(text) | Event::Code(text) => {
                if bold > 0 {
                    out.extend(text.chars().map(sans_bold));
                } else {
                    out.push_str(&text);
                }
            }
            Event::SoftBreak => out.push(' '),
            Event::HardBreak => out.push('\n'),
            Event::Rule => out.push_str("—\n\n"),
            _ => {}
        }
    }

    // Collapse the runs of blank lines the block ends leave behind.
    let mut result = String::new();
    let mut blanks = 0;
    for line in out.trim_end().lines() {
        if line.trim().is_empty() {
            blanks += 1;
            if blanks < 2 {
                result.push('\n');
            }
        } else {
            blanks = 0;
            result.push_str(line);
            result.push('\n');
        }
    }
    result.trim_end().to_string()
}

/// Map ASCII letters/digits to their Mathematical Sans-Serif Bold cousins,
/// leaving everything else (punctuation, accents, emoji) untouched.
fn sans_bold(c: char) -> char {
    let cp = c as u32;
    let mapped = match c {
        'A'..='Z' => 0x1D5D4 + (cp - 'A' as u32),
        'a'..='z' => 0x1D5EE + (cp - 'a' as u32),
        '0'..='9' => 0x1D7EC + (cp - '0' as u32),
        _ => return c,
    };
    char::from_u32(mapped).unwrap_or(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bold_becomes_sans_serif_bold() {
        let out = to_linkedin("A **bold** word.");
        assert!(out.contains('\u{1D5EF}'), "expected sans-bold 'b'"); // 𝗯
        assert!(out.contains("A "));
        assert!(out.contains(" word."));
    }

    #[test]
    fn headings_bold_and_lists_bullet() {
        let out = to_linkedin("# Title\n\n- one\n- two\n");
        assert!(out.starts_with('\u{1D5E7}'), "heading 'T' is sans-bold"); // 𝗧
        assert!(out.contains("• one"));
        assert!(out.contains("• two"));
    }

    #[test]
    fn links_read_as_text_then_url() {
        let out = to_linkedin("See [my site](https://x.io).");
        assert!(out.contains("my site (https://x.io)"));
    }

    #[test]
    fn blank_line_runs_collapse() {
        let out = to_linkedin("one\n\npara two\n");
        assert!(!out.contains("\n\n\n"));
        assert!(out.contains("one"));
        assert!(out.contains("para two"));
    }
}
