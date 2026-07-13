#!/bin/sh
# Build polaris-ffi into an iOS .xcframework + Swift bindings (docs/IOS.md).
#
# Requires FULL Xcode (not just Command Line Tools) and the iOS Rust targets:
#   rustup target add aarch64-apple-ios aarch64-apple-ios-sim
#
# Output: target/ios/PolarisFFI.xcframework  and  target/ios/Generated/polaris_ffi.swift
# Add both to the SwiftUI app target in Xcode.
set -eu

cd "$(dirname "$0")/../.."   # repo root
CRATE=polaris_ffi
OUT=target/ios
GEN=$OUT/Generated
HEADERS=$OUT/Headers

rustup target add aarch64-apple-ios aarch64-apple-ios-sim >/dev/null 2>&1 || true

echo "Building static libs (device + simulator)…"
cargo build -p polaris-ffi --release --target aarch64-apple-ios
cargo build -p polaris-ffi --release --target aarch64-apple-ios-sim

echo "Generating Swift bindings…"
rm -rf "$GEN"; mkdir -p "$GEN"
cargo run -q --bin uniffi-bindgen generate \
  --library "target/aarch64-apple-ios/release/lib${CRATE}.a" \
  --language swift --out-dir "$GEN"

# xcframework wants the modulemap named module.modulemap alongside the header.
rm -rf "$HEADERS"; mkdir -p "$HEADERS"
cp "$GEN/${CRATE}FFI.h" "$HEADERS/"
cp "$GEN/${CRATE}FFI.modulemap" "$HEADERS/module.modulemap"

echo "Packaging xcframework…"
rm -rf "$OUT/PolarisFFI.xcframework"
xcodebuild -create-xcframework \
  -library "target/aarch64-apple-ios/release/lib${CRATE}.a" -headers "$HEADERS" \
  -library "target/aarch64-apple-ios-sim/release/lib${CRATE}.a" -headers "$HEADERS" \
  -output "$OUT/PolarisFFI.xcframework"

echo ""
echo "✓ $OUT/PolarisFFI.xcframework"
echo "✓ $GEN/${CRATE}.swift   (add this source + the xcframework to the app target)"
