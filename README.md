# Polaris ⭐

> A local-first markdown editor with Notion deployment

Polaris is a fast, keyboard-driven terminal text editor that lets you write markdown locally and deploy it to Notion with a single command. Inspired by the minimalism of classic editors and built for modern workflows.

## Features

- 🚀 **Local-First**: 100% offline capability, your files live on your machine
- ⌨️ **Keyboard-Driven**: Fast navigation and editing with keyboard shortcuts
- 📝 **Markdown Native**: Write in pure markdown, deploy anywhere
- 🔄 **Notion Integration**: One-command deployment to Notion pages
- 👁️ **Live Preview**: Built-in markdown preview mode
- 🎯 **Distraction-Free**: Clean, minimal terminal interface
- 💾 **Auto-Save**: Optional autosave keeps your work safe

## Installation

### Prerequisites

- Rust 1.70 or later
- Cargo (comes with Rust)

### Build from Source

```bash
git clone https://github.com/yourusername/polaris.git
cd polaris
cargo build --release
sudo cp target/release/polaris /usr/local/bin/
```

## Quick Start

### 1. Create a New File

```bash
polaris new draft.md   # or just: polaris draft.md
```

This creates a new markdown file and opens it in the GUI editor — a quiet,
centered page with fixed typography, silent autosave, Cmd/Ctrl+F find, and
Cmd/Ctrl+Z undo. The legacy terminal editor remains available as
`polaris tui <file>` until the GUI reaches full parity (M5).

### 2. Writing in Polaris

Once in the editor, you can:

- **Type** to insert text
- **Arrow keys** to navigate
- **Enter** to create new lines
- **Backspace/Delete** to remove text
- **Home/End** to jump to line start/end
- **Tab** to insert 4 spaces

### 3. Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+S` | Save file |
| `Ctrl+Q` | Quit editor |
| `Ctrl+D` | Deploy to Notion |
| `Ctrl+P` | Toggle preview mode |

### 4. Configure Notion Integration

Before deploying to Notion, set up your credentials:

```bash
polaris config --token "your_notion_integration_token" \
               --default-page "your_page_id"
```

Your configuration is stored in `~/.polaris.toml`.

### 5. Deploy to Notion

From within the editor:
- Press `Ctrl+D` to deploy to your default Notion page

Or from the command line:
```bash
polaris deploy draft.md
```

With options:
```bash
# Deploy to a specific page
polaris deploy draft.md --page PAGE_ID

# Replace existing content (default is append)
polaris deploy draft.md --mode replace

# Append to existing content
polaris deploy draft.md --mode append
```

## Usage Examples

### Example 1: Daily Notes

```bash
# Create today's note
polaris new "$(date +%Y-%m-%d).md"

# Write your notes
# Press Ctrl+S to save
# Press Ctrl+D to deploy to Notion
```

### Example 2: Blog Post Workflow

```bash
# Create a new blog post
polaris new blog-post.md

# Write your content in markdown
# Preview with Ctrl+P
# Deploy to Notion when ready with Ctrl+D
```

### Example 3: Documentation

```bash
# Edit existing documentation
polaris docs/api.md

# Make changes
# Save with Ctrl+S
# Deploy updates to Notion with Ctrl+D
```

## Configuration

Polaris uses a TOML configuration file at `~/.polaris.toml`:

```toml
[notion]
token = "secret_..."
default_page = "your-page-id"
```

### Getting Your Notion Token

1. Go to https://www.notion.so/my-integrations
2. Click "+ New integration"
3. Give it a name (e.g., "Polaris")
4. Copy the "Internal Integration Token"
5. Share the target page with your integration

### Getting Your Page ID

From a Notion page URL like:
```
https://www.notion.so/My-Page-abc123def456?v=...
```

The page ID is: `abc123def456`

## Supported Markdown

Polaris converts the following markdown elements to Notion blocks:

- ✅ Headers (H1, H2, H3)
- ✅ Paragraphs
- ✅ Bulleted lists
- ✅ Code blocks with syntax highlighting
- ✅ Inline code
- ✅ Blockquotes
- ✅ Horizontal rules
- ⏳ Bold/Italic (coming soon)
- ⏳ Images (coming soon)
- ⏳ Links (coming soon)

## Project Structure

```
polaris/
├── src/
│   ├── cli/          # Command-line interface
│   ├── config/       # Configuration management
│   ├── editor/       # Text editor (buffer, UI)
│   ├── notion/       # Notion API integration
│   └── main.rs       # Application entry point
├── Cargo.toml
└── README.md
```

## Development

### Running in Development

```bash
cargo run -- new test.md
```

### Running Tests

```bash
cargo test
```

### Building for Release

```bash
cargo build --release
```

## Roadmap

The full project plan lives in [`docs/PLAN.md`](docs/PLAN.md); the design
system in [`design/DESIGN.md`](design/DESIGN.md) with a typeable mock at
[`design/mockup.html`](design/mockup.html).

- [x] **MVP** — terminal editor, markdown preview, file operations, Notion deploy, CLI
- [ ] **Phase 1** — GUI (iced) with fixed typography (iA Writer Quattro / Mono,
  Literata), rope buffer + undo, soft wrap, silent autosave, find, fading
  chrome, smart punctuation
- [ ] **Phase 2** — focus mode, Hemingway mode, zen mode, typewriter scrolling,
  session goals
- [ ] **Phase 3** — writer-friendly version control: named draft snapshots and
  word-level diffs, backed by invisible git
- [ ] **Phase 4** — accept/reject editing workflow; more publish targets
  (HTML/PDF, gist, webhook)

**A note on AI:** Polaris will never generate, autocomplete, or ghost-write
text. Every word in a Polaris document is typed by a person.

## Philosophy

Polaris is built on three principles:

1. **Local-First**: Your data lives on your machine. Cloud is optional.
2. **Keyboard-Driven**: Mouse-free workflows for maximum efficiency.
3. **Markdown-Native**: Plain text is forever. Markdown is universal.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

MIT License - see LICENSE file for details

## Credits

Built with:
- [ratatui](https://github.com/ratatui-org/ratatui) - Terminal UI framework
- [crossterm](https://github.com/crossterm-rs/crossterm) - Terminal manipulation
- [pulldown-cmark](https://github.com/raphlinus/pulldown-cmark) - Markdown parser
- [reqwest](https://github.com/seanmonstar/reqwest) - HTTP client
- [clap](https://github.com/clap-rs/clap) - CLI argument parser

---

**Polaris** - Your true north for focused writing ⭐
