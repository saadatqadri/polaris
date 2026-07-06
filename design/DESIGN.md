# Polaris Design — Phase 1 (GUI)

> The tools should hold still so the writing can move.

An interactive mock of everything below lives in [`design/mockup.html`](./mockup.html) —
open it in a browser, type in it, toggle the theme and preview.

## Principles

1. **One good page.** A single warm column of text, centered, with real margins.
   No panels, no sidebars, no toolbars. The document is the interface.
2. **Typography is the product.** Fonts are bundled into the binary and cannot
   be changed. There is no font setting, no theme marketplace, no plugin API
   for appearance. The constraint is the design.
3. **Chrome recedes.** Filename, save state, and word count exist — but they
   fade out while you type and return when you rest. Nothing animates, blinks,
   or badges while words are arriving.
4. **Every word is human.** AI never composes, autocompletes, or ghost-writes.
   (No AI ships in Phase 1 at all; when it arrives, it may only annotate in the
   margin when explicitly summoned — never touch the buffer.)

## Typefaces (fixed set, embedded via `include_bytes!`)

| Face | License | Role |
|------|---------|------|
| **Newsreader (16pt optical size)** | SIL OFL | Writing mode. Editorial serif with the finest italic of the audition. (Revised 2026-07-06 after daylight use; previously Instrument Sans, originally iA Writer Quattro.) |
| **iA Writer Mono** | SIL OFL | Chrome (status line), code blocks, source-literal contexts. |
| **Literata** | SIL OFL | Preview / reading mode. |

No user-facing font configuration. Ever.

## Design tokens

| Token | Light | Dark | Used for |
|-------|-------|------|----------|
| `bg` | `#FBFAF7` | `#1A1916` | Page. Warm paper / warm near-black — never pure white or black. |
| `ink` | `#24221D` | `#DAD6CC` | Body text. |
| `quiet` | `#A9A498` | `#635F54` | Chrome, markdown syntax marks (`#`, `**`, `>`, `-`). |
| `whisper` | `#DEDAD1` | `#33312B` | Rules, blockquote bars. |
| `star` | `#4E6E8E` | `#8FAECB` | **The only accent.** Cursor and selection. Nothing else. |

Two themes, both fixed. Theme follows the OS by default; one toggle, no editor.

## Layout & type

- Measure: **62ch** max-width, centered. Text never spans the window.
- Body: Newsreader 19px / **1.56** line-height (Newsreader runs optically
  small; 19px matches the old 17.5px sans).
- Top margin ~16vh; bottom padding ~30vh so the cursor never writes at the
  screen's edge (typewriter scrolling lands in Phase 2).
- Headings stay calm: same family, bold, H1 ≈ 1.22em — headings organize,
  they don't shout.
- Markdown marks render in `quiet` at normal weight; the content they wrap
  renders styled (bold, italic). Source is always visible, never hidden.

## Interaction

- **Autosave, silently.** Debounced ~1s after last keystroke. A small `● saved`
  breathes in the top-right chrome. No dirty flag, no save dialogs, no anxiety.
  (Ctrl+S still works and saves immediately — habits deserve respect.)
- **Chrome fade:** on keystroke the chrome fades over 0.6s; 1.2s after the last
  keystroke it returns.
- **Smart typography as you type:** `"` → “ ”, `'` → ‘ ’, `--` → —, `...` → ….
  Applied at input time so the file itself carries the real characters.
- **Preview** (`Ctrl+P`): the same column re-set in Literata, markdown rendered.
  A mode switch, not a split — one page, one focus.

## Keyboard map (Phase 1)

| Key | Action |
|-----|--------|
| `Ctrl+S` | Save now (autosave runs regardless) |
| `Ctrl+P` | Toggle write / preview |
| `Ctrl+T` | Toggle light / dark (the one theme control; follows the OS at launch) |
| `Ctrl+F` | Find |
| `Ctrl+Z` / `Ctrl+Shift+Z` | Undo / redo |
| `Ctrl+←/→` | Word jump |
| `Ctrl+D` | Deploy to Notion |
| `Ctrl+Q` | Quit |

## Implementation notes (iced)

- Fonts load once at startup from embedded bytes; `cosmic-text` (iced's text
  stack) handles shaping, so Quattro/Literata render correctly incl. ligatures.
- The editor view is a custom widget over `polaris-core`'s rope buffer —
  soft wrap comes from real text layout.
- Preview reuses the existing `pulldown-cmark` pipeline, rendering to styled
  iced text spans rather than HTML.
- Window chrome: native decorations for now; frameless is a later decision.

## Explicitly not in Phase 1

Focus/Hemingway/zen modes (Phase 2) · drafts & versioning (Phase 3) ·
collaboration review (Phase 4) · export formats · AI of any kind ·
settings beyond the theme following your OS.
