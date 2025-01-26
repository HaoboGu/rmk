#[path = "./build_common.rs"]
mod common;
#[path = "./schema.rs"]
mod schema;

use std::fs;

use schema::StaticConfig;

fn main() {
    // Set cfg flags for target of MCU
    let mut cfgs = common::CfgSet::new();
    common::set_target_cfgs(&mut cfgs);

    // Load config file
    println!("cargo:rerun-if-env-changed=KEYBOARD_TOML_PATH");
    println!("cargo:rerun-if-env-changed=VIAL_JSON_PATH");
    println!("cargo:rerun-if-changed=default_config.toml");

    // Load default config
    let s = fs::read_to_string("default_config.toml").expect("no default_config.toml");
    let default_config: StaticConfig =
        toml::from_str(&s).expect("Parse `default_config.toml` error");

    let toml_path = std::env::var("KEYBOARD_TOML_PATH").expect(
        "\x1b[1;31mERROR\x1b[0m: KEYBOARD_TOML_PATH should be set in `.cargo/config.toml`\n",
    );
    println!("cargo:rerun-if-changed={}", toml_path);

    let _s = match fs::read_to_string(toml_path.clone()) {
        Ok(s) => s,
        Err(e) => {
            panic!("Read keyboard config file {} error: {}", toml_path, e);
        }
    };

    // Parse keyboard config file content to `KeyboardTomlConfig`
    // let toml_config: KeyboardTomlConfig = toml::from_str(&s).expect("Parse `keyboard.toml` error");

    panic!("num_macro: {}", default_config.num_macros);
}
