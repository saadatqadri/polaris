# CLAUDE.md — Polaris

Polaris is a **local-first markdown editor for distraction-free, human writing**,
with one-command deployment to Notion. Rust throughout.

## Product principles (non-negotiable)

1. **Typography is the product.** Fonts are a fixed, bundled set — Newsreader
   16pt (writing AND preview — one voice since 2026-07-08), iA Writer Mono (chrome/code). There is
   deliberately **no font or appearance configuration** and there never will be.
2. **Every word is human.** AI must never compose, autocomplete, or ghost-write
   into the buffer. Future AI (not now) may only annotate in a margin when
   explicitly summoned, on a marked draft — see **`docs/AI.md`** (binding,
   2026-07-07): no machine words in the buffer as an architectural
   invariant; critique/cuts only, never replacement prose. Do not add AI
   features unprompted.
3. **Chrome recedes.** Minimal UI that fades while typing. No panels, toolbars,
   badges, or notifications in the writing surface.
4. **Local-first.** Plain `.md` files on disk are the source of truth. Cloud
   (Notion) is an optional publish target, nothing more.

The design source of truth is **`design/DESIGN.md`** (tokens, type rules,
interaction specs, keyboard map) and **`design/mockup.html`** (interactive,
typeable mock — open in a browser). Match the mock exactly when building UI.
The **full project plan** — vision, GUI refactor architecture, milestone
acceptance criteria, engineering conventions, risks — is **`docs/PLAN.md`**;
this file is only the condensed handover summary.

## Current state (2026-07-05, M1 complete)

- **Done (MVP):** terminal (ratatui) editor with basic editing, save/save-as,
  markdown preview, `~/.polaris.toml` config, Notion deploy (markdown → Notion
  blocks via pulldown-cmark), clap CLI (`new` / `deploy` / `config`).
- **Decided:** pivot from TUI to a **GUI using `iced`** so typography can be
  owned by the product (terminal can't control fonts). Keyboard-driven and
  local-first are unchanged. Design phase complete and approved-in-principle.
- **M1 done:** Cargo workspace (`crates/polaris-core`, `crates/polaris-notion`,
  `crates/polaris`). `polaris-core` has the ropey buffer, grapheme-aware
  cursor/selection/word-jump (unicode-segmentation), grouped undo/redo,
  file binding + autosave debounce policy, word count, and the
  smart-punctuation transforms (with markdown-`---` escape). 74 tests across
  the workspace; GitHub Actions CI (fmt, clippy -D warnings, test on
  Linux+macOS); `Cargo.lock` committed. The TUI still uses its own (fixed)
  buffer — it is frozen and dies at M5, so it was not rewired to core.

## Roadmap

- **Phase 1 (current):** editor fundamentals + iced GUI shell. Milestones:
  - **M1 — DONE (2026-07-05):** workspace split; `polaris-core` with rope
    buffer, grapheme cursors, grouped undo/redo, autosave policy, word count,
    typography transforms; CI.
  - **M2 — landed 2026-07-05, pending owner's hands-on check:** iced 0.14
    window (`polaris gui <file>`), embedded Quattro/Mono via `include_bytes!`,
    62ch centered column, soft wrap, both themes (OS-detected at startup),
    save + undo/redo through core. Spike outcome (PLAN §7 #3): iced
    `text_editor` is the interaction layer; every edit syncs into
    `polaris-core::Document` as a char-diff (`Document::replace_range`), so
    core owns undo grouping. Custom cosmic-text widget deferred to Phase 2.
    Typeface history: Quattro → Instrument Sans (2026-07-05 audition) →
    **Newsreader 16pt** (2026-07-06, owner call after daylight use; body
    19px/1.56). Mono stays for chrome. Literata retired 2026-07-08: preview unified on Newsreader (owner call — the face change read as noise once both were serifs; the rendering carries the mode switch).
  - **M3 — landed 2026-07-05, pending owner's hands-on check:** silent
    debounced autosave (1s, Cmd+S forces + opens save-as when untitled),
    `● saved` chrome, Cmd+F find (chrome bar, Enter/Shift+Enter cycle, Esc
    dismiss), in-window save-as, and the GUI is now the default:
    `polaris [file]` and `polaris new` open the GUI; the frozen TUI moved to
    `polaris tui`. The update loop (edit→debounce→autosave, save-as, find)
    is covered by headless unit tests in `gui/mod.rs`; what tests can't
    cover is physical typing into the window. NOTE: `main` must stay
    synchronous — iced (tokio feature) panics inside `#[tokio::main]`;
    async commands get their own runtime in `run_command`.
  - **M4 — landed 2026-07-05, pending owner's hands-on check:** chrome fade
    (0.6s out on keystroke, back after 1.2s rest; always visible in
    overlays/preview), word count + reading time, smart punctuation applied
    at input (core transforms; skipped inside code fences/spans via a
    backtick-parity heuristic; one backspace right after a substitution
    restores the literal keystrokes), Cmd/Ctrl+P preview (unified on the writing face 2026-07-08)
    (pulldown-cmark → iced rich text: headings, lists, quotes, code blocks,
    rules; caret-relative scroll on entry; Esc/Cmd+P exits). Known gap vs
    the mock: markdown source marks are NOT yet styled quiet in write mode —
    needs the Phase 2 custom widget/highlighter.
  - **M5 — landed 2026-07-05; PHASE 1 COMPLETE (pending owner's hands-on
    check):** Cmd/Ctrl+D deploys from the GUI — in-chrome confirmation
    (page + mode; in-editor deploys always append, replace stays CLI-only
    because it is destructive), saves first, async via Task::perform,
    result line with time + URL. Notion debt cleared: bold/italic →
    rich-text annotations; `create_page` deleted. TUI deleted (`polaris
    tui` gone; core's tests own the editing domain). `polaris deploy`
    remains headless.
- **Phase 2 — editor widget landed 2026-07-08 (pending hands-on):**
  `gui/editor.rs` replaced the `text_editor` shim entirely — Document is
  the single source of truth (no `apply_diff`). Parity: IME preedit/commit,
  selection with drag + double-click word select, clipboard, auto-scroll.
  New: quiet markdown marks in write mode, steady caret (DESIGN: nothing
  blinks), Cmd+Y typewriter scrolling, Cmd+G focus dim (session flags).
  **Phase 2 complete 2026-07-08 (pending owner's real-writing round):**
  Hemingway (Cmd+E, forward-only; undo deliberately still allowed), zen
  (Cmd+K; status messages and overlays still summon the chrome), session
  goals (Cmd+L, in-chrome input, counts words since set, quiet ✓ when
  met). All session flags — no persistence, no settings, per DESIGN.
  Typewriter hold (45%) awaits feel-tuning from real use.
- **Phase 3 — landed 2026-07-08 (pending hands-on):** drafts per
  docs/DRAFTS.md — polaris-drafts crate (content-addressed zstd snapshots,
  linear manifest, no git; .polaris/ sidecar self-ignores). Cmd+M marks
  (prefilled overlay, kept forever), autos ride the autosave path (10-min
  cadence + file-open baseline, pruned 50/7d), Cmd+Shift+M browser
  (newest-first, word deltas), Enter views with word-level diff (draft-only
  words struck in quiet; Tab flips direction), R restores (snapshots
  current first; one undo group). Rename migrates history. notes/ dir
  reserved for Phase 4 AI annotations.
- **Phase 4:** accept/reject editing workflow; **Write Once, Publish
  Anywhere** (owner direction 2026-07-09, eventual business model): Hugo
  first (owner's saadatqadri.com — front matter + write into content/,
  no git automation v1), then HTML/PDF, LinkedIn (API-restricted:
  format+copy v1), Substack (no API: investigate). polaris-notion
  generalizes into target adapters. Owner writes technical docs: preview
  fidelity (images, mermaid) and target passthrough matter.
- **Phase 5:** ship it — signing/notarization, .app bundle, Homebrew tap,
  quiet updates. Prebuilt releases + install.sh live since 2026-07-07
  (tag `v*` triggers .github/workflows/release.yml).
- **Phase 6:** iOS, iPad first — **design doc: docs/IOS.md**. Native
  SwiftUI over polaris-core via uniffi FFI (iced has no iOS target);
  DocumentGroup app editing .md in Files/iCloud; native text surface first
  (like the desktop M2 shim), custom view for writing modes later. i0 + i1 DONE 2026-07-13: crates/polaris-ffi (uniffi wrapper over core);
  the SwiftUI app in apple/ (DocumentGroup + native TextEditor + bundled
  fonts + core-driven word count) BUILDS AND RUNS on the iPad simulator
  (Xcode 26 now installed). Build: sh apple/setup.sh → open
  apple/Polaris.xcodeproj. Sim on Apple Silicon needs
  ARCHS=arm64 EXCLUDED_ARCHS=x86_64. Generated project/bindings/fonts are
  gitignored (setup.sh recreates). Physical iPad + TestFlight need the
  owner's Apple Developer account; i2 DONE 2026-07-14: owned UITextView (PolarisTextView) with smart
  punctuation at input via the smart_substitution FFI (— … quotes,
  code-guarded), live word_count FFI, DocumentGroup autosave. The
  code-context guard now lives in polaris-core::typography
  (substitute_in_context), shared desktop+iOS. i3+ = undo/selection through
  core + writing modes (fuller custom text view).
  NOTE: the iced 'polaris' crate does NOT build for iOS — apple/ is a
  separate front-end; keep them independent.
  Gate: owner needs Apple Developer account + Xcode for on-device builds
  (I can write all code but can't build/sign to the iPad from here).

## Known gaps / debt

- Markdown source marks (`#`, `**`) are not yet dimmed in write mode —
  needs the Phase 2 custom editor widget (see M4 note above).
- Preview scroll preservation is caret-ratio approximate, not exact.
- Theme: Cmd+T persists to `~/.polaris.toml` (`theme` key; delete it to
  follow the OS). No live OS-theme following.
- Close protection covers window close requests (close button / Cmd+W).
  macOS Cmd+Q via the app menu may terminate without a close request —
  unverified; if last-second keystrokes ever drop on Cmd+Q, that's why.
- Notion: images and links still map to plain text.
- Preview: mermaid blocks show labeled source, not rendered diagrams —
  real rendering needs a JS engine/webview (against the design) or a
  network service (against local-first). Deliberate; revisit only with
  its own design conversation.

## Open questions for the user

- Accent color: current is north-star blue (`#4E6E8E` / dark `#8FAECB`);
  muted starlight gold was the alternative. Awaiting reaction to the mock.
- Body size: Newsreader at 19px/1.56 — sanity-check after real use.

## Working conventions

- Direct commits to `main` are OK for now (owner's explicit choice); better
  practices (PRs, CI, branch protection) planned later. The old branch
  `claude/polaris-markdown-editor-01TR1jZmXhHzE8XujUPiBbSR` is merged-equal to
  main and can be deleted.
- Build: `cargo build` · Run: `cargo run -- <file.md>` · Test:
  `cargo test --workspace` · Lint like CI: `cargo fmt --all --check &&
  cargo clippy --workspace --all-targets -- -D warnings`.
