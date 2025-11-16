use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "polaris")]
#[command(about = "A local-first markdown editor with Notion deployment", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// File to open
    pub file: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create a new markdown file
    New {
        /// Name of the file to create
        filename: String,
    },

    /// Deploy a markdown file to Notion
    Deploy {
        /// File to deploy
        file: PathBuf,

        /// Notion page ID (overrides config default)
        #[arg(short, long)]
        page: Option<String>,

        /// Publishing mode: append or replace
        #[arg(short, long, default_value = "append")]
        mode: String,
    },

    /// Configure Notion integration
    Config {
        /// Set Notion API token
        #[arg(long)]
        token: Option<String>,

        /// Set default Notion page ID
        #[arg(long)]
        default_page: Option<String>,
    },
}
