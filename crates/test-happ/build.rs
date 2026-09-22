//! Compile the test zomes in `zomes/` to wasm, for `src/lib.rs` to embed.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let zomes_dir = manifest_dir.join("zomes");
    let target_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("zomes-target");

    println!(
        "cargo:rerun-if-changed={}",
        zomes_dir.join("Cargo.toml").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        zomes_dir.join("Cargo.lock").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        zomes_dir.join(".cargo").display()
    );
    for zome in ["integrity", "coordinator"] {
        println!("cargo:rerun-if-changed={}", zomes_dir.join(zome).display());
    }

    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
        .current_dir(&zomes_dir)
        .args(["build", "--release", "--target", "wasm32-unknown-unknown"])
        .arg("--target-dir")
        .arg(&target_dir)
        // The outer build's flags and wrappers (e.g. clippy-driver under
        // `cargo clippy`) are for the host crates, not these wasm builds.
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_BUILD_RUSTFLAGS")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env_remove("CARGO_TARGET_DIR")
        .env_remove("CARGO_BUILD_TARGET")
        .status()
        .expect("failed to run cargo for the test zomes");
    assert!(status.success(), "building the test zomes failed");

    println!(
        "cargo:rustc-env=TEST_ZOMES_WASM_DIR={}",
        target_dir.join("wasm32-unknown-unknown/release").display()
    );
}
