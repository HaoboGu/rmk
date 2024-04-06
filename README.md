# RMK

[![Crates.io](https://img.shields.io/crates/v/rmk)](https://crates.io/crates/rmk)
[![Docs](https://img.shields.io/docsrs/rmk)](https://docs.rs/rmk/latest/rmk/)
[![Build](https://github.com/haobogu/rmk/actions/workflows/build.yml/badge.svg)](https://github.com/HaoboGu/rmk/actions)
[![Discord](https://img.shields.io/discord/1166665039793639424?label=discord)](https://discord.gg/HHGA7pQxkG)

[中文](https://github.com/HaoboGu/rmk/blob/main/README_zh.md)

A feature-rich Rust keyboard firmware. 

## Features

- **Support a wide range of microcontrollers**: Powered by [embassy](https://github.com/embassy-rs/embassy), RMK supports a wide range of microcontrollers, such as stm32/nrf/rp2040
- **Real-time keymap editing**: RMK has built-in [vial](https://get.vial.today) support, the keymap can be changed on-the-fly
- **Advanced keyboard features**: Many advanced keyboard features are available by default in RMK, such as layer switch, media control, system control, mouse control, etc
- **Wireless**: (Experimental) BLE wireless support with auto-reconnection/multiple devices feature for nrf52 microcontrollers, tested on nrf52840

## News

- [2024.03.07] BLE support with auto-reconnection/multiple devices feature for nrf52840/nrf52832 has beed added to RMK! Checkout [boards/nrf52840_ble](https://github.com/HaoboGu/rmk/blob/main/boards/nrf52840_ble/src/main.rs) and [boards/nrf52832_ble](https://github.com/HaoboGu/rmk/blob/main/boards/nrf52832_ble/src/main.rs) for details.

- [2024.02.18] Version `0.1.4` is just released! This release contains a new [build script](https://github.com/HaoboGu/rmk/blob/main/boards/stm32h7/build.rs) for generating vial config, minor API update and a brand new [user documentation page](https://haobogu.github.io/rmk).

<details>

<summary>Click to checkout more news</summary>

- [2024.01.26] 🎉[rmk-template](https://github.com/HaoboGu/rmk-template) is released! Now you can create your own keyboard firmware with a single command: `cargo generate --git https://github.com/HaoboGu/rmk-template`

- [2024.01.18] RMK just released version `0.1.0`! By migrating to [Embassy](https://github.com/embassy-rs/embassy), RMK now has better async support, more supported MCUs and much easier usages than before. For examples, check [`boards`](https://github.com/HaoboGu/rmk/tree/main/boards) folder!

</details>

## [User Documentation](https://haobogu.github.io/rmk/guide_overview.html) 

## [API Reference](https://docs.rs/rmk/latest/rmk/)

## Usage

### Option 1: Initialize from template
You can use [rmk-template](https://github.com/HaoboGu/rmk-template) to initialize your project.

```shell
cargo install cargo-generate
cargo generate --git https://github.com/HaoboGu/rmk-template
```

Then follow the steps in generated `README.md`. Check RMK's [User Guide](https://haobogu.github.io/rmk/guide_overview.html) for details.

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

3. Flash

   If your rp2040 board is connected with a debugging probe, just use the following command to flash RMK firmware to the board:

   ```shell
   cd boards/rp2040
   cargo run
   ```

   If you don't have a debugging probe, you can use `elf2uf2-rs` to flash your firmware via USB. There are several additional steps you have to do:

   1. Install `elf2uf2-rs`: `cargo install elf2uf2-rs`
   2. Update `boards/rp2040/.cargo/config.toml`, use `elf2uf2` as the flashing tool
      ```diff
      - runner = "probe-rs run --chip RP2040"
      + runner = "elf2uf2-rs -d"
      ```
   3. Connect your rp2040 board holding the BOOTSEL key, ensure that rp's USB drive appears
   4. Flash
      ```shell
      cd boards/rp2040
      cargo run
      ```
      Then, you will see logs like if everything goes right:
      ```shell
      Finished release [optimized + debuginfo] target(s) in 0.21s
      Running `elf2uf2-rs -d 'target\thumbv6m-none-eabi\release\rmk-rp2040'`
      Found pico uf2 disk G:\
      Transfering program to pico
      173.00 KB / 173.00 KB [=======================] 100.00 % 193.64 KB/s  
      ```

#### stm32h7

1. Install [openocd](https://github.com/openocd-org/openocd)

2. Build the firmware

   ```shell
   cd boards/stm32h7
   cargo build
   ```

3. Flash

   Make sure you have a debugging probe connected to your board. You can use both `probe-rs` and `openocd` to flash the firmware: 

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

## [Roadmap](https://haobogu.github.io/rmk/roadmap.html)

Current roadmap of RMK can be found [here](https://haobogu.github.io/rmk/roadmap.html).

## Minimum Supported Rust Version (MSRV)

This crate requires stable Rust 1.75 and up. 

## License

RMK is licensed under either of

- Apache License, Version 2.0 (LICENSE-APACHE or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license (LICENSE-MIT or <http://opensource.org/licenses/MIT>)

at your option.
