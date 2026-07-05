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

## Current state (2026-07-05)

- **Done (MVP, ~1,200 lines):** terminal (ratatui) editor with basic editing,
  save/save-as, markdown preview, `~/.polaris.toml` config, Notion deploy
  (markdown → Notion blocks via pulldown-cmark), clap CLI
  (`new` / `deploy` / `config`). Builds clean on stable; a few dead-code warnings.
- **Decided:** pivot from TUI to a **GUI using `iced`** so typography can be
  owned by the product (terminal can't control fonts). Keyboard-driven and
  local-first are unchanged. Design phase complete and approved-in-principle.
- **No tests exist yet.** No CI. `Cargo.lock` is currently gitignored — consider
  committing it (recommended for binaries) as part of the next infra touch.

## Roadmap

- **Phase 1 (next):** editor fundamentals + iced GUI shell. Milestones:
  - **M1 — `polaris-core` crate (START HERE):** extract buffer logic out of the
    binary into a UI-agnostic library crate. Replace `Vec<String>` buffer with
    a rope (`ropey`), grapheme-aware cursors (`unicode-segmentation`),
    undo/redo stack, unit tests.
  - **M2:** iced window, embedded fonts (`include_bytes!`), 62ch centered
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

- `src/editor/buffer.rs` — `insert_char`/`backspace` index by **bytes**, so any
  non-ASCII input (é, curly quotes, em-dash) panics. Fixed properly by M1's
  rope rewrite; don't band-aid it.
- No word wrap, no undo — the TUI is not prose-usable; that's why Phase 1 exists.
- `src/notion/client.rs` — `clear_page_blocks` fetches only the first page of
  blocks (no pagination cursor) and `create_page` is dead code.
- Bold/italic markdown maps to plain text in Notion blocks (annotations TODO).

## Open questions for the user

- Accent color: current is north-star blue (`#4E6E8E` / dark `#8FAECB`);
  muted starlight gold was the alternative. Awaiting reaction to the mock.
- Body size 17.5px Quattro — may want a touch larger after trying the mock.

## Working conventions

- Direct commits to `main` are OK for now (owner's explicit choice); better
  practices (PRs, CI, branch protection) planned later. The old branch
  `claude/polaris-markdown-editor-01TR1jZmXhHzE8XujUPiBbSR` is merged-equal to
  main and can be deleted.
- Build: `cargo build` · Run: `cargo run -- <file.md>` · No test suite yet —
  add one starting with `polaris-core` in M1.
- When restructuring for M1, move to a workspace: `polaris-core` (lib) +
  `polaris` (bin) so the core stays UI-agnostic.
