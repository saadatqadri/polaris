//! Word-level diff between two document states, for the drafts browser.

use similar::{ChangeTag, TextDiff};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffKind {
    Equal,
    /// Present in `old` only.
    Removed,
    /// Present in `new` only.
    Added,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffSpan {
    pub kind: DiffKind,
    pub text: String,
}

/// Word-level diff spans. Concatenating Equal+Removed reproduces `old`;
/// Equal+Added reproduces `new`.
pub fn word_diff(old: &str, new: &str) -> Vec<DiffSpan> {
    let diff = TextDiff::from_words(old, new);
    let mut spans: Vec<DiffSpan> = Vec::new();
    for change in diff.iter_all_changes() {
        let kind = match change.tag() {
            ChangeTag::Equal => DiffKind::Equal,
            ChangeTag::Delete => DiffKind::Removed,
            ChangeTag::Insert => DiffKind::Added,
        };
        match spans.last_mut() {
            Some(last) if last.kind == kind => last.text.push_str(change.value()),
            _ => spans.push(DiffSpan {
                kind,
                text: change.value().to_string(),
            }),
        }
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    fn join(spans: &[DiffSpan], keep: DiffKind) -> String {
        spans
            .iter()
            .filter(|s| s.kind == DiffKind::Equal || s.kind == keep)
            .map(|s| s.text.as_str())
            .collect()
    }

    #[test]
    fn diff_reconstructs_both_sides() {
        let old = "the quick brown fox jumps";
        let new = "the slow brown fox jumps high";
        let spans = word_diff(old, new);
        assert_eq!(join(&spans, DiffKind::Removed), old);
        assert_eq!(join(&spans, DiffKind::Added), new);
    }

    #[test]
    fn identical_texts_are_one_equal_span() {
        let spans = word_diff("same text", "same text");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].kind, DiffKind::Equal);
    }

    #[test]
    fn diff_handles_multiline_and_unicode() {
        let old = "café one\ntwo";
        let new = "café one\nthree — two";
        let spans = word_diff(old, new);
        assert_eq!(join(&spans, DiffKind::Removed), old);
        assert_eq!(join(&spans, DiffKind::Added), new);
        assert!(spans.iter().any(|s| s.kind == DiffKind::Added));
    }
}
