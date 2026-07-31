//! Copies `memory.x` into the linker search path and sets the link args.
//!
//! Unlike the Vial examples, there's no `vial.json` to compress here: Rynk's
//! physical-layout blob is baked from `[layout]` in `keyboard.toml` by the
//! `#[rmk_keyboard]` macro at expansion time, not by this build script.

use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    // Put `memory.x` in our output directory and on the linker search path.
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());
    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(include_bytes!("memory.x"))
        .unwrap();
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rerun-if-changed=keyboard.toml");

    // `--nmagic` is required if memory section addresses are not aligned to 0x10000.
    // See https://github.com/rust-embedded/cortex-m-quickstart/pull/95
    println!("cargo:rustc-link-arg=--nmagic");
    // Link script provided by cortex-m-rt, plus defmt's.
    println!("cargo:rustc-link-arg=-Tlink.x");
    println!("cargo:rustc-link-arg=-Tdefmt.x");
}
