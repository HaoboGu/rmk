#[path = "./build_common.rs"]
mod common;

use std::path::Path;
use std::process::Command;
use std::{env, fs};

fn main() {
    // Set the compilation target configuration
    let mut cfgs = common::CfgSet::new();
    common::set_target_cfgs(&mut cfgs);

    println!("cargo:rerun-if-changed=build.rs");

    // Compute build hash and write to constants.rs
    let build_hash = compute_build_hash();
    let constants = format!(
        "#[allow(clippy::redundant_static_lifetimes)]\npub(crate) const BUILD_HASH: u32 = {build_hash:#010x};\n"
    );

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("constants.rs");
    fs::write(&dest_path, constants).expect("Failed to write constants.rs file");
}
fn compute_build_hash() -> u32 {
    // Get the short hash of the latest Git commit. Use "unknown" if it fails
    let commit_id = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string());

    // Get and format current local time
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    // Combine data and compute CRC32
    let combined = format!("{commit_id}_{now}");
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(combined.as_bytes());
    hasher.finalize()
}
