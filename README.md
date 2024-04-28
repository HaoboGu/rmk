# RMK

[![Crates.io](https://img.shields.io/crates/v/rmk)](https://crates.io/crates/rmk)
[![Docs](https://img.shields.io/docsrs/rmk)](https://docs.rs/rmk/latest/rmk/)
[![Build](https://github.com/haobogu/rmk/actions/workflows/build.yml/badge.svg)](https://github.com/HaoboGu/rmk/actions)
[![Discord](https://img.shields.io/discord/1166665039793639424?label=discord)](https://discord.gg/HHGA7pQxkG)

[中文](https://github.com/HaoboGu/rmk/blob/main/README_zh.md)

A feature-rich Rust keyboard firmware. 

## Features

- **Support a wide range of microcontrollers**: Powered by [embassy](https://github.com/embassy-rs/embassy), RMK supports a wide range of microcontrollers, such as stm32/nrf/rp2040/esp32
- **Real-time keymap editing**: RMK has built-in [vial](https://get.vial.today) support, the keymap can be changed on-the-fly
- **Advanced keyboard features**: Many advanced keyboard features are available by default in RMK, such as layer switch, media control, system control, mouse control, etc
- **Wireless**: (Experimental) BLE wireless support with auto-reconnection/multiple devices feature for nrf52 and esp32 microcontrollers, tested on nrf52840 and esp32c3
- **Easy configuration**: RMK provides a simple way to build your keyboard: a `keyboard.toml` is all you need! For experienced Rust user, you can still customize your firmware easily using RMK

## News

- [2024.05.01] RMK's new configuration system is available at main branch! This new feature brings a totally new way to build your keyboard firmware: using a config file `keyboard.toml`. The document can be found [here](https://haobogu.github.io/rmk/configuration.html), you can also check out the [`examples`](https://github.com/HaoboGu/rmk/blob/main/examples/) folder for both config way and rust way to use RMK.

- [2024.04.07] BLE support for esp32 is available now on main branch, you can try the example at [`examples/use_rust/esp32c3_ble`](https://github.com/HaoboGu/rmk/blob/main/examples/use_rust/esp32c3_ble/src/main.rs) and [`examples/use_rust/esp32s3_ble`](https://github.com/HaoboGu/rmk/blob/main/examples/use_rust/esp32s3_ble/src/main.rs). It will be released to crates.io soon, after some additional testing.

- [2024.03.07] BLE support with auto-reconnection/multiple devices feature for nrf52840/nrf52832 has been added to RMK! Checkout [`examples/use_rust/nrf52840_ble`](https://github.com/HaoboGu/rmk/blob/main/examples/use_rust/nrf52840_ble/src/main.rs) and [`examples/use_rust/nrf52832_ble`](https://github.com/HaoboGu/rmk/blob/main/examples/use_rust/nrf52832_ble/src/main.rs) for details.

<details>

<summary>Click to checkout more news</summary>

- [2024.02.18] Version `0.1.4` is just released! This release contains a new [build script](https://github.com/HaoboGu/rmk/blob/main/examples/use_rust/stm32h7/build.rs) for generating vial config, minor API update and a brand new [user documentation page](https://haobogu.github.io/rmk).

- [2024.01.26] 🎉[rmk-template](https://github.com/HaoboGu/rmk-template) is released! Now you can create your own keyboard firmware with a single command: `cargo generate --git https://github.com/HaoboGu/rmk-template`

- [2024.01.18] RMK just released version `0.1.0`! By migrating to [Embassy](https://github.com/embassy-rs/embassy), RMK now has better async support, more supported MCUs and much easier usages than before. For examples, check [`examples`](https://github.com/HaoboGu/rmk/tree/main/examples) folder!

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

Example can be found at [`examples`](https://github.com/HaoboGu/rmk/blob/main/examples). The following is a simple
step-to-step instruction for rp2040. For other microcontrollers, the steps should be identical with a debug probe.

#### rp2040

1. Install [probe-rs](https://github.com/probe-rs/probe-rs)

   ```shell
   cargo install probe-rs --features cli
   ```

2. Build the firmware

   ```shell
   cd examples/use_rust/rp2040
   cargo build
   ```

3. Flash using debug probe

   If you have a debug probe connected to your rp2040 board, flashing is quite simple: run the following command to automatically compile and flash RMK firmware to the board:

   ```shell
   cd examples/use_rust/rp2040
   cargo run
   ```

4. (Optional) Flash using USB

   If you don't have a debug probe, you can use `elf2uf2-rs` to flash your firmware via USB. There are several additional steps you have to do:

   1. Install `elf2uf2-rs`: `cargo install elf2uf2-rs`
   2. Update `examples/use_rust/rp2040/.cargo/config.toml`, use `elf2uf2` as the flashing tool
      ```diff
      - runner = "probe-rs run --chip RP2040"
      + runner = "elf2uf2-rs -d"
      ```
   3. Connect your rp2040 board holding the BOOTSEL key, ensure that rp's USB drive appears
   4. Flash
      ```shell
      cd examples/use_rust/rp2040
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

## [Roadmap](https://haobogu.github.io/rmk/roadmap.html)

Current roadmap of RMK can be found [here](https://haobogu.github.io/rmk/roadmap.html).

## Minimum Supported Rust Version (MSRV)

This crate requires stable Rust 1.75 and up. 

## License

RMK is licensed under either of

- Apache License, Version 2.0 (LICENSE-APACHE or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license (LICENSE-MIT or <http://opensource.org/licenses/MIT>)

at your option.
