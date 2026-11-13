//! `uniffi-bindgen`, built from the crates.io `uniffi` release the workspace uses, so the
//! generator always matches the scaffolding in `jf-ffi` (UniFFI checks this with checksums).
//!
//! `cargo xtask bindings` runs it; by hand:
//! `cargo run -p jf-ffi-bindgen --bin uniffi-bindgen -- generate --language kotlin --out-dir <dir> <path to libjf_ffi>`.

fn main() {
    uniffi::uniffi_bindgen_main()
}
