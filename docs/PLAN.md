# Polaris — Project Plan

> Polaris is a local-first markdown editor for distraction-free, human writing,
> with one-command deployment to Notion.

This is the full project plan: product definition, the GUI refactor, and the
phased roadmap. The design system it implements lives in
[`design/DESIGN.md`](../design/DESIGN.md) and the typeable mock in
[`design/mockup.html`](../design/mockup.html). Day-to-day agent handover notes
live in [`CLAUDE.md`](../CLAUDE.md).

---

## 1. Vision

Modern writing tools are visually noisy, ergonomically inefficient, and —
increasingly — eager to write *for* you. Polaris is the counterposition:

- **Local-first.** Plain `.md` files on disk are the source of truth. Offline
  always works. Cloud (Notion today, more later) is a publish target, never a home.
- **Distraction-free.** One warm page, fixed beautiful typography, chrome that
  fades while you type. The document is the interface.
- **Human.** Every word in a Polaris document was typed by a person. AI never
  composes, autocompletes, or ghost-writes. When AI eventually arrives
  (Phase 4+, opt-in), it may only *annotate in a margin when explicitly
  summoned* — an editor, never an author.

Inspirations: Draft (draftin.com) for writer-centric version control,
accept/reject editing, and Hemingway mode; iA Writer for typographic discipline.

### Target users

Writers and long-form creators; engineers writing docs; researchers drafting
offline; Notion users who dislike Notion's editor; anyone who wants their
writing to stay theirs.

### Product principles (non-negotiable)

1. **Typography is the product.** Bundled fonts — iA Writer Quattro (writing),
   iA Writer Mono (chrome/code), Literata (preview). No font or appearance
   settings, ever. Two themes (light/dark) following the OS.
2. **Every word is human.** No AI generation into the buffer, under any flag.
3. **Chrome recedes.** No panels, toolbars, badges, or notifications in the
   writing surface.
4. **Keyboard-driven.** The mouse is optional everywhere.
5. **Deterministic publishing.** Same markdown → same Notion structure.

---

## 2. Where we are

**Shipped (TUI MVP, ~1,200 lines of Rust):**

- ratatui/crossterm terminal editor: char editing, arrow navigation,
  save / save-as, status bar, markdown preview mode
- `~/.polaris.toml` config (Notion token + default page)
- Markdown → Notion blocks (pulldown-cmark → Notion API JSON): headings,
  paragraphs, bulleted lists, code blocks, quotes, dividers, inline code
- Notion deploy with append/replace modes (CLI `polaris deploy` and Ctrl+D)
- clap CLI: `new` / `deploy` / `config`

**Known debt:**

| Issue | Where | Disposition |
|---|---|---|
| Byte-indexed editing panics on non-ASCII input | `src/editor/buffer.rs` | **Fixed 2026-07-05** (char-indexed, with tests). M1's rope rewrite still replaces this buffer wholesale |
| No word wrap, no undo | TUI editor | Superseded by GUI (Phase 1) |
| `clear_page_blocks` reads only first page of blocks (no pagination cursor) | `src/notion/client.rs` | **Fixed 2026-07-05** (cursor pagination; delete errors now propagate) |
| Bold/italic map to plain text in Notion | `src/notion/blocks.rs` | **Fixed in M5** (rich-text annotations) |
| `create_page` is dead code | `src/notion/client.rs` | **Deleted in M5** |
| No CI | repo | CI + committed lockfile land with M1 |

Also fixed 2026-07-05, with 30 unit tests (TUI buffer semantics + blocks
converter): `polaris new` clobbering existing files, unusable quit-confirm,
Ctrl+D deploying the stale on-disk copy (now saves first, appends, and works
from every launch path), Esc not leaving preview, ordered lists flattening to
bullets, and paragraphs after headings being merged into the heading block.
The buffer tests double as the acceptance suite for M1's rope rewrite.

**Decided:** pivot the front-end from TUI to a GUI, because a terminal cannot
control fonts and typography is a core product value. Design phase is complete
(`design/`), approved in principle; two open questions in §7.

---

## 3. The GUI refactor

### Why iced

Pure Rust (no web stack, no Electron), single-binary output, keyboard-first is
natural, and its text stack (`cosmic-text`) does real shaping so embedded
Quattro/Literata render properly. Alternatives considered: egui (immediate-mode
feel wrong for a document surface), Tauri (webview = heavier, drifts from
"quiet native tool"), gpui (immature ecosystem).

### Target architecture

Move to a Cargo workspace. The core stays UI-agnostic so front-ends are
replaceable (the TUI could return later as a second face):

```
polaris/
├── Cargo.toml            # workspace
├── crates/
│   ├── polaris-core/     # THE library: no UI, no I/O assumptions
│   │   ├── buffer.rs     # rope text buffer (ropey)
│   │   ├── cursor.rs     # grapheme-aware cursor & selection (unicode-segmentation)
│   │   ├── history.rs    # undo/redo (grouped edit operations)
│   │   ├── document.rs   # file binding, autosave policy, word count
│   │   └── typography.rs # smart punctuation transforms ("→“” , --→—, ...→…)
│   ├── polaris-notion/   # markdown→blocks + API client (moved from src/notion)
│   └── polaris/          # the binary: iced app + clap CLI
│       ├── app.rs        # iced Model/Message/update/view
│       ├── editor/       # editor view, markdown-quiet highlighter
│       ├── preview.rs    # Literata reading mode (pulldown-cmark → iced spans)
│       ├── chrome.rs     # fading status line, word count
│       ├── theme.rs      # the two fixed themes (tokens from DESIGN.md)
│       └── fonts.rs      # include_bytes! embedded faces
```

**Buffer model:** `ropey` rope with all edits expressed as operations
(insert/delete + range), which gives:
- O(log n) edits on large documents
- char/byte/line indexing that makes grapheme-correct cursors tractable
- an operation log for undo/redo (grouped by word/pause, so Ctrl+Z undoes
  human-sized chunks, not single keystrokes)

**Editor widget decision (flagged, resolve in M2):** iced ships a
`text_editor` widget. Preferred path is **own the buffer in `polaris-core`
and render via a custom widget over cosmic-text**, because Phase 2 features
(typewriter scrolling, focus-mode paragraph dimming) and our markdown-quiet
styling need layout control that `text_editor` may not expose. Fallback if the
custom widget is too costly for M2: start on `text_editor` + its `Highlighter`
trait for markdown styling, keep `polaris-core` as the document model, and
swap the widget in Phase 2. Time-box the spike to a couple of days.

**Fonts:** WOFF2/TTF files vendored under `assets/fonts/` (all SIL OFL —
license texts vendored alongside), loaded once at startup via
`include_bytes!`. No runtime font discovery, no fallback to system fonts for
body text.

**TUI disposition:** frozen now (no new TUI features), deleted at M5 when the
GUI reaches parity. `polaris deploy` stays fully headless throughout.

---

## 4. Phase 1 — Editor fundamentals + GUI shell

Each milestone lands as working, committed software.

### M1 — `polaris-core` (start here)
Workspace split; rope buffer; grapheme-aware cursor movement (arrows, word-jump,
Home/End, line up/down with sticky column); insert/delete/newline; undo/redo
with edit grouping; word count; smart-punctuation transform functions.
**Tests:** unit tests for every operation, including é/emoji/CJK/curly-quote
input, undo grouping, and word counts. This crate is where the test culture starts.
**Done when:** `cargo test -p polaris-core` passes and the old byte-panic is
impossible by construction.

### M2 — The window
iced app opens a file in the fixed writing surface: embedded fonts, 62ch
centered measure, Quattro 17.5px/1.62, soft word wrap, visible-but-quiet
markdown marks, cursor + selection in the accent color, both themes following
the OS. Editing wired to `polaris-core` (typing, enter, backspace/delete,
arrows, word-jump, selection, undo/redo).
**Done when:** you can comfortably type a multi-paragraph document with
curly quotes and em-dashes, resize the window, and nothing panics or shifts.

### M3 — Writing essentials
Silent debounced autosave (~1s after last keystroke; Ctrl+S forces immediate);
`● saved` indicator; Ctrl+F find (bar appears in chrome, Enter/Shift+Enter
cycles matches, Esc dismisses); in-window save-as prompt for untitled buffers;
open-file behavior for `polaris <file>` and `polaris new <file>`.
**Done when:** a full write-save-reopen loop needs no terminal and no dialogs.

### M4 — Quiet chrome & typography polish
Chrome fade (0.6s out on keystroke, back 1.2s after rest); live word count +
reading time; smart punctuation applied on input via core transforms;
Ctrl+P preview mode in Literata (same column, rendered markdown, scroll
position preserved between modes).
**Done when:** the app matches `design/mockup.html` side by side.

### M5 — Reconnect the pipeline, retire the TUI
Ctrl+D deploys via existing Notion module with a minimal in-chrome
confirmation (page + mode) and result line (URL, timestamp). Fix Notion debt:
pagination in `clear_page_blocks`, bold/italic annotations, wire or delete
`create_page`. Delete `src/editor` TUI code; `polaris deploy` remains headless.
**Done when:** the GUI is the only editor face and deploy works end-to-end
from both keyboard and CLI.

---

## 5. Later phases

### Phase 2 — The writing modes
- **Focus mode:** dim all paragraphs except the current one
- **Hemingway mode:** backspace/delete disabled — forward only, edit later
- **Zen mode:** chrome fully hidden until summoned
- **Typewriter scrolling:** cursor line held vertically centered
- **Session goals:** optional word-count target with a whisper-quiet progress cue

### Phase 3 — Drafts (writer-friendly version control)
Draft's killer feature, local-first: invisible git under `.polaris/` (or the
user's repo if present). Autosaves commit to a hidden ref; **Ctrl+M "marks a
draft"** with a name ("Draft 3 — after Sarah's notes"); a drafts browser shows
named versions with **word-level diffs** and one-key restore. The writer never
sees git vocabulary. Design doc required before build (data model, merge-free
linear history, repo-inside-repo handling).

### Phase 4 — Editing workflow & publish anywhere
- Import an edited copy → word-level diff → accept/reject each change
  (Draft's collaboration model, without a server)
- Publish targets beyond Notion: HTML/PDF export, GitHub gist, generic webhook
- Only here do we *consider* summoned-AI margin annotations, per principle #2 —
  and only critique/questions, never text generation

---

## 6. Engineering conventions

- **Testing:** unit tests required in `polaris-core` and `polaris-notion`
  (blocks conversion is pure-function testable; API client behind a trait so
  deploy logic tests against a fake). GUI logic kept thin.
- **CI (add during M1):** GitHub Actions running `cargo fmt --check`,
  `cargo clippy -- -D warnings`, `cargo test` on Linux + macOS.
- **Commit `Cargo.lock`** (this repo ships a binary).
- **Workflow:** direct commits to `main` are acceptable for now (owner's
  choice); revisit branch protection + PRs once CI exists.
- **MSRV:** latest stable; no nightly features.

## 7. Open decisions

| # | Question | Options | Status |
|---|---|---|---|
| 1 | Accent color | North-star blue `#4E6E8E`/`#8FAECB` (current) vs. muted starlight gold | **Decided 2026-07-05: north-star blue** |
| 2 | Body size | 17.5px (current) vs. 18–19px | **Decided 2026-07-05: 17.5px**, revisit in the real GUI |
| 3 | Editor widget | Custom cosmic-text widget vs. iced `text_editor` + highlighter | **Decided in M2: start on `text_editor`**, with `polaris-core` as the document model synced via a char-diff shim; custom widget lands with Phase 2 (typewriter scrolling / focus dimming need it) |
| 4 | Window chrome | Native decorations (current plan) vs. frameless | Defer until after M4 |
| 5 | Writing typeface | Quattro vs Lora / Newsreader / Alegreya / Instrument Sans (audition page, 2026-07-05) | **Decided 2026-07-05: Instrument Sans** (true italics vendored). Still bundled-only, no user setting |

## 8. Risks

- **iced API churn / widget gap** — mitigated by the M2 time-boxed spike and
  by keeping all document logic in `polaris-core` (front-end is replaceable).
- **Text rendering quality** (ligatures, fractional metrics, HiDPI) — validate
  in M2 against the mock on both a HiDPI and a 1080p display before M3.
- **Scope gravity toward "just one more setting"** — the principles section
  exists to say no. New settings require editing this plan first.
