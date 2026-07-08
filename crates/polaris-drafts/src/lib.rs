//! polaris-drafts — writer-friendly version control (docs/DRAFTS.md).
//!
//! A content-addressed snapshot store beside the document — no git: the
//! history is linear and merge-free, marked drafts are kept forever, autos
//! are pruned. No UI, no clock (callers pass unix milliseconds), fully
//! testable.

pub mod diff;
pub mod store;

pub use diff::{word_diff, DiffKind, DiffSpan};
pub use store::{DraftStore, Entry, Kind, AUTO_INTERVAL_MS};
