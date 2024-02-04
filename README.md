# RMK

[![Crates.io](https://img.shields.io/crates/v/rmk)](https://crates.io/crates/rmk)
[![Docs](https://img.shields.io/docsrs/rmk)](https://docs.rs/rmk/latest/rmk/)
[![Build](https://github.com/haobogu/rmk/actions/workflows/build.yml/badge.svg)](https://github.com/HaoboGu/rmk/actions)

Keyboard firmware with layer/dynamic keymap/[vial](https://get.vial.today) support, written in Rust.

> RMK is under active development, breaking changes may happen. If you have any problem please check the latest doc or file an issue.

## News

- [2024.01.26] 🎉[rmk-template](https://github.com/HaoboGu/rmk-template) is released! Now you can create your own keyboard firmware with a single command: `cargo generate --git https://github.com/HaoboGu/rmk-template`

- [2024.01.18] RMK just released version `0.1.0`! By migrating to [Embassy](https://github.com/embassy-rs/embassy), RMK now has better async support, more supported MCUs and much easier usages than before. For examples using Embassy, check [`boards`](https://github.com/HaoboGu/rmk/tree/main/boards) folder!

## Prerequisites

This crate requires stable Rust 1.75 and up. `openocd`(stm32) or `probe-rs`(stm32/rp2040) is used for flashing & debugging.

## Usage

### Option 1: Initialize from template
You can use [rmk-template](https://github.com/HaoboGu/rmk-template) to initialize your project.

```
cargo install cargo-generate
cargo generate --git https://github.com/HaoboGu/rmk-template
```

### Option 2: Try built-in examples

Example can be found at [`boards`](https://github.com/HaoboGu/rmk/blob/main/boards). The following is a simple
step-to-step instruction for rp2040 and stm32h7

#### rp2040

1. Install [probe-rs](https://github.com/probe-rs/probe-rs)

   ```shell
   cargo install probe-rs --features cli
   ```

2. Build the firmware

   ```shell
   cd boards/rp2040
   cargo build
   ```

3. Flash it

   ```shell
   cd boards/rp2040
   cargo run
   ```

#### stm32h7

1. Install [openocd](https://github.com/openocd-org/openocd)

2. Build the firmware

   ```shell
   cd boards/stm32h7
   cargo build
   ```

3. Flash

   You can use both `probe-rs` and `openocd` to flash the firmware: 

   ```shell
   # Use openocd
   openocd -f openocd.cfg -c "program target/thumbv7em-none-eabihf/debug/rmk-stm32h7 preverify verify reset exit"
   
   # Use probe-rs
   cd boards/stm32h7
   cargo run
   ```

4. (Optional) Debug firmware using CMSIS-DAP

   Open the project using VSCode, choose `Cortex-Debug - stm32h7` debug profile, then press `F5`, the firmware will be automatically compiled and flashed. A debug session is started after flashing.
   Check [`.vscode/tasks.json`](https://github.com/HaoboGu/rmk/blob/main/.vscode/tasks.json) and [`.vscode/launch.json`](https://github.com/HaoboGu/rmk/blob/main/.vscode/launch.json) for details.

## Roadmap

A lot of todos at the list, any contributions are welcomed :)

- [x] layer
- [x] system/media keys
- [x] vial support
- [x] eeprom
- [x] project template
- [x] mouse key
- [ ] keyboard macro
- [ ] encoder
- [ ] RGB
- [ ] wireless

## License

RMK is licensed under either of

- Apache License, Version 2.0 (LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license (LICENSE-MIT or http://opensource.org/licenses/MIT)

at your option.
