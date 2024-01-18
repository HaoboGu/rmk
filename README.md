# RMK

[![Crates.io](https://img.shields.io/crates/v/rmk)](https://crates.io/crates/rmk)
[![Docs](https://img.shields.io/docsrs/rmk)](https://docs.rs/rmk/latest/rmk/)
[![Build](https://github.com/haobogu/rmk/actions/workflows/build.yml/badge.svg)](https://github.com/HaoboGu/rmk/actions)

Keyboard firmware for cortex-m, with layer/dynamic keymap/vial support, written in Rust and tested on stm32 and rp2040.

## News

Rmk just released version 0.1.0, migrate to [Embassy](https://github.com/embassy-rs/embassy)! By migrating to Embassy, Rmk now has better async support, supports more MCUs  much easier APIs than before. For examples using Embassy, check [`boards`](https://github.com/HaoboGu/rmk/tree/main/boards) folder!

## Prerequisites

This crate requires **nightly** Rust. `openocd`(stm32) or `probe-rs`(rp2040) is used for flashing & debugging.

## Usage

You can build your own keyboard firmware using RMK or try built-in firmware example for stm32h7 & rp2040.

### Build your own firmware
Example can be found at [`boards`](https://github.com/HaoboGu/rmk/blob/main/boards). The following is a simple
step-to-step instruction for creating your own firmware:

1. Create a rust embedded project, Add rmk to your project using `cargo add rmk`
2. Choose your target, use `rustup target add <your-target-name>` to install the
   target. [Here](https://docs.rust-embedded.org/book/intro/install.html) is the doc for target choosing. For example,
   rp2040 is Cortex-M0+, so its target is `thumbv6m-none-eabi`.
3. Create `.cargo/config.toml` in your project's root, specify your target here.
   See [`boards/stm32h7/.cargo/config.toml`](https://github.com/HaoboGu/rmk/blob/main/boards/stm32h7/.cargo/config.toml)
4. Create `main.rs`, initialize your MCU in rtic's `mod app`, create usb polling task and keyboard task.
   See [`boards/stm32h7/src/main.rs`](https://github.com/HaoboGu/rmk/blob/main/boards/stm32h7/src/main.rs)

### Use built-in example

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
   ```shell
   openocd -f openocd.cfg -c "program target/thumbv7em-none-eabihf/debug/rmk-stm32h7 preverify verify reset exit"
   ```
4. (Optional) Debug firmware using CMSIS-DAP

   Open the project using VSCode, press `F5`, the firmware will be automatically compiled and flashed. A debug session is started after flashing.
   Check [`.vscode/tasks.json`](https://github.com/HaoboGu/rmk/blob/main/.vscode/tasks.json) and [`.vscode/launch.json`](https://github.com/HaoboGu/rmk/blob/main/.vscode/launch.json) for details.

## Roadmap

A lot of todos at the list, any contributions are welcomed :)

- [x] support rp2040
- [x] basic keyboard functions
- [x] layer
- [x] system/media keys
- [x] vial support
- [x] eeprom
- [ ] keyboard macro
- [ ] encoder
- [ ] RGB
- [ ] cli tools

## License
Rmk is licensed under either of

- Apache License, Version 2.0 (LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license (LICENSE-MIT or http://opensource.org/licenses/MIT)

at your option.