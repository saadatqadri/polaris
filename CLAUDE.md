# CLAUDE.md — Polaris

Polaris is a **local-first markdown editor for distraction-free, human
writing**, with publish-anywhere syndication (Notion today; Hugo, Substack,
more next). Rust core, native front-ends. This file is the condensed
handover; the full plan is **`docs/PLAN.md`**.

## Product principles (non-negotiable)

1. **Typography is the product.** Fixed, bundled faces — **Newsreader 16pt**
   (writing + preview) and **iA Writer Mono** (chrome/code). No font or
   appearance configuration, ever.
2. **Every word is human.** AI must never compose, autocomplete, or
   ghost-write into the buffer. Future AI may only annotate in a margin when
   summoned, on a marked draft — **`docs/AI.md`** is binding (critique/cuts
   only, never replacement prose; no machine words in the buffer as an
   architectural invariant). Do not add AI features unprompted.
3. **Chrome recedes.** Minimal UI that fades while typing. No panels,
   toolbars, or badges in the writing surface.
4. **Local-first.** Plain `.md` on disk is the source of truth. Cloud is a
   publish target, never a home.
5. **The goal:** make the writer *better* — write more, revise more, honest
   critique. See PLAN §1.

Design source of truth: **`design/DESIGN.md`** (tokens, type, keyboard maps,
iPad interaction) + **`design/mockup.html`**.

## Where we are (2026-07-16)

**Desktop (Rust/iced) — Phases 1–3 COMPLETE, released `v0.2.3`; Phase 4 P1
(publish layer) shipped since.**
- Cargo workspace: `polaris-core` (rope buffer, grapheme cursors, grouped
  undo, autosave policy, word count, typography), `polaris-notion`
  (markdown→blocks + API), `polaris-drafts` (content-addressed snapshots +
  word diff), `polaris-publish` (the `Target` trait + Notion/Hugo adapters —
  Phase 4 P1), `polaris-ffi` (uniffi bridge for iOS), `polaris` (the iced
  GUI + clap CLI).
- **Phase 1:** the editor — silent autosave, find, save-as, rename, deploy
  to Notion, preview, themes.
- **Phase 2:** custom editor widget (Document is the single source of truth;
  IME, selection, clipboard, quiet markdown marks, steady caret). Writing
  modes: typewriter (Cmd+Y), focus dim (Cmd+G), Hemingway (Cmd+E), zen
  (Cmd+K), session goals (Cmd+L).
- **Phase 3:** drafts — Cmd+M marks (kept forever), auto-snapshots, Cmd+Shift+M
  browser with word-level diffs, one-key restore (undoable). Per docs/DRAFTS.md.
- Distribution: `install.sh` + tag-triggered GitHub Releases
  (`.github/workflows/release.yml`). Welcome tour on first run / `polaris welcome`.
- Nav polish shipped: visual-line up/down, Cmd/Option arrow + delete
  conventions, fallback-glyph stick fix. CI (fmt, clippy -D warnings, test on
  Linux+macOS) is green and gates every push.

**iPad (SwiftUI over the Rust core) — Phase 6 pulled forward, i0–i3 + modes
DONE, running on the owner's physical iPad.** (`apple/`, docs/IOS.md.)
- Native SwiftUI DocumentGroup app over `polaris-core` via uniffi FFI (iced
  has no iOS target — `apple/` is a **separate front-end**; keep independent).
- i0 FFI spike · i1 the page (Newsreader, warm paper, core-driven word count)
  · i2 owned `UITextView` + smart punctuation via `smart_substitution` FFI ·
  i3 preview via `render_preview` FFI.
- **Writing modes on iPad:** a quiet floating **✧** control (bottom-trailing)
  summons a menu — **Preview** (⌘P), **Typewriter** (⌘Y), **Hemingway** (⌘E),
  all working; **Focus** deferred (needs per-paragraph dimming / a custom text
  surface). Keyboard iPads get the same ⌘ shortcuts (hold-⌘ overlay documents
  them). iPad interaction model decided in DESIGN.md (2026-07-14).
- Build: `sh apple/setup.sh` → `open apple/Polaris.xcodeproj`. Device build:
  `xcodebuild … -sdk iphoneos -destination 'platform=iOS,id=<udid>'
  -allowProvisioningUpdates` then `xcrun devicectl device install/launch`.
  Sim on Apple Silicon needs `ARCHS=arm64 EXCLUDED_ARCHS=x86_64`. Team ID
  UQPYK46RBW baked into project.yml. Generated project/bindings/fonts are
  gitignored (setup.sh recreates them).

## What's next — Phase 4 + the rest of the MVP

Phase 4 is the big remaining MVP arc. Design doc: **`docs/PHASE4.md`**
(approved 2026-07-16). **P1 shipped** — the rest is open.
- **Write Once, Publish Anywhere** (owner direction, eventual business model).
  **P1 done:** new `polaris-publish` crate — one `Target` trait (markdown
  `Doc` in, `Outcome` out), Notion + Hugo adapters. Cmd+D goes through a
  config-built registry (one target fires through, ≥2 show a ✧ picker);
  `polaris publish [--to id] [--force]` CLI; `[hugo]` + `default_target` in
  `~/.polaris.toml` (`[notion]` unchanged); `deploy` kept as the Notion
  append/replace path. Hugo = front matter + write into `content/`, strips
  the leading title H1, no git automation. **Next targets (P2):** Substack
  (v1 format-and-paste → clipboard, then email-to-draft), HTML/PDF, LinkedIn
  (format+copy). Clipboard plumbing (`Outcome::Clipboard { hint, body }`,
  app does the copy) is in place, unused until P2.
- **Accept/reject editing workflow** (P3) — import an edited copy → word-level
  diff → accept/reject each change (Draft's model, no server), reusing the
  `polaris-drafts` diff. Bound up with the AI.md critique pass (Phase 4+).
- **Part C — Preview additions, both shipped:** the reading pointer (P5 —
  keeps your place + arrow-key nav, round-trips the caret on Cmd+P) and
  **inline notes** (P6 — N adds a block note in preview, [/] jump, x resolve,
  Shift+X delete, Cmd+Shift+N hide; `NoteStore` in `polaris-drafts` persists
  `.polaris/<name>/notes/live.json`, re-anchors by quote, Cmd+M freezes notes
  with the draft). The human-first face of the AI.md margin — notes live in
  the sidecar, never the buffer. v1 is block-granular; sub-sentence anchors +
  true right-margin layout deferred.
- These halves are independent of each other and of iOS.

iOS follow-ons (not blocking Phase 4): Focus mode (custom text surface),
undo/selection through core, drafts + publish on iPad, TestFlight for the
friend.

## Known gaps / debt

- **Desktop:** markdown source marks are quiet in write mode ✓ (custom
  widget); preview scroll is caret-ratio approximate; theme override persists
  to `~/.polaris.toml` (no live OS-theme following); Cmd+Q via the macOS app
  menu may bypass close-flush (unverified). Notion: images/links → plain text.
  Preview mermaid = labeled source, not rendered (deliberate — a JS
  engine/webview is against the design).
- **iPad:** Focus mode not wired (needs custom surface); editing is native
  `UITextView` with core driving typography + word count (undo/selection are
  UIKit's, not core's yet); autosave is DocumentGroup's.

## Working conventions

- Direct commits to `main` (owner's choice). CI must stay green: `cargo fmt
  --all --check && cargo clippy --workspace --all-targets -- -D warnings &&
  cargo test --workspace`. **Heads-up:** CI's clippy is often newer than a
  local toolchain and catches lints (collapsible_match, if_same_then_else) a
  stale local clippy misses — run `rustup update` or expect a fixup commit.
- Desktop: `cargo run -- <file.md>`. Verify GUI changes by launching (the
  headless `gui/mod.rs` tests can't see rendering).
- Release: bump workspace version, tag `v0.x.y`, push tag → installer serves it.
- iOS: see build commands above; simulator smoke via `xcrun simctl`, on-device
  via `devicectl`. Synthetic taps don't reach the sim's Metal view (use a
  harness or keyboard events); the owner tests real touch.
- End commit messages with the Co-Authored-By + Claude-Session trailers.
