#[path = "./build_common.rs"]
mod common;
#[path = "./schema.rs"]
mod schema;

use std::fs;

use schema::StaticConfig;

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
    // Load config file
    println!("cargo:rerun-if-env-changed=KEYBOARD_TOML_PATH");
    println!("cargo:rerun-if-env-changed=VIAL_JSON_PATH");
    println!("cargo:rerun-if-changed=default_config.toml");

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

    // Load default config
    let s = fs::read_to_string("default_config.toml").expect("no default_config.toml");
    let default_config: StaticConfig = toml::from_str(&s).expect("Parse `default_config.toml` error");

    // TODO: Don't panic when there's no `toml`, aka using Rust
    let toml_path = std::env::var("KEYBOARD_TOML_PATH")
        .expect("\x1b[1;31mERROR\x1b[0m: KEYBOARD_TOML_PATH should be set in `.cargo/config.toml`\n");
    println!("cargo:rerun-if-changed={}", toml_path);

    let _s = match fs::read_to_string(toml_path.clone()) {
        Ok(s) => s,
        Err(e) => {
            panic!("Read keyboard config file {} error: {}", toml_path, e);
        }
    };

    // Parse keyboard config file content to `KeyboardTomlConfig`
    // let toml_config: KeyboardTomlConfig = toml::from_str(&s).expect("Parse `keyboard.toml` error");

    // panic!("num_macro: {}", default_config.num_macros);
}
