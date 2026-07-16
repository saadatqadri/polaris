//! The Notion adapter — the first target, wrapping the existing
//! `polaris-notion` API client behind the [`Target`] trait.

use crate::{Doc, Outcome, Target};
use anyhow::Result;
use async_trait::async_trait;
use polaris_notion::{NotionClient, PublishMode};

/// Publishes to a single configured Notion page. Append semantics match the
/// desktop's long-standing Cmd+D behaviour; the headless `polaris deploy`
/// command still owns the append/replace choice directly.
pub struct NotionTarget {
    client: NotionClient,
    page_id: String,
}

impl NotionTarget {
    pub fn new(token: impl Into<String>, page_id: impl Into<String>) -> Self {
        Self {
            client: NotionClient::new(token),
            page_id: page_id.into(),
        }
    }
}

#[async_trait]
impl Target for NotionTarget {
    fn id(&self) -> &'static str {
        "notion"
    }

    fn label(&self) -> String {
        "Notion".to_string()
    }

    async fn publish(&self, doc: Doc<'_>) -> Result<Outcome> {
        let url = self
            .client
            .deploy(doc.markdown, &self.page_id, PublishMode::Append)
            .await?;
        Ok(Outcome::Url(url))
    }
}
