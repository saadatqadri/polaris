# Polaris on iOS — iPad first (Phase 6 design)

> Status: i0–i2 DONE — Polaris runs on the physical iPad; smart
> punctuation works. (i1 on device 2026-07-14, i2 same day.)
> Required by PLAN §5 Phase 6.

## The one hard truth

**iced does not run on iOS.** Everything in `crates/polaris` — the window,
the editor widget, preview, chrome, overlays — is desktop-only and cannot
come along. So "Polaris on iPad" is **not a port; it is a new native
front-end** (SwiftUI/UIKit) over the Rust core.

The good news is exactly why we split the workspace at M1: **`polaris-core`
(and `polaris-drafts`, `polaris-notion`) are pure portable Rust.** They
compile to `aarch64-apple-ios` today. The document model, rope buffer,
grapheme cursors, undo, word count, typography transforms, drafts store,
and publish adapters all cross the bridge unchanged. Only the UI is
rewritten. That is the payoff of "front-ends are replaceable" made real.

## Architecture

```
┌─ SwiftUI app (new) ────────────────────────────┐
│  DocumentGroup over .md files (Files / iCloud)  │
│  editor surface · chrome · preview · drafts UI  │
│  bundled fonts (Newsreader, iA Writer Mono)     │
└──────────────┬──────────────────────────────────┘
               │  FFI (uniffi-generated Swift)
┌──────────────┴──────────────────────────────────┐
│  polaris-core   (buffer, cursor, undo, wc, typo) │
│  polaris-drafts (snapshots, diff)  [later]       │
│  polaris-notion / publish adapters [later]       │
│  compiled to an .xcframework (device + sim)      │
└──────────────────────────────────────────────────┘
```

### FFI: uniffi

[uniffi](https://mozilla.github.io/uniffi-rs/) generates a Swift module
from a Rust interface — the mature Rust↔Swift path. A new thin crate
`crates/polaris-ffi` wraps the core with `#[uniffi::export]` types; `cargo`
+ a build script produce an `.xcframework` (arm64 device + arm64 sim) and a
Swift package. No hand-rolled C ABI.

**Core stays fs-free on iOS.** `Document::open`/`save` do `std::fs`, which
fights the iOS sandbox (security-scoped URLs, `UIDocument`). So the FFI
exposes the **in-memory** document API — `Document::from_str` / `text()` /
edits — and **Swift owns all file I/O** via `DocumentGroup`/`UIDocument`.
Core already supports this (it's how the GUI's untitled buffers work), so
no core changes are needed for the MVP.

### The editor surface: native first (mirror the desktop trajectory)

Desktop went M2 `text_editor`-shim → Phase 2 custom widget. iOS should walk
the same road:

- **MVP:** a native `UITextView` (or SwiftUI `TextEditor`) is the surface;
  `polaris-core::Document` is the model; edits sync core↔view by diff, just
  like the desktop M2 shim (`apply_diff`). This gives real iOS text editing
  — hardware-keyboard support, selection, IME, dictation — *for free* from
  UIKit, and gets a genuine Polaris onto the iPad fast.
- **Later:** a custom `UITextView` subclass for the writing modes
  (typewriter scrolling, focus dimming) and quiet markdown marks — the iOS
  analog of the Phase 2 widget spike. Not needed to start writing.

## Documents: DocumentGroup + Files/iCloud

Polaris iPad is a **`DocumentGroup` app** editing `.md` files. This gives,
from the OS, for free: the Files-app browser, iCloud Drive sync, document
thumbnails, and versioning. Local-first survives intact — the `.md` is the
source of truth; iCloud is *Apple's* sync layer, not ours, and the file
works offline and in any other editor.

Drafts (`.polaris/` sidecar) on iOS is deferred past MVP: sidecar dirs
inside an iCloud document container need care (coordinated writes, the
sidecar must travel with the doc). Design that when drafts land on iOS.

## Typography & design

Same fixed set — bundle the Newsreader and iA Writer Mono TTFs in the app,
register with `UIFont`/`CTFontManager`. Same tokens (warm paper / warm
near-black), same two themes following the system, same one page, same
principles. `design/DESIGN.md` governs iOS too; only the widget toolkit
differs.

## What ports vs. what's rewritten

| Ports as-is (Rust, via FFI) | Rewritten natively (SwiftUI) |
|---|---|
| rope buffer, grapheme cursors, undo/redo | the page / editor surface |
| word count, reading time | chrome, status, overlays |
| smart-punctuation transforms | preview view |
| markdown → Notion blocks | drafts browser |
| drafts store + word diff | gestures, keyboard handling |
| publish adapters | file I/O (DocumentGroup) |

## Milestones (iOS)

- **i0 — FFI spike — DONE (2026-07-13).** `crates/polaris-ffi`: a uniffi
  wrapper (`PolarisDocument`) over core's in-memory editing. **Proven on
  the host**: `swift/check-host.sh` builds the lib, generates the Swift
  bindings, and runs `swift/roundtrip.swift` — real Swift creating a
  document, inserting "héllo 👋 world", undo/redo, reading back — all green.
  The iOS `.xcframework` build (`build-xcframework.sh`) is written and
  waits only on full Xcode + the iOS SDK (Command Line Tools lack it). The
  bridge itself is confirmed sound.
- **i1 — The page — DONE (2026-07-13), simulator-verified.** `apple/`: a
  SwiftUI DocumentGroup app editing `.md`, native `TextEditor` surface,
  warm-paper tokens, Newsreader + iA Writer Mono bundled, chrome word
  count driven by `polaris-core` through the FFI. Builds, installs, and
  runs on the iPad Pro simulator (screenshots in the i1 commit). Built via
  xcodegen (`apple/setup.sh`). **Remaining for the owner:** run on the
  physical iPad (needs the Apple Developer account) — the code is done.
  Autosave through UIDocument + full core-sync are i2.
- **i2 — DONE (2026-07-14), simulator-verified.** An owned `UITextView`
  (`PolarisTextView`, UIViewRepresentable) replaces SwiftUI's `TextEditor`,
  so smart punctuation runs at input time through `polaris-core`
  (`smart_substitution` FFI): typed `--`→—, `...`→…, quotes curl, skipped
  in code — confirmed on the iPad simulator. Live word count via the
  stateless `word_count` FFI. Autosave is DocumentGroup's (edits write
  back to the bound text; iOS persists). The code-context guard moved into
  `polaris-core::typography` (`substitute_in_context`), shared by desktop +
  iOS. Remaining: undo/selection through core + writing modes need the
  fuller custom text view (i3+).
- **i3 — Preview.** Reuse the markdown pipeline (FFI → attributed string).
- **i4 — Drafts + publish.** FFI to `polaris-drafts`; the publish targets.
- **Later:** custom text view for writing modes; iPhone layout.

## Prerequisites (owner) — the honest gate

I can write all the Rust, the FFI, and the Swift. I **cannot** build, sign,
or deploy to your physical iPad from here — that runs on your Mac through
Xcode. So before i1 can reach the device you need:

1. **An Apple Developer account** ($99/yr) — required for on-device
   installs beyond 7-day free provisioning, and for TestFlight (how your
   friend would test).
2. **Xcode** on the Mac.
3. iOS Rust targets: `rustup target add aarch64-apple-ios
   aarch64-apple-ios-sim`.

With those, the loop is: `cargo` builds the xcframework → Xcode builds the
SwiftUI app against it → run on the iPad (or TestFlight).

## Risks

- **FFI ergonomics / edit throughput** — sending the whole document string
  per keystroke is fine at prose sizes; revisit only if a huge doc lags.
- **Writing modes need a custom text view** — the biggest later effort,
  same as the desktop widget was. MVP dodges it with native editing.
- **Drafts + iCloud sidecar** — coordinated writes; deferred, design later.
- **Two front-ends to maintain** — desktop (iced) and iOS (SwiftUI) share
  the core but diverge in UI. Accepted cost; the core is the moat.

## Recommendation

Start with **i0 (the FFI spike)** — it's small, it proves uniffi +
xcframework + Swift works against `polaris-core`, and it's the honest
gate before committing to i1. In parallel, get the Apple Developer account
so i1 can reach the iPad the moment it's built. Everything after i0 is
"real app," but i0 tells us the bridge is sound.
