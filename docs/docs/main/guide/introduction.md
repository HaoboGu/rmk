# RMK introduction

[![Crates.io](https://img.shields.io/crates/v/rmk)](https://crates.io/crates/rmk)

[![Docs](https://img.shields.io/docsrs/rmk)](https://docs.rs/rmk/latest/rmk/)

[![Build](https://github.com/haobogu/rmk/actions/workflows/build.yml/badge.svg)](https://github.com/HaoboGu/rmk/actions)


RMK is a Rust keyboard firmware framework featuring comprehensive functionality including layer support, dynamic keymap editing, Vial integration, BLE wireless connectivity, and more. It makes keyboard firmware customization easy and accessible.

The following table compares features available from RMK, Keyberon, QMK and ZMK:

|  | RMK | Keyberon | QMK | ZMK |
| --- | --- | --- | --- | --- |
| Language | Rust | Rust | C | C |
| USB keyboard support | ✅ | ✅ | ✅ | ✅ |
| BLE keyboard support | ✅ |  |  | ✅ |
| Real-time keymap editing | ✅ |  | ✅ | 🚧 |
| Wired split support | ✅ | ✅ | ✅ |  |
| Wireless split support | ✅ |  |  | ✅ |
| ARM chips(STM32/nRF/RP2040) support | ✅ | ✅ | ✅ | ✅ |
| Full RISC-V & Xtensa chips support | ✅ |  |  |  |
| Keyboard configuration | toml (easy) | Rust code (hard) | json + makefile (medium) | Kconfig + devicetree(hard) |
| Layers/Macros/Media key support | ✅ | ✅ | ✅ | ✅ |
| [Morse key](./features/configuration/behavior.md#morse-and-tap-dance) support | ✅ |  |  |  |