//! polaris-ffi — the Swift/iOS bridge to `polaris-core` (docs/IOS.md, i0).
//!
//! A thin uniffi wrapper: the native front-end owns file I/O and the text
//! surface; this exposes the in-memory document engine (edit, undo, word
//! count) as a Swift object. No `std::fs` here — persistence is the host's
//! job, matching the desktop's untitled-buffer path.

use std::sync::{Arc, Mutex};

use polaris_core::Document;

uniffi::setup_scaffolding!();

/// A live document the Swift layer drives. Interior `Mutex` because uniffi
/// objects are shared as `Arc` and their methods take `&self`.
#[derive(uniffi::Object)]
pub struct PolarisDocument {
    inner: Mutex<Document>,
}

#[uniffi::export]
impl PolarisDocument {
    /// Open an in-memory document seeded with `text` (the host loaded it
    /// from the Files app / iCloud).
    #[uniffi::constructor]
    pub fn new(text: String) -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(Document::from_str(&text)),
        })
    }

    /// The full document text, for the host to persist.
    pub fn text(&self) -> String {
        self.inner.lock().unwrap().text()
    }

    pub fn insert(&self, text: String) {
        self.inner.lock().unwrap().insert_str(&text);
    }

    pub fn newline(&self) {
        self.inner.lock().unwrap().insert_newline();
    }

    pub fn backspace(&self) {
        self.inner.lock().unwrap().backspace();
    }

    /// Undo one grouped edit; `true` if anything changed.
    pub fn undo(&self) -> bool {
        self.inner.lock().unwrap().undo()
    }

    pub fn redo(&self) -> bool {
        self.inner.lock().unwrap().redo()
    }

    pub fn word_count(&self) -> u32 {
        self.inner.lock().unwrap().word_count() as u32
    }

    /// Caret position as a char index.
    pub fn cursor(&self) -> u64 {
        self.inner.lock().unwrap().cursor().pos as u64
    }

    /// Move the caret to a char index (the host text view drives this).
    pub fn set_cursor(&self, pos: u64) {
        self.inner
            .lock()
            .unwrap()
            .set_cursor_pos(pos as usize, false);
    }

    pub fn len_chars(&self) -> u64 {
        self.inner.lock().unwrap().buffer().len_chars() as u64
    }

    pub fn is_dirty(&self) -> bool {
        self.inner.lock().unwrap().is_dirty()
    }
}

#[cfg(test)]
mod tests {
    use super::PolarisDocument;

    #[test]
    fn document_round_trips_through_the_bridge_type() {
        let doc = PolarisDocument::new("héllo".to_string());
        doc.set_cursor(doc.len_chars()); // caret to end, then append
        doc.insert(" 👋 world".to_string());
        assert_eq!(doc.text(), "héllo 👋 world");
        assert_eq!(doc.word_count(), 2); // the emoji isn't a word
        assert!(doc.is_dirty());

        // Grouped undo removes the whole inserted run.
        assert!(doc.undo());
        assert_eq!(doc.text(), "héllo");
        assert!(doc.redo());
        assert_eq!(doc.text(), "héllo 👋 world");
    }

    #[test]
    fn editing_primitives() {
        let doc = PolarisDocument::new(String::new());
        doc.insert("one".to_string());
        doc.newline();
        doc.insert("two".to_string());
        assert_eq!(doc.text(), "one\ntwo");
        doc.backspace();
        assert_eq!(doc.text(), "one\ntw");
        assert_eq!(doc.cursor(), 6);
    }
}
