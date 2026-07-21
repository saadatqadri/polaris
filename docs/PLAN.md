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
- **Write Once, Publish Anywhere** *(owner direction, 2026-07-09 — and the
  eventual business model)*: Polaris is where everything gets drafted —
  blog posts, technical docs, LinkedIn posts, newsletters — and publishing
  is syndication outward to whatever platform: Notion today; Hugo next
  (owner's saadatqadri.com); LinkedIn, Substack, and more later. The
  document's plain-markdown home never moves; targets are adapters.

**The eventual goal** *(owner, 2026-07-10)*: not just a clean page —
**Polaris should make its writer a better writer and a better thinker.**
Only three things reliably do that: writing more, revising more, honest
critique. The product maps onto them exactly — Phases 1–2 make writing
frictionless and separate generation from editing (Hemingway); Phase 3
makes revision visible (your own draft diffs are self-knowledge); Phase 4's
summoned critique asks questions instead of rewriting, because doing the
fixing is the learning. What Polaris refuses to do is part of the design:
a tool that writes for you atrophies the skill it claims to serve.

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

## 2. Where we are (2026-07-15)

The sections below (§3–§4) are the historical design record of the GUI
refactor — kept because the architecture decisions still hold. Current
status, condensed (see `CLAUDE.md` for the working handover):

**Desktop (Rust/iced) — Phases 1–3 COMPLETE, released `v0.2.3`.** A Cargo
workspace (`polaris-core`, `polaris-notion`, `polaris-drafts`,
`polaris-ffi`, `polaris`): the custom editor widget with Document as the
single source of truth, silent autosave, find, rename, deploy to Notion,
preview, the writing modes (typewriter/focus/Hemingway/zen/goals), and
drafts (mark/browse/word-diff/restore). `install.sh` + tag-triggered
releases; welcome tour; CI green on every push.

**iPad (SwiftUI over the core) — Phase 6 pulled forward, i0–i3 + modes
running on the owner's device.** `apple/` is a native SwiftUI DocumentGroup
app over `polaris-core` via the uniffi FFI (see `docs/IOS.md`): the page,
smart punctuation, preview, and a floating ✧ modes control
(Preview/Typewriter/Hemingway). Focus mode + TestFlight are follow-ons.

**Next: Phase 4** (§5) — publish-anywhere (Hugo + Substack first) and the
accept/reject editing workflow — plus the rest of the MVP.

The TUI MVP that started this project was superseded by the GUI and deleted
at M5; its early bug fixes and the original Notion debt are all resolved.

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
Draft's killer feature, local-first: the document quietly keeps its own
history. **Ctrl+M "marks a draft"** with a name ("Draft 3 — after Sarah's
notes"); a drafts browser shows named versions with **word-level diffs**
and one-key restore. The writer never sees version-control vocabulary.
**Design doc: [`docs/DRAFTS.md`](DRAFTS.md)** (2026-07-08) — it revises
the original "invisible git" sketch to a content-addressed snapshot store
(no git: linear merge-free history, tiny documents, self-ignoring
`.polaris/` sidecar), defines the data model, snapshot policy, browser UX,
and reserves annotation storage per `docs/AI.md`. Awaiting owner approval.

### Phase 4 — Editing workflow & publish anywhere
**Design doc: [`docs/PHASE4.md`](PHASE4.md)** (approved 2026-07-16) —
**complete on desktop (P1–P6, 2026-07-16 → 07-21).** The publish-adapter
layer (`polaris-publish`: Notion, Hugo, HTML, Substack, LinkedIn behind one
`Target` trait; Cmd+D picker; `polaris publish` CLI), the accept/reject
workflow, and Part C's Preview additions (reading pointer, inline notes,
inline images). Remaining Phase 4 work is the iPad port and the AI pass.
- Import an edited copy → word-level diff → accept/reject each change
  (Draft's collaboration model, without a server)
- **Publish targets beyond Notion** (the Write-Once-Publish-Anywhere arc).
  The two the owner has named as priorities are **Hugo** and **Substack**:
  - **Hugo (first — owner's saadatqadri.com).** Cheapest by far, because
    Hugo *is* markdown: generate front matter (title from H1, date, draft
    flag) and write into the configured `content/` directory of the site
    repo; the owner's existing deploy pipeline does the rest. Config: a
    `[hugo]` section (content dir, optional front-matter defaults). No git
    automation in v1 — Polaris writes the file, the user commits.
  - **Substack (named priority — newsletters).** No official publishing
    API, so v1 is honest about the mechanics. Two candidate paths, to
    settle at design time:
    - **Email-to-draft**: many Substacks accept a post by emailing a
      publication-specific address; Polaris renders the markdown to a
      Substack-friendly email (HTML) and sends via the user's SMTP or a
      `mailto:`. Lands as a Substack *draft* to review and send — which
      fits Polaris's "you press publish, not the machine" stance.
    - **Format-and-paste**: convert to the rich HTML Substack's editor
      accepts and copy to clipboard; the user pastes into a new post.
      Zero-dependency fallback, always works.
    Recommendation: ship format-and-paste first (trivial, reliable),
    investigate email-to-draft as the one-step upgrade. No unofficial
    API scraping — fragile and against ToS.
  - **HTML/PDF export** — local, no accounts, table stakes.
  - **LinkedIn** — API access is restrictive (partner program for posting);
    v1 realism is "format for LinkedIn + copy to clipboard" until/unless
    API access lands.
  - Architecture: `polaris-notion` generalizes into a `publish` layer of
    target adapters (markdown in, platform out); Cmd+D grows a target
    picker only when there are ≥2 targets — one keystroke stays one
    keystroke. Clipboard/email targets and file/API targets share the
    adapter trait; the difference is just the "out" side.
- **Technical-docs preview fidelity** (owner writes a lot of these):
  tables ✓ (2026-07-09), code blocks ✓; still open — inline images in
  preview, mermaid rendering (deliberately parked, see CLAUDE.md), and
  how each target handles them (Hugo passes mermaid/images through
  natively — another reason it's first)
- Only here do we *consider* summoned-AI margin annotations, and only under
  the binding rules in [`docs/AI.md`](AI.md) (2026-07-07): no machine words
  in the buffer as an architectural invariant, summonable only on a marked
  draft (never the live buffer), critique/questions/cuts only — never
  replacement prose. Its own design doc is required before build

---

### Phase 5 — Ship it
Getting Polaris into other people's hands (started 2026-07-07 with
tag-triggered GitHub Releases + `install.sh` for the first outside tester):
- macOS code signing + notarization; a proper `.app` bundle (double-click
  launch, Dock icon, Cmd+Q safety)
- Homebrew tap; Linux packaging as demand appears
- A quiet update check (chrome-styled, never a dialog)
- A small website: the mock is already the pitch

### Phase 6 — iOS (iPad first, then iPhone)
The architecture bet pays off or it doesn't: **`polaris-core` is the
portable asset.** iced does not target iOS, so the mobile front-end will
be native (SwiftUI) over the Rust core compiled for iOS behind FFI
(e.g. uniffi) — exactly the "front-ends are replaceable" split from §3.
- iPad first: external-keyboard writing is the core scenario
- Documents stay plain `.md` via the Files app / iCloud Drive — local-first
  survives intact, sync is the OS's job, not ours
- Same fixed typography, no settings, AI rules per `docs/AI.md`
- **Design doc: [`docs/IOS.md`](IOS.md)** (2026-07-13) — iced doesn't run on
  iOS, so the front-end is native SwiftUI over polaris-core via uniffi FFI;
  a DocumentGroup app editing .md in Files/iCloud; native text surface
  first (mirrors the desktop M2 shim), custom text view for writing modes
  later. Milestone i0 = the FFI spike. Gate: owner needs an Apple Developer
  account + Xcode to reach the physical iPad.

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
| 5 | Writing typeface | Quattro vs Lora / Newsreader / Alegreya / Instrument Sans (audition page, 2026-07-05) | **Decided: Newsreader, 16pt optical size** (2026-07-06, revising the Instrument Sans pick after daylight use). Still bundled-only, no user setting |

## 8. Risks

- **iced API churn / widget gap** — mitigated by the M2 time-boxed spike and
  by keeping all document logic in `polaris-core` (front-end is replaceable).
- **Text rendering quality** (ligatures, fractional metrics, HiDPI) — validate
  in M2 against the mock on both a HiDPI and a 1080p display before M3.
- **Scope gravity toward "just one more setting"** — the principles section
  exists to say no. New settings require editing this plan first.
