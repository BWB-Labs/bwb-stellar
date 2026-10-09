//! Tells the tests whether the upgrade fixture's WASM is built. The upgrade
//! test imports that binary at compile time; without it, a plain `cargo test`
//! would fail to compile. Instead, a single test fails with a hint to run
//! `scripts/test.sh`, which builds the fixture first and copies it to
//! `target/fixtures/` at the workspace root, a fixed place whatever
//! CARGO_TARGET_DIR says.
use std::path::Path;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(fixture_missing)");
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let wasm = Path::new(&manifest).join("../../target/fixtures/offer_token_v2_fixture.wasm");
    println!("cargo::rerun-if-changed={}", wasm.display());
    if !wasm.exists() {
        println!("cargo::rustc-cfg=fixture_missing");
    }
}
