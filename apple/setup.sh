#!/bin/sh
# One-shot setup for the Polaris iPad app (docs/IOS.md, i1).
# Builds the Rust FFI xcframework, stages the generated Swift binding and the
# bundled fonts, and generates the Xcode project. Run from anywhere.
#
# Requires: full Xcode, xcodegen (brew install xcodegen), and the iOS Rust
# targets (the framework script adds them).
set -eu

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"

echo "1/4  Building the Rust FFI xcframework…"
sh "$ROOT/crates/polaris-ffi/build-xcframework.sh"

echo "2/4  Staging the generated Swift binding…"
mkdir -p "$HERE/Generated"
cp "$ROOT/target/ios/Generated/polaris_ffi.swift" "$HERE/Generated/"

echo "3/4  Staging bundled fonts…"
mkdir -p "$HERE/Polaris/Fonts"
cp "$ROOT/crates/polaris/assets/fonts/Newsreader16pt-Regular.ttf" \
   "$ROOT/crates/polaris/assets/fonts/Newsreader16pt-Italic.ttf" \
   "$ROOT/crates/polaris/assets/fonts/Newsreader16pt-SemiBold.ttf" \
   "$ROOT/crates/polaris/assets/fonts/iAWriterMonoS-Regular.ttf" \
   "$HERE/Polaris/Fonts/"

echo "4/4  Generating the Xcode project…"
( cd "$HERE" && xcodegen generate )

echo ""
echo "✓ open $HERE/Polaris.xcodeproj  — build & run on an iPad or the simulator."
echo "  (Simulator on Apple Silicon: build with ARCHS=arm64 EXCLUDED_ARCHS=x86_64.)"
