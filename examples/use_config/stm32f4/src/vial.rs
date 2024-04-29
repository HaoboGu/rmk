// Use `build.rs` automatically generate vial config, according to `vial.json`
// Please put `vial.json` at your project's root
include!(concat!(env!("OUT_DIR"), "/config_generated.rs"));
