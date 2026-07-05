# CLAUDE.md — Polaris

Polaris is a **local-first markdown editor for distraction-free, human writing**,
with one-command deployment to Notion. Rust throughout.

## Product principles (non-negotiable)

1. **Typography is the product.** Fonts are a fixed, bundled set — iA Writer
   Quattro (writing), iA Writer Mono (chrome/code), Literata (preview). There is
   deliberately **no font or appearance configuration** and there never will be.
2. **Every word is human.** AI must never compose, autocomplete, or ghost-write
   into the buffer. Future AI (not now) may only annotate in a margin when
   explicitly summoned. Do not add AI features unprompted.
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
  - **M2 (next):** iced window, embedded fonts (`include_bytes!`), 62ch centered
    column, soft word wrap, basic editing.
  - **M3:** silent debounced autosave, find (Ctrl+F), word-jump, in-window
    save-as prompt.
  - **M4:** fading chrome (0.6s fade, 1.2s return), live word count, smart
    punctuation on input (`"`→“”, `--`→—, `...`→…), light/dark themes
    following the OS.
  - **M5:** rewire Notion deploy + CLI to the new front-end; keep
    `polaris deploy` headless.
- **Phase 2:** focus mode, Hemingway mode (backspace disabled), zen mode,
  typewriter scrolling, session word goals.
- **Phase 3:** writer-friendly version control ("mark draft" snapshots, named
  versions, word-level diffs) backed by invisible git.
- **Phase 4:** accept/reject editing workflow; more publish targets (HTML/PDF,
  gist, webhook).

## Known bugs / debt in existing code

- No word wrap, no undo — the TUI is not prose-usable; that's why Phase 1 exists.
- `src/notion/client.rs` — `create_page` is dead code (wire up or remove in M5).
- Bold/italic markdown maps to plain text in Notion blocks (annotations TODO, M5).
- Fixed 2026-07-05 (see PLAN §2): byte-index Unicode panics, `polaris new`
  clobbering, quit-confirm, deploy-of-stale-copy, `clear_page_blocks`
  pagination, ordered lists, paragraph-after-heading merging.

## Open questions for the user

- Accent color: current is north-star blue (`#4E6E8E` / dark `#8FAECB`);
  muted starlight gold was the alternative. Awaiting reaction to the mock.
- Body size 17.5px Quattro — may want a touch larger after trying the mock.

## Working conventions

- Direct commits to `main` are OK for now (owner's explicit choice); better
  practices (PRs, CI, branch protection) planned later. The old branch
  `claude/polaris-markdown-editor-01TR1jZmXhHzE8XujUPiBbSR` is merged-equal to
  main and can be deleted.
- Build: `cargo build` · Run: `cargo run -- <file.md>` · Test:
  `cargo test --workspace` · Lint like CI: `cargo fmt --all --check &&
  cargo clippy --workspace --all-targets -- -D warnings`.
