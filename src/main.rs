mod cli;
mod config;
mod editor;
mod notion;

use anyhow::{Context, Result};
use clap::Parser;
use cli::{Cli, Commands};
use config::Config;
use editor::{Editor, TextBuffer};
use notion::{NotionClient, PublishMode};
use std::fs;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::New { filename }) => {
            let path = PathBuf::from(&filename);
            if path.exists() {
                anyhow::bail!(
                    "File already exists: {}. Open it with 'polaris {}'",
                    filename,
                    filename
                );
            }
            fs::write(&path, "").context("Failed to create file")?;
            println!("Created: {}", filename);

            let buffer = TextBuffer::from_file(path)?;
            run_editor(buffer).await?;
        }

        Some(Commands::Deploy { file, page, mode }) => {
            deploy_file(file, page, &mode).await?;
        }

        Some(Commands::Config { token, default_page }) => {
            configure(token, default_page)?;
        }

        None => {
            if let Some(file) = cli.file {
                // Open existing file or create new
                let buffer = if file.exists() {
                    TextBuffer::from_file(file)?
                } else {
                    println!("File does not exist. Creating new file: {:?}", file);
                    fs::write(&file, "").context("Failed to create file")?;
                    TextBuffer::from_file(file)?
                };

                run_editor(buffer).await?;
            } else {
                // No file specified, create a new untitled buffer
                run_editor(TextBuffer::new()).await?;
            }
        }
    }

    Ok(())
}

async fn run_editor(buffer: TextBuffer) -> Result<()> {
    let mut editor = Editor::new(buffer);
    editor.run()?;

    // If user pressed Ctrl+D, deploy (the editor saves the buffer before setting this)
    if editor.should_deploy {
        if let Some(ref path) = editor.buffer.file_path {
            deploy_file(path.clone(), None, "append").await?;
        }
    } else if editor.buffer.dirty && editor.buffer.file_path.is_none() {
        println!("\nFile not saved. Use Ctrl+S in the editor or save with a filename.");
    }

    Ok(())
}

async fn deploy_file(file: PathBuf, page_id: Option<String>, mode_str: &str) -> Result<()> {
    let config = Config::load()?;

    let page_id = page_id.or_else(|| config.notion.default_page.clone())
        .ok_or_else(|| anyhow::anyhow!("No Notion page ID specified. Use --page or configure default in ~/.polaris.toml"))?;

    let markdown = fs::read_to_string(&file)
        .with_context(|| format!("Failed to read file: {:?}", file))?;

    let mode = match mode_str {
        "replace" => PublishMode::Replace,
        "append" => PublishMode::Append,
        _ => PublishMode::Append,
    };

    let client = NotionClient::new(&config)?;

    println!("Deploying to Notion...");
    let url = client.deploy(&markdown, &page_id, mode).await?;

    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    println!("\n✓ Deployment successful!");
    println!("  Time: {}", timestamp);
    println!("  Mode: {}", mode_str);
    println!("  URL: {}", url);

    Ok(())
}

fn configure(token: Option<String>, default_page: Option<String>) -> Result<()> {
    let mut config = Config::load().unwrap_or_default();

    if let Some(t) = token {
        config.notion.token = Some(t);
        println!("Notion token updated");
    }

    if let Some(p) = default_page {
        config.notion.default_page = Some(p);
        println!("Default Notion page updated");
    }

    config.save()?;
    println!("Configuration saved to ~/.polaris.toml");

    Ok(())
}
