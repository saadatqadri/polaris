//! The publish layer: markdown in, platform out.
//!
//! Every publish destination — an API (Notion), a file on disk (Hugo), a
//! clipboard hand-off (Substack paste, LinkedIn) — implements one
//! [`Target`] trait. The document reaches a target as a plain markdown
//! [`Doc`]; targets never see Polaris's buffer, the GUI, or the FFI. What a
//! target produced comes back as an [`Outcome`]. Adding a destination is
//! adding one adapter module here (see `notion.rs`, `hugo.rs`); the binary's
//! registry turns config into a live target list. Design: `docs/PHASE4.md`.

mod hugo;
mod notion;

pub use hugo::HugoTarget;
pub use notion::NotionTarget;

use anyhow::Result;
use async_trait::async_trait;
use std::path::{Path, PathBuf};

/// A document handed to a target: markdown plus just enough context to name
/// it. Borrowed and cheap — the caller owns the strings.
pub struct Doc<'a> {
    pub markdown: &'a str,
    /// The document's on-disk path, if it has been saved. Used to derive a
    /// title when the markdown has no H1.
    pub source_path: Option<&'a Path>,
}

impl<'a> Doc<'a> {
    pub fn new(markdown: &'a str, source_path: Option<&'a Path>) -> Self {
        Self {
            markdown,
            source_path,
        }
    }

    /// The document's title: the first ATX H1 (`# …`) if there is one, else
    /// the source file's stem. `None` only for an unsaved buffer with no H1.
    pub fn title(&self) -> Option<String> {
        for line in self.markdown.lines() {
            if let Some(rest) = line.trim_start().strip_prefix("# ") {
                let title = rest.trim();
                if !title.is_empty() {
                    return Some(title.to_string());
                }
            }
        }
        self.source_path
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
    }
}

/// What a target produced when asked to publish.
#[derive(Debug, Clone)]
pub enum Outcome {
    /// A live URL — an API target (Notion).
    Url(String),
    /// A file written to disk — a file target (Hugo, later HTML/PDF).
    Path(PathBuf),
    /// The user finishes the last step by hand: `body` is ready for the
    /// clipboard (the *app* places it there — this crate links no clipboard
    /// backend), and `hint` says what to do with it. Used by API-less
    /// targets (Substack paste, LinkedIn) in a later phase.
    Clipboard { hint: String, body: String },
}

/// A publish destination. One implementation per platform; the binary builds
/// a `Vec<Box<dyn Target>>` from config and drives it behind Cmd+D.
#[async_trait]
pub trait Target: Send + Sync {
    /// Stable id used in config and on the CLI: `"notion"`, `"hugo"`, …
    fn id(&self) -> &'static str;
    /// Human label for the picker: `"Notion"`, `"Hugo — saadatqadri.com"`.
    fn label(&self) -> String;
    /// Publish the document. File and clipboard targets do no real awaiting;
    /// API targets do the network I/O here.
    async fn publish(&self, doc: Doc<'_>) -> Result<Outcome>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_prefers_the_first_h1() {
        let md = "some intro line\n\n# The Real Title\n\nbody\n\n# Later Heading";
        let doc = Doc::new(md, None);
        assert_eq!(doc.title().as_deref(), Some("The Real Title"));
    }

    #[test]
    fn title_falls_back_to_the_file_stem() {
        let path = PathBuf::from("/tmp/my-post.md");
        let doc = Doc::new("no heading here\n", Some(&path));
        assert_eq!(doc.title().as_deref(), Some("my-post"));
    }

    #[test]
    fn title_is_none_for_an_unsaved_headingless_buffer() {
        assert_eq!(Doc::new("just prose\n", None).title(), None);
    }

    #[test]
    fn a_bare_hash_is_not_a_title() {
        // "#" without a space, and an empty "# " both fail CommonMark's H1.
        let doc = Doc::new("#nope\n# \n# yes\n", None);
        assert_eq!(doc.title().as_deref(), Some("yes"));
    }
}
