//! The snapshot store: a sidecar `.polaris/<file-name>/` directory next to
//! the document. Content-addressed zstd objects + a linear JSON manifest.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use unicode_segmentation::UnicodeSegmentation;

/// At most one auto snapshot per this interval of active editing.
pub const AUTO_INTERVAL_MS: u64 = 10 * 60 * 1000;
/// Autos are pruned only beyond BOTH limits (kept if recent OR among last N).
pub const AUTO_KEEP_COUNT: usize = 50;
pub const AUTO_KEEP_MS: u64 = 7 * 24 * 3600 * 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Marked,
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub kind: Kind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub created_ms: u64,
    pub object: String,
    pub words: usize,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Manifest {
    entries: Vec<Entry>,
}

pub struct DraftStore {
    /// `<doc dir>/.polaris/<file-name>`
    root: PathBuf,
    manifest: Manifest,
}

fn sidecar_root(doc_path: &Path) -> io::Result<PathBuf> {
    // Bare relative names ("draft.md") have an empty parent — anchor to the
    // working directory so the sidecar still lands next to the file.
    let doc_path = std::path::absolute(doc_path)?;
    let dir = doc_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "document has no directory"))?;
    let name = doc_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "document has no name"))?;
    Ok(dir.join(".polaris").join(name))
}

fn hash16(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    digest[..8].iter().map(|b| format!("{b:02x}")).collect()
}

fn count_words(text: &str) -> usize {
    text.unicode_words().count()
}

impl DraftStore {
    pub fn for_document(doc_path: &Path) -> io::Result<Self> {
        let root = sidecar_root(doc_path)?;
        let manifest = match fs::read_to_string(root.join("manifest.json")) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => Manifest::default(),
        };
        Ok(Self { root, manifest })
    }

    pub fn entries(&self) -> &[Entry] {
        &self.manifest.entries
    }

    pub fn last(&self) -> Option<&Entry> {
        self.manifest.entries.last()
    }

    pub fn marked_count(&self) -> usize {
        self.manifest
            .entries
            .iter()
            .filter(|e| e.kind == Kind::Marked)
            .count()
    }

    /// Snapshot `text`. Autos are skipped (Ok(None)) when nothing changed
    /// since the last snapshot; marked drafts always add a manifest entry
    /// (the body is deduplicated by content hash either way).
    pub fn snapshot(
        &mut self,
        text: &str,
        kind: Kind,
        name: Option<String>,
        now_ms: u64,
    ) -> io::Result<Option<Entry>> {
        let object = hash16(text);
        if kind == Kind::Auto && self.last().is_some_and(|e| e.object == object) {
            return Ok(None);
        }
        self.ensure_dirs()?;
        let path = self.object_path(&object);
        if !path.exists() {
            let compressed = zstd::encode_all(text.as_bytes(), 3).map_err(io::Error::other)?;
            fs::write(&path, compressed)?;
        }
        let entry = Entry {
            id: format!("d-{now_ms:013x}-{}", &object[..4]),
            kind,
            name,
            created_ms: now_ms,
            object,
            words: count_words(text),
        };
        self.manifest.entries.push(entry.clone());
        self.save_manifest()?;
        Ok(Some(entry))
    }

    pub fn load(&self, entry: &Entry) -> io::Result<String> {
        let bytes = fs::read(self.object_path(&entry.object))?;
        let raw = zstd::decode_all(&bytes[..]).map_err(io::Error::other)?;
        String::from_utf8(raw).map_err(io::Error::other)
    }

    /// Policy: one auto per [`AUTO_INTERVAL_MS`] of active editing.
    pub fn should_auto_snapshot(&self, now_ms: u64) -> bool {
        match self
            .manifest
            .entries
            .iter()
            .rev()
            .find(|e| e.kind == Kind::Auto)
        {
            Some(auto) => now_ms.saturating_sub(auto.created_ms) >= AUTO_INTERVAL_MS,
            None => true,
        }
    }

    /// Prune autos beyond BOTH limits (older than 7 days AND not among the
    /// newest 50 autos). Marked drafts are never pruned. Unreferenced
    /// objects are deleted. Returns how many entries were removed.
    pub fn prune(&mut self, now_ms: u64) -> io::Result<usize> {
        let auto_ids: Vec<&str> = self
            .manifest
            .entries
            .iter()
            .filter(|e| e.kind == Kind::Auto)
            .map(|e| e.id.as_str())
            .collect();
        let protected_from = auto_ids.len().saturating_sub(AUTO_KEEP_COUNT);
        let keep_ids: std::collections::HashSet<String> = auto_ids[protected_from..]
            .iter()
            .map(|s| s.to_string())
            .collect();

        let before = self.manifest.entries.len();
        self.manifest.entries.retain(|e| {
            e.kind == Kind::Marked
                || keep_ids.contains(&e.id)
                || now_ms.saturating_sub(e.created_ms) < AUTO_KEEP_MS
        });
        let removed = before - self.manifest.entries.len();
        if removed > 0 {
            let referenced: std::collections::HashSet<&str> = self
                .manifest
                .entries
                .iter()
                .map(|e| e.object.as_str())
                .collect();
            if let Ok(dir) = fs::read_dir(self.root.join("objects")) {
                for file in dir.flatten() {
                    let name = file.file_name();
                    let object = name.to_string_lossy();
                    let object = object.trim_end_matches(".zst");
                    if !referenced.contains(object) {
                        let _ = fs::remove_file(file.path());
                    }
                }
            }
            self.save_manifest()?;
        }
        Ok(removed)
    }

    /// Follow a document rename: move the sidecar history to the new name.
    /// Best-effort — a missing old store or an existing new one is a no-op.
    pub fn migrate(old_doc: &Path, new_doc: &Path) -> io::Result<()> {
        let old = sidecar_root(old_doc)?;
        let new = sidecar_root(new_doc)?;
        if old == new || !old.exists() || new.exists() {
            return Ok(());
        }
        if let Some(parent) = new.parent() {
            fs::create_dir_all(parent)?;
            let gitignore = parent.join(".gitignore");
            if !gitignore.exists() {
                fs::write(&gitignore, "*\n")?;
            }
        }
        fs::rename(&old, &new)
    }

    fn ensure_dirs(&self) -> io::Result<()> {
        fs::create_dir_all(self.root.join("objects"))?;
        fs::create_dir_all(self.root.join("notes"))?; // reserved: docs/AI.md
        if let Some(polaris_dir) = self.root.parent() {
            let gitignore = polaris_dir.join(".gitignore");
            if !gitignore.exists() {
                fs::write(&gitignore, "*\n")?;
            }
        }
        Ok(())
    }

    fn object_path(&self, object: &str) -> PathBuf {
        self.root.join("objects").join(format!("{object}.zst"))
    }

    fn save_manifest(&self) -> io::Result<()> {
        let json = serde_json::to_string_pretty(&self.manifest).map_err(io::Error::other)?;
        fs::write(self.root.join("manifest.json"), json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_doc(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("polaris-drafts-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir.join("doc.md")
    }

    #[test]
    fn bare_relative_paths_anchor_to_the_working_directory() {
        // `polaris draft.md` passes a parentless relative path; the store
        // must still resolve (this was a real bug: Cmd+M showed the
        // untitled hint on a named file).
        let store = DraftStore::for_document(Path::new("bare-relative.md")).unwrap();
        assert!(store.root.is_absolute());
        assert!(store.root.ends_with(".polaris/bare-relative.md"));
    }

    #[test]
    fn snapshot_load_roundtrip_and_dedup() {
        let doc = temp_doc("roundtrip");
        let mut store = DraftStore::for_document(&doc).unwrap();

        let e1 = store
            .snapshot(
                "draft one — café",
                Kind::Marked,
                Some("Draft 1".into()),
                1000,
            )
            .unwrap()
            .unwrap();
        assert_eq!(store.load(&e1).unwrap(), "draft one — café");
        assert_eq!(e1.words, 3);

        // Marking again without edits: new entry, same object, one body.
        let e2 = store
            .snapshot(
                "draft one — café",
                Kind::Marked,
                Some("Draft 2".into()),
                2000,
            )
            .unwrap()
            .unwrap();
        assert_eq!(e1.object, e2.object);
        assert_eq!(store.entries().len(), 2);
        let objects = fs::read_dir(doc.parent().unwrap().join(".polaris/doc.md/objects"))
            .unwrap()
            .count();
        assert_eq!(objects, 1, "content-addressed: one body");

        // Reopen from disk: manifest persists.
        let reopened = DraftStore::for_document(&doc).unwrap();
        assert_eq!(reopened.entries().len(), 2);
        assert_eq!(reopened.entries()[0].name.as_deref(), Some("Draft 1"));
    }

    #[test]
    fn autos_skip_unchanged_and_respect_interval() {
        let doc = temp_doc("autos");
        let mut store = DraftStore::for_document(&doc).unwrap();

        assert!(store.should_auto_snapshot(0));
        assert!(store.snapshot("v1", Kind::Auto, None, 0).unwrap().is_some());
        assert!(store
            .snapshot("v1", Kind::Auto, None, 50)
            .unwrap()
            .is_none());
        assert!(!store.should_auto_snapshot(AUTO_INTERVAL_MS - 1));
        assert!(store.should_auto_snapshot(AUTO_INTERVAL_MS));
        assert!(store
            .snapshot("v2", Kind::Auto, None, AUTO_INTERVAL_MS)
            .unwrap()
            .is_some());
    }

    #[test]
    fn gitignore_self_ignores_the_sidecar() {
        let doc = temp_doc("gitignore");
        let mut store = DraftStore::for_document(&doc).unwrap();
        store.snapshot("x", Kind::Auto, None, 0).unwrap();
        let gitignore = doc.parent().unwrap().join(".polaris/.gitignore");
        assert_eq!(fs::read_to_string(gitignore).unwrap(), "*\n");
    }

    #[test]
    fn prune_keeps_marked_and_recent_autos() {
        let doc = temp_doc("prune");
        let mut store = DraftStore::for_document(&doc).unwrap();

        store
            .snapshot("keep me", Kind::Marked, Some("Draft 1".into()), 0)
            .unwrap();
        // 60 old autos (distinct content), all beyond 7 days at prune time.
        for i in 0..60u64 {
            store
                .snapshot(&format!("auto {i}"), Kind::Auto, None, i)
                .unwrap();
        }
        let now = AUTO_KEEP_MS + 1_000_000;
        let removed = store.prune(now).unwrap();
        assert_eq!(removed, 10, "kept the newest 50 autos");
        assert_eq!(store.marked_count(), 1, "marked survive everything");

        // Recent autos survive regardless of count position.
        let mut store2 = DraftStore::for_document(&temp_doc("prune2")).unwrap();
        for i in 0..60u64 {
            store2
                .snapshot(&format!("auto {i}"), Kind::Auto, None, now + i)
                .unwrap();
        }
        assert_eq!(store2.prune(now + 100).unwrap(), 0, "all within 7 days");
    }

    #[test]
    fn migrate_follows_a_rename() {
        let doc = temp_doc("migrate");
        let mut store = DraftStore::for_document(&doc).unwrap();
        let entry = store
            .snapshot("history", Kind::Marked, Some("Draft 1".into()), 0)
            .unwrap()
            .unwrap();

        let new_doc = doc.parent().unwrap().join("renamed.md");
        DraftStore::migrate(&doc, &new_doc).unwrap();

        let moved = DraftStore::for_document(&new_doc).unwrap();
        assert_eq!(moved.entries().len(), 1);
        assert_eq!(moved.load(&entry).unwrap(), "history");
        assert!(DraftStore::for_document(&doc).unwrap().entries().is_empty());
    }
}
