//! Substack — format-and-paste. No official publishing API, so v1 is honest:
//! render the markdown to HTML and hand it to the clipboard as a rich paste
//! (the app sets the HTML flavour); the writer pastes into a new post and
//! presses publish. Email-to-draft is a later upgrade (docs/PHASE4.md).

use crate::{body_html, Doc, Outcome, Target};
use anyhow::Result;
use async_trait::async_trait;

pub struct SubstackTarget;

#[async_trait]
impl Target for SubstackTarget {
    fn id(&self) -> &'static str {
        "substack"
    }

    fn label(&self) -> String {
        "Substack".to_string()
    }

    async fn publish(&self, doc: Doc<'_>) -> Result<Outcome> {
        Ok(Outcome::Clipboard {
            hint: "paste into a new Substack post".to_string(),
            html: Some(body_html(doc.markdown)),
            // Plain fallback for editors that ignore the rich flavour.
            text: doc.markdown.to_string(),
        })
    }
}
