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

### Quick install (macOS / Linux)

```bash
curl -fsSL https://raw.githubusercontent.com/saadatqadri/polaris/main/install.sh | sh
```

Fetches the latest prebuilt binary from GitHub Releases into
`~/.local/bin` (override with `POLARIS_INSTALL_DIR`). Binaries are not
yet code-signed; installing via the script avoids macOS quarantine.

### Build from source

Requires Rust (stable):

```bash
git clone https://github.com/saadatqadri/polaris.git
cd polaris
cargo install --path crates/polaris
```

## Quick Start

### 1. Create a New File

```bash
polaris new draft.md   # or just: polaris draft.md
```

This creates a new markdown file and opens it in the editor — one quiet,
centered page with fixed typography.

### 2. Writing in Polaris

- **Autosave is silent** — the file is written ~1s after your last
  keystroke; `● saved` appears in the chrome when at rest
- **Smart punctuation as you type** — `--` becomes —, quotes curl, `...`
  becomes …; never inside code, and one Backspace right after a
  substitution restores what you typed
- The chrome (filename, word count, reading time) fades while you type
  and returns when you rest
- **Closing the window is always safe** — pending changes are flushed on
  the way out; an untitled buffer with content gets one prompt to be named

### 3. Keyboard Shortcuts

`Cmd` on macOS, `Ctrl` elsewhere:

| Shortcut | Action |
|----------|--------|
| `Cmd+S` | Save now (autosave runs regardless); save-as for untitled |
| `Cmd+F` | Find (Enter/Shift+Enter cycle matches, Esc dismisses) |
| `Cmd+R` | Rename the file (prefilled; renames in place, never overwrites) |
| `Cmd+P` | Toggle write / preview (Literata reading mode) |
| `Cmd+T` | Toggle light / dark theme (remembered across launches) |
| `Cmd+D` | Deploy to Notion (Enter confirms, Esc cancels) |
| `Cmd+Z` / `Cmd+Shift+Z` | Undo / redo |
| `Cmd+Y` | Typewriter scrolling — your line holds still |
| `Cmd+G` | Focus mode — dim everything but the current paragraph |
| `Cmd+E` | Hemingway mode — forward only, no deleting |
| `Cmd+K` | Zen mode — chrome hidden until summoned |
| `Cmd+L` | Session word goal (a number sets it, empty clears it) |

### 4. Configure Notion Integration

Before deploying to Notion, set up your credentials:

```bash
polaris config --token "your_notion_integration_token" \
               --default-page "your_page_id"
```

Your configuration is stored in `~/.polaris.toml`.

### 5. Deploy to Notion

From within the editor: press `Cmd+D`, review the confirmation in the
chrome (page + mode; in-editor deploys always append), and press Enter.

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
# set by the in-app Cmd+T toggle; delete the line to follow the OS again
theme = "light"

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
- ✅ Bulleted and numbered lists
- ✅ Bold / italic (rich-text annotations)
- ✅ Code blocks with language tags
- ✅ Inline code
- ✅ Blockquotes
- ✅ Horizontal rules
- ⏳ Images (coming soon)
- ⏳ Links (coming soon)

## Project Structure

```
polaris/
├── crates/
│   ├── polaris-core/     # UI-agnostic document core: rope buffer,
│   │                     # grapheme cursors, undo/redo, typography
│   ├── polaris-notion/   # markdown → Notion blocks + API client
│   └── polaris/          # the binary: iced GUI + clap CLI
│       ├── assets/fonts/ # embedded faces (Newsreader, iA Writer
│       │                 # Mono, Literata — all SIL OFL)
│       └── src/gui/      # window, editor, preview, theme, chrome
├── design/               # DESIGN.md + interactive mock
└── docs/PLAN.md          # the full project plan
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
- [ ] **Phase 1** — GUI (iced) with fixed typography (Newsreader / iA Writer Mono /
  Literata), rope buffer + undo, soft wrap, silent autosave, find, fading
  chrome, smart punctuation
- [x] **Phase 2** — custom editor widget, focus mode, Hemingway mode, zen
  mode, typewriter scrolling, session goals
- [ ] **Phase 3** — writer-friendly version control: named draft snapshots and
  word-level diffs, backed by invisible git
- [ ] **Phase 4** — accept/reject editing workflow; more publish targets
  (HTML/PDF, gist, webhook)
- [ ] **Phase 5** — ship it: signed/notarized `.app`, Homebrew tap, quiet
  updates (prebuilt releases + installer already live)
- [ ] **Phase 6** — iOS: iPad first, then iPhone — native front-end over
  `polaris-core` via FFI, documents in the Files app

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
- [iced](https://github.com/iced-rs/iced) - GUI framework
- [ropey](https://github.com/cessen/ropey) - Rope text buffer
- [pulldown-cmark](https://github.com/raphlinus/pulldown-cmark) - Markdown parser
- [reqwest](https://github.com/seanmonstar/reqwest) - HTTP client
- [clap](https://github.com/clap-rs/clap) - CLI argument parser

Typefaces: [Newsreader](https://github.com/productiontype/Newsreader),
[iA Writer Mono](https://github.com/iaolo/iA-Fonts),
[Literata](https://github.com/googlefonts/literata) — all SIL OFL, bundled.

---

**Polaris** - Your true north for focused writing ⭐
