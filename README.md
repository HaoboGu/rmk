<!-- PROJECT LOGO -->
<br />
<p align="center">
  <a href="https://github.com/haobogu/rmk">
    <img src="https://github.com/HaoboGu/rmk/blob/2b73951291fd829075277d64e0aeb8ea82d0bea9/docs/src/images/rmk_logo2.png?raw=true" alt="Logo" width="150">
  </a>

  <p align="center">
  A feature-rich Rust keyboard firmware. 
  <br />
  <br />
  <a href="https://crates.io/crates/rmk"><img src="https://img.shields.io/crates/v/rmk"></a>
  <a href="https://docs.rs/rmk/latest/rmk/"><img src="https://img.shields.io/docsrs/rmk"></a>
  <a href="https://github.com/HaoboGu/rmk/actions"><img src="https://github.com/haobogu/rmk/actions/workflows/build.yml/badge.svg"></a>
  <a href="https://discord.gg/HHGA7pQxkG"><img src="https://img.shields.io/discord/1166665039793639424?label=discord"></a>
  </p>
</p>

👉 Join our [Discord server](https://discord.gg/HHGA7pQxkG) if you have anything to discuss!

----- 
[中文](https://github.com/HaoboGu/rmk/blob/main/README_zh.md)


## Features

- **Support a wide range of microcontrollers**: Powered by [embassy](https://github.com/embassy-rs/embassy), RMK supports a wide range of microcontrollers, such as stm32/nRF/rp2040/esp32
- **Real-time keymap editing**: RMK has built-in [Vial](https://get.vial.today) support, the keymap can be changed on-the-fly. You can even use Vial to edit keymap over BLE directly
- **Advanced keyboard features**: Many advanced keyboard features are available by default in RMK, such as layer switch, media control, system control, mouse control, etc
- **Wireless**: BLE wireless support with auto-reconnection/multiple devices feature for nRF52 and esp32 microcontrollers, tested on nRF52840, esp32c3 and esp32s3
- **Easy configuration**: RMK provides a simple way to build your keyboard: a `keyboard.toml` is all you need! For experienced Rust user, you can still customize your firmware easily using RMK
- **Low latency and low-power ready**: RMK has a typical 2 ms latency in wired mode and 10 ms latency in wireless mode. By enabling `async_matrix` feature, RMK has very low power consumption, with a 2000mah battery, RMK can provide several months battery life

## [User Documentation](https://haobogu.github.io/rmk/user_guide/1_guide_overview.html) | [API Reference](https://docs.rs/rmk/latest/rmk/) | [FAQs](https://haobogu.github.io/rmk/faq.html) | [Changelog](https://github.com/HaoboGu/rmk/blob/main/rmk/CHANGELOG.md)

## Real World Examples

### [rmk-ble-keyboard](https://github.com/HaoboGu/rmk-ble-keyboard)

<img src="https://github.com/HaoboGu/rmk/blob/main/docs/src/images/rmk_ble_keyboard.jpg?raw=true" width="60%">

## Usage

### Option 1: Initialize from template
You can use [rmk-template](https://github.com/HaoboGu/rmk-template) to initialize your project.

```shell
cargo install cargo-generate flip-link
cargo generate --git https://github.com/HaoboGu/rmk-template
```

Then follow the steps in generated `README.md`. Check RMK's [User Guide](https://haobogu.github.io/rmk/user_guide/1_guide_overview.html) for details.

### Option 2: Try built-in examples

Example can be found at [`examples`](https://github.com/HaoboGu/rmk/blob/main/examples). The following is a simple step-to-step instruction for rp2040. For other microcontrollers, the steps should be identical with a debug probe.

#### rp2040

1. Install [probe-rs](https://github.com/probe-rs/probe-rs)

   ```shell
   curl --proto '=https' --tlsv1.2 -LsSf https://github.com/probe-rs/probe-rs/releases/latest/download/probe-rs-tools-installer.sh | sh
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

   If you don't have a debug probe, you can use `elf2uf2-rs` to flash your rp2040 firmware via USB. There are several additional steps you have to do:

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

This crate uses latest stable. Other versions should work, but they're not tested.

## License

RMK is licensed under either of

- Apache License, Version 2.0 (LICENSE-APACHE or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license (LICENSE-MIT or <http://opensource.org/licenses/MIT>)

at your option.
