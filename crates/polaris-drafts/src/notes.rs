//! Inline notes — a writer reviewing their own work in preview. Stored beside
//! the document in the same `.polaris/<name>/notes/` sidecar as drafts: local,
//! travels with the folder, never a server. Each note anchors to a source span
//! plus a quote of the anchored text, so it survives edits by best-effort
//! re-anchoring; a note whose quote has vanished is kept and flagged detached,
//! never silently dropped. Design: docs/PHASE4.md decision #7.
//!
//! This is deliberately the same margin an AI critique pass would use
//! (docs/AI.md): built for the human reviewer first, and — because notes live
//! only here in the sidecar, never in the buffer — no machine words can reach
//! the document by construction.

use crate::store::sidecar_root;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NoteState {
    Open,
    Resolved,
}

fn open_state() -> NoteState {
    NoteState::Open
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    /// Byte offset of the anchor in the source. Kept current by re-anchoring.
    pub start: usize,
    /// Byte offset just past the anchor (`start + quote.len()`). Informational.
    pub end: usize,
    /// The anchored source text — used to relocate the note after edits.
    pub quote: String,
    pub body: String,
    pub created_ms: u64,
    #[serde(default = "open_state")]
    pub state: NoteState,
    /// The quote could not be found on the last re-anchor: kept, shown
    /// detached at its stale position.
    #[serde(default)]
    pub detached: bool,
}

impl Note {
    pub fn is_open(&self) -> bool {
        self.state == NoteState::Open
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct NotesFile {
    notes: Vec<Note>,
}

pub struct NoteStore {
    /// `<doc dir>/.polaris/<file-name>`
    root: PathBuf,
    notes: Vec<Note>,
}

fn hash4(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    digest[..2].iter().map(|b| format!("{b:02x}")).collect()
}

/// The occurrence of `needle` in `haystack` nearest to `origin` (byte offset).
fn nearest(haystack: &str, needle: &str, origin: usize) -> Option<usize> {
    haystack
        .match_indices(needle)
        .map(|(i, _)| i)
        .min_by_key(|&i| i.abs_diff(origin))
}

impl NoteStore {
    pub fn for_document(doc_path: &Path) -> io::Result<Self> {
        let root = sidecar_root(doc_path)?;
        let notes = match fs::read_to_string(root.join("notes").join("live.json")) {
            Ok(s) => serde_json::from_str::<NotesFile>(&s)
                .unwrap_or_default()
                .notes,
            Err(_) => Vec::new(),
        };
        Ok(Self { root, notes })
    }

    pub fn notes(&self) -> &[Note] {
        &self.notes
    }

    pub fn is_empty(&self) -> bool {
        self.notes.is_empty()
    }

    /// Add a note anchored to `[start, end)` with `quote` as its anchor text.
    /// Returns the new note's id.
    pub fn add(
        &mut self,
        start: usize,
        end: usize,
        quote: String,
        body: String,
        now_ms: u64,
    ) -> io::Result<String> {
        let id = format!("n-{now_ms:013x}-{}", hash4(&format!("{start}:{body}")));
        self.notes.push(Note {
            id: id.clone(),
            start,
            end,
            quote,
            body,
            created_ms: now_ms,
            state: NoteState::Open,
            detached: false,
        });
        self.save()?;
        Ok(id)
    }

    pub fn edit(&mut self, id: &str, body: String) -> io::Result<()> {
        if let Some(note) = self.notes.iter_mut().find(|n| n.id == id) {
            note.body = body;
        }
        self.save()
    }

    /// Flip a note between open and resolved (kept either way).
    pub fn toggle_resolved(&mut self, id: &str) -> io::Result<()> {
        if let Some(note) = self.notes.iter_mut().find(|n| n.id == id) {
            note.state = match note.state {
                NoteState::Open => NoteState::Resolved,
                NoteState::Resolved => NoteState::Open,
            };
        }
        self.save()
    }

    pub fn remove(&mut self, id: &str) -> io::Result<()> {
        self.notes.retain(|n| n.id != id);
        self.save()
    }

    /// Relocate each note to the nearest occurrence of its quote in `source`:
    /// unchanged text keeps the note in place, moved text carries it along, a
    /// vanished quote detaches it (kept, flagged). Returns whether anything
    /// changed, so the caller can persist only when needed.
    pub fn reanchor(&mut self, source: &str) -> bool {
        let mut changed = false;
        for note in &mut self.notes {
            if note.quote.is_empty() {
                continue;
            }
            match nearest(source, &note.quote, note.start) {
                Some(pos) => {
                    if note.detached || note.start != pos {
                        changed = true;
                    }
                    note.start = pos;
                    note.end = pos + note.quote.len();
                    note.detached = false;
                }
                None => {
                    if !note.detached {
                        changed = true;
                    }
                    note.detached = true;
                }
            }
        }
        changed
    }

    pub fn save(&self) -> io::Result<()> {
        self.ensure_dir()?;
        let file = NotesFile {
            notes: self.notes.clone(),
        };
        let json = serde_json::to_string_pretty(&file).map_err(io::Error::other)?;
        fs::write(self.notes_dir().join("live.json"), json)
    }

    /// Freeze the current notes alongside a marked draft: `notes/<draft-id>
    /// .json`. The draft's text is frozen, so these never drift — the exact
    /// hook an AI critique pass would attach to (docs/AI.md).
    pub fn freeze_to(&self, draft_id: &str) -> io::Result<()> {
        if self.notes.is_empty() {
            return Ok(());
        }
        self.ensure_dir()?;
        let file = NotesFile {
            notes: self.notes.clone(),
        };
        let json = serde_json::to_string_pretty(&file).map_err(io::Error::other)?;
        fs::write(self.notes_dir().join(format!("{draft_id}.json")), json)
    }

    fn notes_dir(&self) -> PathBuf {
        self.root.join("notes")
    }

    fn ensure_dir(&self) -> io::Result<()> {
        fs::create_dir_all(self.notes_dir())?;
        if let Some(polaris_dir) = self.root.parent() {
            let gitignore = polaris_dir.join(".gitignore");
            if !gitignore.exists() {
                let _ = fs::write(&gitignore, "*\n");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_doc(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("polaris-notes-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir.join("doc.md")
    }

    #[test]
    fn add_persists_and_reloads() {
        let doc = temp_doc("persist");
        let mut store = NoteStore::for_document(&doc).unwrap();
        store
            .add(10, 20, "the quote".into(), "weak transition".into(), 1000)
            .unwrap();

        let reopened = NoteStore::for_document(&doc).unwrap();
        assert_eq!(reopened.notes().len(), 1);
        assert_eq!(reopened.notes()[0].body, "weak transition");
        assert_eq!(reopened.notes()[0].quote, "the quote");
        assert!(reopened.notes()[0].is_open());
    }

    #[test]
    fn reanchor_keeps_moves_and_detaches() {
        let doc = temp_doc("reanchor");
        let mut store = NoteStore::for_document(&doc).unwrap();
        let src = "alpha beta gamma";
        let start = src.find("beta").unwrap();
        store
            .add(start, start + 4, "beta".into(), "note".into(), 0)
            .unwrap();

        // Unchanged source: stays put, no change reported.
        assert!(!store.reanchor(src));
        assert_eq!(store.notes()[0].start, start);
        assert!(!store.notes()[0].detached);

        // Text pushed later in the document: the note follows.
        let moved = "xxxxx alpha beta gamma";
        assert!(store.reanchor(moved));
        assert_eq!(store.notes()[0].start, moved.find("beta").unwrap());
        assert!(!store.notes()[0].detached);

        // Quote gone: detached, but kept.
        assert!(store.reanchor("alpha gamma"));
        assert!(store.notes()[0].detached);
        assert_eq!(store.notes().len(), 1);

        // Quote returns: re-attaches.
        assert!(store.reanchor("beta again"));
        assert!(!store.notes()[0].detached);
    }

    #[test]
    fn reanchor_picks_the_nearest_occurrence() {
        let doc = temp_doc("nearest");
        let mut store = NoteStore::for_document(&doc).unwrap();
        // Two occurrences of "x"; anchor near the second.
        let src = "x .......... x";
        let second = src.rfind('x').unwrap();
        store.add(second, second + 1, "x".into(), "n".into(), 0).unwrap();
        store.reanchor(src);
        assert_eq!(store.notes()[0].start, second, "keeps the nearer match");
    }

    #[test]
    fn toggle_resolve_and_remove() {
        let doc = temp_doc("state");
        let mut store = NoteStore::for_document(&doc).unwrap();
        let id = store.add(0, 3, "abc".into(), "cut this".into(), 0).unwrap();

        store.toggle_resolved(&id).unwrap();
        assert_eq!(store.notes()[0].state, NoteState::Resolved);
        store.toggle_resolved(&id).unwrap();
        assert!(store.notes()[0].is_open());

        store.remove(&id).unwrap();
        assert!(store.is_empty());
        assert!(NoteStore::for_document(&doc).unwrap().is_empty());
    }

    #[test]
    fn freeze_writes_a_draft_scoped_copy() {
        let doc = temp_doc("freeze");
        let mut store = NoteStore::for_document(&doc).unwrap();
        store.add(0, 3, "abc".into(), "keep".into(), 0).unwrap();
        store.freeze_to("d-000-aa").unwrap();

        let frozen = doc.parent().unwrap().join(".polaris/doc.md/notes/d-000-aa.json");
        assert!(frozen.exists());
        assert!(fs::read_to_string(frozen).unwrap().contains("keep"));
    }
}
