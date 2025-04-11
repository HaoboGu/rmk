#[path = "./build_common.rs"]
mod common;

use std::path::Path;
use std::process::Command;
use std::{env, fs};

fn main() {
    // Set the compilation config
    let mut cfgs = common::CfgSet::new();
    common::set_target_cfgs(&mut cfgs);

    // Ensure build.rs is re-run if files change
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=build.rs");

    // Get the short hash of the latest Git commit. If it fails, use "unknown"
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

    // Get the current local time and format it
    let now = chrono::Local::now();

    // Combine data and compute CRC32
    let combined = format!("{}_{}", commit_id, now);
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(combined.as_bytes());
    let build_hash = hasher.finalize();

    // Generate file contents
    let contents = format!("pub(crate) const BUILD_HASH: u32 = {:#010x};\n", build_hash);

    // Write to constants.rs in the OUT_DIR
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("constants.rs");
    fs::write(&dest_path, contents).expect("Failed to write build identifier");
}
