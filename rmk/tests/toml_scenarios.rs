//! TOML scenario suite. Each `run_tests!` line expands one file from
//! `tests/scenarios/` into simulator tests; see `tests/scenarios/README.md`.

pub mod common;

rmk_macro::run_tests!("tests/scenarios/bilateral.toml");
rmk_macro::run_tests!("tests/scenarios/combo.toml");
rmk_macro::run_tests!("tests/scenarios/encoder.toml");
rmk_macro::run_tests!("tests/scenarios/hold_on_other_press.toml");
rmk_macro::run_tests!("tests/scenarios/layer.toml");
rmk_macro::run_tests!("tests/scenarios/macros.toml");
rmk_macro::run_tests!("tests/scenarios/morse.toml");
rmk_macro::run_tests!("tests/scenarios/morse_hrm.toml");
rmk_macro::run_tests!("tests/scenarios/morse_rollover.toml");
rmk_macro::run_tests!("tests/scenarios/one_shot.toml");
rmk_macro::run_tests!("tests/scenarios/permissive_hold.toml");
rmk_macro::run_tests!("tests/scenarios/tap_dance.toml");
