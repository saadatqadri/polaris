//! Binding generator entry point:
//!   cargo run --bin uniffi-bindgen generate \
//!     --library <lib> --language swift --out-dir <dir>
fn main() {
    uniffi::uniffi_bindgen_main()
}
