use anyhow::{Context, Result, anyhow};
use reqwest::Client;
use serde_json::{json, Value};
use crate::config::Config;
use super::blocks::markdown_to_notion_blocks;

const NOTION_API_VERSION: &str = "2022-06-28";
const NOTION_API_BASE: &str = "https://api.notion.com/v1";

pub enum PublishMode {
    Append,
    Replace,
}

pub struct NotionClient {
    client: Client,
    token: String,
}

impl NotionClient {
    pub fn new(config: &Config) -> Result<Self> {
        let token = config.notion.token.as_ref()
            .ok_or_else(|| anyhow!("Notion token not configured. Please set it in ~/.polaris.toml"))?
            .clone();

        Ok(Self {
            client: Client::new(),
            token,
        })
    }

    pub async fn deploy(
        &self,
        markdown: &str,
        page_id: &str,
        mode: PublishMode,
    ) -> Result<String> {
        let blocks = markdown_to_notion_blocks(markdown);

        match mode {
            PublishMode::Replace => {
                // First, delete existing blocks
                self.clear_page_blocks(page_id).await?;
                // Then append new blocks
                self.append_blocks(page_id, blocks).await?;
            }
            PublishMode::Append => {
                self.append_blocks(page_id, blocks).await?;
            }
        }

        Ok(format!("https://notion.so/{}", page_id.replace("-", "")))
    }

    async fn clear_page_blocks(&self, page_id: &str) -> Result<()> {
        // Collect all block IDs first — the children endpoint paginates at 100
        let url = format!("{}/blocks/{}/children", NOTION_API_BASE, page_id);
        let mut block_ids: Vec<String> = Vec::new();
        let mut start_cursor: Option<String> = None;

        loop {
            let mut request = self.client
                .get(&url)
                .header("Authorization", format!("Bearer {}", self.token))
                .header("Notion-Version", NOTION_API_VERSION)
                .query(&[("page_size", "100")]);

            if let Some(ref cursor) = start_cursor {
                request = request.query(&[("start_cursor", cursor.as_str())]);
            }

            let response = request.send().await
                .context("Failed to get page blocks")?;

            if !response.status().is_success() {
                let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                return Err(anyhow!("Failed to get page blocks: {}", error_text));
            }

            let data: Value = response.json().await
                .context("Failed to parse response")?;

            if let Some(blocks) = data["results"].as_array() {
                block_ids.extend(
                    blocks.iter().filter_map(|b| b["id"].as_str().map(String::from))
                );
            }

            start_cursor = if data["has_more"].as_bool().unwrap_or(false) {
                data["next_cursor"].as_str().map(String::from)
            } else {
                None
            };

            if start_cursor.is_none() {
                break;
            }
        }

        for block_id in block_ids {
            let delete_url = format!("{}/blocks/{}", NOTION_API_BASE, block_id);
            let response = self.client
                .delete(&delete_url)
                .header("Authorization", format!("Bearer {}", self.token))
                .header("Notion-Version", NOTION_API_VERSION)
                .send()
                .await
                .with_context(|| format!("Failed to delete block {}", block_id))?;

            if !response.status().is_success() {
                let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                return Err(anyhow!("Failed to delete block {}: {}", block_id, error_text));
            }
        }

        Ok(())
    }

    async fn append_blocks(&self, page_id: &str, blocks: Vec<Value>) -> Result<()> {
        if blocks.is_empty() {
            return Ok(());
        }

        let url = format!("{}/blocks/{}/children", NOTION_API_BASE, page_id);

        // Notion API has a limit on batch size, so we send in chunks
        for chunk in blocks.chunks(100) {
            let body = json!({
                "children": chunk
            });

            let response = self.client
                .patch(&url)
                .header("Authorization", format!("Bearer {}", self.token))
                .header("Notion-Version", NOTION_API_VERSION)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
                .context("Failed to append blocks")?;

            if !response.status().is_success() {
                let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                return Err(anyhow!("Failed to append blocks: {}", error_text));
            }
        }

        Ok(())
    }

    pub async fn create_page(&self, parent_page_id: &str, title: &str, blocks: Vec<Value>) -> Result<String> {
        let url = format!("{}/pages", NOTION_API_BASE);

        let body = json!({
            "parent": {
                "type": "page_id",
                "page_id": parent_page_id
            },
            "properties": {
                "title": {
                    "title": [{
                        "text": {
                            "content": title
                        }
                    }]
                }
            },
            "children": blocks
        });

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Notion-Version", NOTION_API_VERSION)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Failed to create page")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow!("Failed to create page: {}", error_text));
        }

        let data: Value = response.json().await
            .context("Failed to parse response")?;

        let page_id = data["id"].as_str()
            .ok_or_else(|| anyhow!("No page ID in response"))?;

        Ok(format!("https://notion.so/{}", page_id.replace("-", "")))
    }
}
