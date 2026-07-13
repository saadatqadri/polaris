#!/bin/sh
# i0 host proof: build polaris-ffi for the Mac, generate the Swift bindings,
# and run roundtrip.swift against the real polaris-core — no iOS SDK needed.
# Verifies the bridge itself; the iOS build (build-xcframework.sh) needs Xcode.
set -eu

cd "$(dirname "$0")/../../.."   # repo root
GEN=$(mktemp -d)
trap 'rm -rf "$GEN"' EXIT

cargo build -p polaris-ffi
cargo run -q --bin uniffi-bindgen generate \
  --library target/debug/libpolaris_ffi.dylib \
  --language swift --out-dir "$GEN"
mv "$GEN/polaris_ffiFFI.modulemap" "$GEN/module.modulemap"

# Top-level code must live in main.swift when compiling multiple files.
cp crates/polaris-ffi/swift/roundtrip.swift "$GEN/main.swift"
swiftc -O -I "$GEN" -L target/debug -lpolaris_ffi \
  -Xcc -fmodule-map-file="$GEN/module.modulemap" \
  "$GEN/polaris_ffi.swift" "$GEN/main.swift" \
  -o "$GEN/roundtrip"

DYLD_LIBRARY_PATH=target/debug "$GEN/roundtrip"
