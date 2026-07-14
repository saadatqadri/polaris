# Polaris for iPad (Phase 6, i1)

A native SwiftUI `DocumentGroup` app editing `.md` files, over the Rust
`polaris-core` via the uniffi FFI. Design and rationale: [`docs/IOS.md`](../docs/IOS.md).

**Status (2026-07-13):** builds, installs, and runs on the iPad simulator.
The warm-paper page renders in Newsreader / iA Writer Mono, and the word
count in the chrome is `polaris-core` computing it live through the bridge.
Editing is native `TextEditor`; deeper core-sync and the writing modes are
i2+.

## Build & run

Prerequisites: full **Xcode**, `brew install xcodegen`, Rust with
`rustup target add aarch64-apple-ios aarch64-apple-ios-sim`.

```sh
sh apple/setup.sh          # builds the FFI xcframework, stages resources,
                           # generates apple/Polaris.xcodeproj
open apple/Polaris.xcodeproj
```

Then pick an iPad simulator (or your device) and Run. On an Apple-Silicon
Mac, command-line simulator builds need `ARCHS=arm64 EXCLUDED_ARCHS=x86_64`
because the xcframework is arm64-only.

## Layout

- `Polaris/PolarisApp.swift` — the `DocumentGroup` entry point.
- `Polaris/WritingView.swift` — the writing surface + chrome.
- `Polaris/MarkdownDocument.swift` — the `.md` `FileDocument`.
- `Polaris/Theme.swift` — design tokens + fonts (design/DESIGN.md).
- `project.yml` — the xcodegen spec (source of truth for the project).

Generated/derived, not committed (recreated by `setup.sh`):
`Polaris.xcodeproj/`, `Generated/`, `Polaris/Fonts/`, `build/`.
