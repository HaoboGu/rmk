---
name: test-changes
description: Test code after changes, to ensure that all changes are good enough and don't break current examples. Use when the changes are done or when the user asks "test changes"
---

When testing code, there are three steps:

1. Run unit test: go to the root of the current project, then run unittests in `rmk` crate: `cd rmk && cargo test --no-default-features --features "storage,std,vial,_ble"`
1. When small changes is finished, go to the root of the current project, then check the following two examples: `cd examples/use_rust/nrf52840_ble_split_dongle && cargo build --release` and `cd examples/use_config/nrf52840_ble_split && cargo build --release`
2. When huge changes is finished, check all examples by running: `sh scripts/clippy_all.sh && sh scripts/check_all.sh`