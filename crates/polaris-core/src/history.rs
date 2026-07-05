//! Undo/redo as an operation log. Operations are grouped into human-sized
//! chunks (roughly words); [`crate::document::Document`] decides where group
//! boundaries fall and callers may force one via `commit_undo_group` (e.g. on
//! a typing pause).

use crate::buffer::Buffer;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditOp {
    Insert { at: usize, text: String },
    Remove { at: usize, text: String },
}

impl EditOp {
    fn inverted(&self) -> EditOp {
        match self {
            EditOp::Insert { at, text } => EditOp::Remove {
                at: *at,
                text: text.clone(),
            },
            EditOp::Remove { at, text } => EditOp::Insert {
                at: *at,
                text: text.clone(),
            },
        }
    }

    fn apply(&self, buffer: &mut Buffer) {
        match self {
            EditOp::Insert { at, text } => buffer.insert(*at, text),
            EditOp::Remove { at, text } => {
                buffer.remove(*at..*at + text.chars().count());
            }
        }
    }
}

#[derive(Debug, Clone)]
struct EditGroup {
    ops: Vec<EditOp>,
    cursor_before: usize,
    cursor_after: usize,
}

#[derive(Debug, Clone, Default)]
pub struct History {
    undo: Vec<EditGroup>,
    redo: Vec<EditGroup>,
    open: Option<EditGroup>,
}

impl History {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an already-applied operation into the open group.
    pub fn record(&mut self, op: EditOp, cursor_before: usize, cursor_after: usize) {
        self.redo.clear();
        match &mut self.open {
            Some(group) => {
                group.ops.push(op);
                group.cursor_after = cursor_after;
            }
            None => {
                self.open = Some(EditGroup {
                    ops: vec![op],
                    cursor_before,
                    cursor_after,
                });
            }
        }
    }

    /// The last operation of the open group, if any (used for grouping
    /// heuristics).
    pub fn open_last_op(&self) -> Option<&EditOp> {
        self.open.as_ref().and_then(|g| g.ops.last())
    }

    /// Close the open group so the next edit starts a new one.
    pub fn commit(&mut self) {
        if let Some(group) = self.open.take() {
            self.undo.push(group);
        }
    }

    /// Undo one group. Returns the cursor position to restore.
    pub fn undo(&mut self, buffer: &mut Buffer) -> Option<usize> {
        self.commit();
        let group = self.undo.pop()?;
        for op in group.ops.iter().rev() {
            op.inverted().apply(buffer);
        }
        let pos = group.cursor_before;
        self.redo.push(group);
        Some(pos)
    }

    /// Redo one group. Returns the cursor position to restore.
    pub fn redo(&mut self, buffer: &mut Buffer) -> Option<usize> {
        self.commit();
        let group = self.redo.pop()?;
        for op in &group.ops {
            op.apply(buffer);
        }
        let pos = group.cursor_after;
        self.undo.push(group);
        Some(pos)
    }

    pub fn can_undo(&self) -> bool {
        self.open.is_some() || !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_redo_roundtrip() {
        let mut buffer = Buffer::new();
        let mut history = History::new();

        buffer.insert(0, "hi");
        history.record(
            EditOp::Insert {
                at: 0,
                text: "hi".into(),
            },
            0,
            2,
        );

        assert_eq!(history.undo(&mut buffer), Some(0));
        assert_eq!(buffer.to_string(), "");
        assert_eq!(history.redo(&mut buffer), Some(2));
        assert_eq!(buffer.to_string(), "hi");
    }

    #[test]
    fn grouped_ops_undo_together() {
        let mut buffer = Buffer::new();
        let mut history = History::new();

        for (i, c) in "abc".chars().enumerate() {
            buffer.insert(i, &c.to_string());
            history.record(
                EditOp::Insert {
                    at: i,
                    text: c.to_string(),
                },
                i,
                i + 1,
            );
        }

        assert_eq!(history.undo(&mut buffer), Some(0));
        assert_eq!(buffer.to_string(), "");
    }

    #[test]
    fn commit_splits_groups() {
        let mut buffer = Buffer::new();
        let mut history = History::new();

        buffer.insert(0, "a");
        history.record(
            EditOp::Insert {
                at: 0,
                text: "a".into(),
            },
            0,
            1,
        );
        history.commit();
        buffer.insert(1, "b");
        history.record(
            EditOp::Insert {
                at: 1,
                text: "b".into(),
            },
            1,
            2,
        );

        assert_eq!(history.undo(&mut buffer), Some(1));
        assert_eq!(buffer.to_string(), "a");
        assert_eq!(history.undo(&mut buffer), Some(0));
        assert_eq!(buffer.to_string(), "");
    }

    #[test]
    fn new_edit_clears_redo() {
        let mut buffer = Buffer::new();
        let mut history = History::new();

        buffer.insert(0, "a");
        history.record(
            EditOp::Insert {
                at: 0,
                text: "a".into(),
            },
            0,
            1,
        );
        history.undo(&mut buffer);
        assert!(history.can_redo());

        buffer.insert(0, "b");
        history.record(
            EditOp::Insert {
                at: 0,
                text: "b".into(),
            },
            0,
            1,
        );
        assert!(!history.can_redo());
    }

    #[test]
    fn remove_op_undoes_to_insert() {
        let mut buffer = Buffer::from_str("héllo");
        let mut history = History::new();

        let removed = buffer.remove(1..2);
        history.record(
            EditOp::Remove {
                at: 1,
                text: removed,
            },
            2,
            1,
        );

        assert_eq!(buffer.to_string(), "hllo");
        history.undo(&mut buffer);
        assert_eq!(buffer.to_string(), "héllo");
    }
}
