//! polaris-core — the UI-agnostic document core for Polaris.
//!
//! Owns the rope buffer, grapheme-aware cursors and selection, grouped
//! undo/redo, file binding + autosave policy, word count, and the
//! smart-punctuation transforms. Front-ends (the iced GUI, the CLI) drive
//! [`Document`] and render its state; nothing here knows about UI or clocks.

pub mod buffer;
pub mod cursor;
pub mod document;
pub mod history;
pub mod typography;

pub use buffer::Buffer;
pub use cursor::Cursor;
pub use document::{AutosaveTimer, Document};
