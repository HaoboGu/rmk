<!-- PROJECT LOGO -->
<br />
<p align="center">
  <a href="https://github.com/haobogu/rmk">
    <img src="https://github.com/HaoboGu/rmk/blob/2b73951291fd829075277d64e0aeb8ea82d0bea9/docs/src/images/rmk_logo2.png?raw=true" alt="Logo" width="150">
  </a>

  <p align="center">
  A feature-rich keyboard firmware written in Rust.
  <br />
  <br />
  <a href="https://crates.io/crates/rmk"><img src="https://img.shields.io/crates/v/rmk"></a>
  <a href="https://docs.rs/rmk/latest/rmk/"><img src="https://img.shields.io/docsrs/rmk"></a>
  <a href="https://github.com/HaoboGu/rmk/actions"><img src="https://github.com/haobogu/rmk/actions/workflows/build.yml/badge.svg"></a>
  <a href="https://discord.gg/HHGA7pQxkG"><img src="https://img.shields.io/discord/1166665039793639424?label=discord"></a>
  </p>
</p>

👉 Join our [Discord server](https://discord.gg/HHGA7pQxkG) for discussions, support, and community collaboration!

----- 
[中文](https://github.com/HaoboGu/rmk/blob/main/README_zh.md)


## Features

- **Broad microcontroller compatibility**: Leveraging [embassy](https://github.com/embassy-rs/embassy), RMK supports a comprehensive range of microcontrollers, including stm32, nRF, rp2040(w), esp32, etc
- **Dynamic keymap customization**: RMK offers native [Vial](https://get.vial.today) support, enabling real-time keymap modifications. You can even edit keymaps over BLE connections wirelessly
- **Advanced keyboard functionality**: RMK comes with lots of advanced keyboard features by default, including layer switching, media controls, system commands, mouse control, and more
- **Wireless connectivity**: BLE wireless support with automatic reconnection and multi-device capabilities for nRF52 and esp32 microcontrollers, tested on nRF52840, esp32c3, esp32s3
- **Easy configuration**: RMK simplifies keyboard development through a single `keyboard.toml` configuration file. For Rust enthusiasts, the firmware remains highly customizable using Rust code
- **Optimized performance**: RMK achieves approximately 2ms latency in wired mode and 10ms in wireless mode. With the `async_matrix` feature enabled, power consumption is significantly reduced—a 2000mAh battery can power your keyboard for several months

## [User Documentation](https://haobogu.github.io/rmk/user_guide/1_guide_overview.html) | [API Reference](https://docs.rs/rmk/latest/rmk/) | [FAQs](https://haobogu.github.io/rmk/faq.html) | [Changelog](https://github.com/HaoboGu/rmk/blob/main/rmk/CHANGELOG.md)

## Real-World Implementations

### [rmk-ble-keyboard](https://github.com/HaoboGu/rmk-ble-keyboard)

<img src="https://github.com/HaoboGu/rmk/blob/main/docs/src/images/rmk_ble_keyboard.jpg?raw=true" width="60%">

### [dactyl-lynx-rmk](https://github.com/whitelynx/dactyl-lynx-rmk)

<img src="https://raw.githubusercontent.com/whitelynx/dactyl-lynx-keyboard/refs/heads/main/resources/skeleton-prototype.jpg" width="60%">

## Getting Started

### Option 1: Start with a Template

Quickly bootstrap your project using [rmkit](https://github.com/HaoboGu/rmkit) and the official RMK [project template](https://github.com/HaoboGu/rmk-template/tree/feat/rework).

```shell
cargo install rmkit flip-link
# If you encounter installation issues on Windows, try this alternative command:
# powershell -ExecutionPolicy ByPass -c "irm https://github.com/haobogu/rmkit/releases/download/v0.0.9/rmkit-installer.ps1 | iex"
rmkit init
```

For comprehensive guidance, refer to the [User Guide](https://haobogu.github.io/rmk/user_guide/1_guide_overview.html).

### Option 2: Explore Built-in Examples

Browse the examples in the [`examples`](https://github.com/HaoboGu/rmk/blob/main/examples) directory. Below are step-by-step instructions for rp2040 development. The process is similar for other microcontrollers when using a debug probe.

#### rp2040 Setup

1. Install [probe-rs](https://github.com/probe-rs/probe-rs)

   ```shell
   curl --proto '=https' --tlsv1.2 -LsSf https://github.com/probe-rs/probe-rs/releases/latest/download/probe-rs-tools-installer.sh | sh
   ```

2. Build the firmware

   ```shell
   cd examples/use_rust/rp2040
   cargo build --release
   ```

3. Flash using a debug probe

   With a debug probe connected to your rp2040 board, simply run:

   ```shell
   cd examples/use_rust/rp2040
   cargo run --release
   ```

4. (Optional) Flash via USB

   Without a debug probe, you can use `elf2uf2-rs` to flash via USB:

   1. Install the tool: `cargo install elf2uf2-rs`
   2. Modify `examples/use_rust/rp2040/.cargo/config.toml` to use `elf2uf2`:
      ```diff
      - runner = "probe-rs run --chip RP2040"
      + runner = "elf2uf2-rs -d"
      ```
   3. Connect your rp2040 board while holding the BOOTSEL button until the USB drive appears
   4. Flash the firmware:
      ```shell
      cd examples/use_rust/rp2040
      cargo run --release
      ```
      Upon successful completion, you'll see output similar to:
      ```shell
      Finished release [optimized + debuginfo] target(s) in 0.21s
      Running `elf2uf2-rs -d 'target\thumbv6m-none-eabi\release\rmk-rp2040'`
      Found pico uf2 disk G:\
      Transfering program to pico
      173.00 KB / 173.00 KB [=======================] 100.00 % 193.64 KB/s  
      ```

## [Development Roadmap](https://haobogu.github.io/rmk/roadmap.html)

Current roadmap of RMK can be found [here](https://haobogu.github.io/rmk/roadmap.html).

## Minimum Supported Rust Version (MSRV)

RMK is developed against the latest stable Rust release. While other versions may work, they are not fully tested.

## License

RMK is licensed under either of

- Apache License, Version 2.0 (LICENSE-APACHE or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license (LICENSE-MIT or <http://opensource.org/licenses/MIT>)

at your option.
