mod cli;
mod config;
mod gui;

use anyhow::{Context, Result};
use clap::Parser;
use cli::{Cli, Commands};
use config::Config;
use polaris_notion::{NotionClient, PublishMode};
use std::fs;
use std::path::PathBuf;

// `main` stays synchronous: iced (with its tokio feature) drives its own
// runtime and panics if started inside another one. Only the async commands
// (deploy, the TUI's deploy path) get a runtime.
fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Gui { file }) => run_gui(file),

        Some(Commands::New { filename }) => {
            let path = PathBuf::from(&filename);
            if path.exists() {
                anyhow::bail!(
                    "File already exists: {}. Open it with 'polaris {}'",
                    filename,
                    filename
                );
            }
            run_gui(Some(path))
        }

        None => run_gui(cli.file),

        Some(command) => {
            let runtime = tokio::runtime::Runtime::new().context("Failed to start runtime")?;
            runtime.block_on(run_command(command))
        }
    }
}

async fn run_command(command: Commands) -> Result<()> {
    match command {
        Commands::Deploy { file, page, mode } => deploy_file(file, page, &mode).await,

        Commands::Config {
            token,
            default_page,
        } => configure(token, default_page),

        Commands::Gui { .. } | Commands::New { .. } => unreachable!("handled in main"),
    }
}

fn run_gui(file: Option<PathBuf>) -> Result<()> {
    if let Some(ref f) = file {
        if f.exists() {
            // Validate readability up front; the GUI boot can't return errors.
            fs::read_to_string(f).with_context(|| format!("Cannot read file: {:?}", f))?;
        }
    }
    gui::run(file).map_err(|e| anyhow::anyhow!("GUI failed: {e}"))
}

async fn deploy_file(file: PathBuf, page_id: Option<String>, mode_str: &str) -> Result<()> {
    let config = Config::load()?;

    let page_id = page_id
        .or_else(|| config.notion.default_page.clone())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No Notion page ID specified. Use --page or configure default in ~/.polaris.toml"
            )
        })?;

    let markdown =
        fs::read_to_string(&file).with_context(|| format!("Failed to read file: {:?}", file))?;

    let mode = match mode_str {
        "replace" => PublishMode::Replace,
        "append" => PublishMode::Append,
        _ => PublishMode::Append,
    };

    let token = config.notion.token.clone().ok_or_else(|| {
        anyhow::anyhow!("Notion token not configured. Please set it in ~/.polaris.toml")
    })?;
    let client = NotionClient::new(token);

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
