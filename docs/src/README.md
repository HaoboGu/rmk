# RMK

[![Crates.io](https://img.shields.io/crates/v/rmk)](https://crates.io/crates/rmk)
[![Docs](https://img.shields.io/docsrs/rmk)](https://docs.rs/rmk/latest/rmk/)
[![Build](https://github.com/haobogu/rmk/actions/workflows/build.yml/badge.svg)](https://github.com/HaoboGu/rmk/actions)

RMK is a Rust keyboard firmware crate with lots of usable features, like layer support, dynamic keymap, vial support, BLE wireless, etc, makes firmware customization easy and accessible.


The following table compares features available from RMK, Keyberon, QMK and ZMK:

|                                     | RMK         | Keyberon         | QMK                      | ZMK                        |
| ----------------------------------- | ----------- | ---------------- | ------------------------ | -------------------------- |
| Language                            | Rust        | Rust             | C                        | C                          |
| USB support                         | ✅           | ✅                | ✅                        | ✅                          |
| BLE support                         | ✅           |                  |                          | ✅                          |
| Via/Vial support                    | ✅           |                  | ✅                        |                            |
| Split support                       | ✅           | ✅                | ✅                        | ✅                          |
| Wireless split support              | ✅           |                  |                          | ✅                          |
| ARM chips(STM32/nRF/RP2040) support | ✅           | ✅                | ✅                        | ✅                          |
| RISC-V & Xtensa chips support       | ✅           |                  |                          |                            |
| Mouse key                           | ✅           |                  | ✅                        |                            |
| Keyboard configuration              | toml (easy) | Rust code (hard) | json + makefile (medium) | Kconfig + devicetree(hard) |
| Layers/Macros/Media key support     | ✅           | ✅                | ✅                        | ✅                          |


