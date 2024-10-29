# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0] - 2024-10-28

### Added

- Restart function of ESP32
- Methods for optimizing nRF BLE power consumption, now the idle current is decreased to about 20uA
- Multi-device support for nRF BLE
- New `wm` "With Modifier" macro to support basic keycodes with modifiers active
- Voltage divider to estimate battery voltage
- Per chip/board default settings
- i18n support of documentation
- Use flip-link as default linker

### Changed

- BREAKING: use reference of keymap in `run_rmk` 
- BREAKING: refactor the whole macro crate, update `keyboard.toml` config, old `keyboard.toml` config may raise compilation error
- Decouple the matrix(input device) and keyboard implementation
- Stop scanning matrix after releasing all keys
- Move creation of Debouncer and Matrix to `run_rmk_*` function from `initialize_*_and_run`

### Fixed

- Unexpected power consumption for nRF
- Extra memory usage by duplicating keymaps
- A COL/ROW typo
- Stackoverflow of some ESP32 chips by increasing default ESP main stack size

## [0.3.2] - 2024-10-05

### Fixed

- Fix vial not work for nRF

## [0.3.1] - 2024-10-03

### Added

- Automate uf2 firmware generation via `cargo-make`
- Storage and vial support for ESP series
- Vial over BLE support for Windows 
- `TO` and `DF` action support

### Changed

- Update `bitfield-struct` to v0.9
- Update `esp32-nimble` to v0.8, as well as used `ESP_IDF_VERSION` to v5.2.3
- Use 0x60000 as the default start addr for nRF52

### Fixed

- Fix no device detected on vial desktop

## [0.3.0] - 2024-09-11

### Changed

- BREAKING: all public keyboard APIs are merged into `run_rmk` and `run_rmk_with_async_flash`. Compared with many different APIs for different chips before, the new API is more self-consistent. Different arguments are enabled by feature gates.

### Added

- Basic split keyboard support via serial and BLE
- ESP32C6 support
- Reboot mechanism for cortex-m chips

## [0.2.4] - 2024-08-06

### Added

- `MatrixTrait` which is used in keyboard instead of a plain `Matrix` struct

### Changed

- Update versions of dependecies

## [0.2.3] - 2024-07-25

### Fixed

- Fix keymap doesn't change issue
- Fix with_modifier action doesn't trigger the key with modifier
- Fix capital letter is not send in keyboard macro

### Changed

- Yield everytime after sending a keyboard report to channel
- Update `sequential-storage` to v3.0.0
- Update `usbd-hid` to v0.7.1

## [0.2.2] - 2024-07-12

- Add keyboard macro support
- Support vial keymap reset command
- Fix default `lt!` and `lm!` implementation

## [0.2.1] - 2024-06-14

### Fixed

- Fix USB not responding when the light service is not enabled

## [0.2.0] - 2024-06-14

### Added

- Support led status update from ble 
- Support more nRF chips: nRF52833, nRF52810, nRF52811

## [0.1.21] - 2024-06-08

### Added

- Add `async_matrix` feature, which enables async detection of key press and reduces power consumption

## [0.1.20] - 2024-06-06

### Added

- Support read default keymap from `keyboard.toml`, see https://haobogu.github.io/rmk/keyboard_configuration.html#keymap-config

## [0.1.17] - 2024-06-04

### Fixed

- Fixed doc display error on docs.rs 

## [0.1.16] - 2024-06-01

### Added

- Add new CHANGELOG.md


