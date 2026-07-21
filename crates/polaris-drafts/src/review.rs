//! Accept/reject editing (docs/PHASE4.md P3). Import an edited copy of a
//! document, see it as a sequence of *changes* over the current text, and
//! accept or reject each one — Draft-the-app's collaboration model, no server.
//!
//! Built on the same word diff the drafts browser uses. A change is proposed,
//! never auto-applied: `applied()` is a pure fold the caller adopts through
//! the core as one undo group. The buffer is only ever written by the human
//! pressing accept — the same invariant docs/AI.md demands, generalised to any
//! external edit.

use crate::diff::{word_diff, DiffKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    /// Incoming adds text not in the current document.
    Insert,
    /// Incoming removes text that is in the current document.
    Delete,
    /// Incoming swaps some current text for different text.
    Replace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Pending,
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub kind: ChangeKind,
    /// The current document's text at this spot (empty for an insertion).
    pub old: String,
    /// The incoming edit's text (empty for a deletion).
    pub new: String,
    pub decision: Decision,
}

/// One reviewable document: fixed context interleaved with proposed changes.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Item {
    Context(String),
    Change(Change),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Review {
    items: Vec<Item>,
}

impl Review {
    /// Diff the current document against an imported edited copy into a
    /// sequence of changes, each Pending until decided.
    pub fn from_texts(current: &str, incoming: &str) -> Self {
        let mut items = Vec::new();
        let spans = word_diff(current, incoming);
        let mut i = 0;
        while i < spans.len() {
            if spans[i].kind == DiffKind::Equal {
                items.push(Item::Context(spans[i].text.clone()));
                i += 1;
                continue;
            }
            // Gather a contiguous run of removed/added spans into one change.
            let mut old = String::new();
            let mut new = String::new();
            while i < spans.len() && spans[i].kind != DiffKind::Equal {
                match spans[i].kind {
                    DiffKind::Removed => old.push_str(&spans[i].text),
                    DiffKind::Added => new.push_str(&spans[i].text),
                    DiffKind::Equal => unreachable!("guarded by the while condition"),
                }
                i += 1;
            }
            let kind = if old.is_empty() {
                ChangeKind::Insert
            } else if new.is_empty() {
                ChangeKind::Delete
            } else {
                ChangeKind::Replace
            };
            items.push(Item::Change(Change {
                kind,
                old,
                new,
                decision: Decision::Pending,
            }));
        }
        Review { items }
    }

    pub fn changes(&self) -> impl Iterator<Item = &Change> {
        self.items.iter().filter_map(|it| match it {
            Item::Change(c) => Some(c),
            Item::Context(_) => None,
        })
    }

    pub fn change_count(&self) -> usize {
        self.changes().count()
    }

    pub fn is_empty(&self) -> bool {
        self.change_count() == 0
    }

    pub fn accepted_count(&self) -> usize {
        self.changes()
            .filter(|c| c.decision == Decision::Accepted)
            .count()
    }

    pub fn decided_count(&self) -> usize {
        self.changes()
            .filter(|c| c.decision != Decision::Pending)
            .count()
    }

    /// The nth change (by change order, skipping context).
    pub fn change(&self, index: usize) -> Option<&Change> {
        self.changes().nth(index)
    }

    /// Decide the nth change.
    pub fn set_decision(&mut self, index: usize, decision: Decision) {
        let mut n = 0;
        for item in &mut self.items {
            if let Item::Change(change) = item {
                if n == index {
                    change.decision = decision;
                    return;
                }
                n += 1;
            }
        }
    }

    /// Decide every change at once (accept-all / reject-all).
    pub fn set_all(&mut self, decision: Decision) {
        for item in &mut self.items {
            if let Item::Change(change) = item {
                change.decision = decision;
            }
        }
    }

    /// The document text for the current decisions: accepted changes take the
    /// incoming text, rejected and still-pending changes keep the current
    /// text. All-pending reproduces the current document exactly; all-accepted
    /// reproduces the imported copy exactly.
    pub fn applied(&self) -> String {
        let mut out = String::new();
        for item in &self.items {
            match item {
                Item::Context(text) => out.push_str(text),
                Item::Change(change) => match change.decision {
                    Decision::Accepted => out.push_str(&change.new),
                    Decision::Rejected | Decision::Pending => out.push_str(&change.old),
                },
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_pending_reproduces_current_all_accepted_reproduces_incoming() {
        let current = "the quick brown fox jumps";
        let incoming = "the slow brown fox leaps high";
        let mut review = Review::from_texts(current, incoming);

        assert_eq!(review.applied(), current, "pending keeps current");
        review.set_all(Decision::Accepted);
        assert_eq!(review.applied(), incoming, "accepted yields incoming");
        review.set_all(Decision::Rejected);
        assert_eq!(review.applied(), current, "rejected keeps current");
    }

    #[test]
    fn change_kinds_are_classified() {
        // Pure insertion.
        let ins = Review::from_texts("a c", "a b c");
        assert_eq!(ins.change_count(), 1);
        assert_eq!(ins.change(0).unwrap().kind, ChangeKind::Insert);

        // Pure deletion.
        let del = Review::from_texts("a b c", "a c");
        assert_eq!(del.change(0).unwrap().kind, ChangeKind::Delete);

        // Replacement.
        let rep = Review::from_texts("a b c", "a X c");
        assert_eq!(rep.change(0).unwrap().kind, ChangeKind::Replace);
    }

    #[test]
    fn per_change_decisions_compose() {
        let current = "one two three four";
        let incoming = "one TWO three FOUR";
        let mut review = Review::from_texts(current, incoming);
        assert_eq!(review.change_count(), 2);

        // Accept the first change, reject the second.
        review.set_decision(0, Decision::Accepted);
        review.set_decision(1, Decision::Rejected);
        assert_eq!(review.applied(), "one TWO three four");
        assert_eq!(review.accepted_count(), 1);
        assert_eq!(review.decided_count(), 2);
    }

    #[test]
    fn identical_documents_have_no_changes() {
        let review = Review::from_texts("same words here", "same words here");
        assert!(review.is_empty());
        assert_eq!(review.applied(), "same words here");
    }
}
