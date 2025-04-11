# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.1] - 2025-04-11

### Added

- Internal pull-up resistor config for rotary encoders

### Changed

- Default polling interval of battery ADC

### Fixed

- Wrong GPIO pull for stm32 

## [0.5.0] - 2025-04-06

### Added

- `SHIFTED` key
- Transperant key
- Rotary encoder config
- Joystick config
- Fork behavior config
- Add keycode alias

### Changed

- Change `UP` key to `Up`
- Deny unknown fields by default

### Fixed

- Wrong config for esp32 + direct pin
- Compilation error when the pin list is empty

## [0.4.2] - 2025-01-22

### Changed

- Update embassy dependencies to latest

## [0.4.1] - 2025-01-02

### Added

- Add `MT` and `TH` in layout section

### Fixed

- Fix wrong default `TapHoldConfig` settings
- Fix Chip/board checking error in rmk-macro
- Fix L/Rgui typo in macro -> L/RGui

## [0.4.0] - 2024-12-16

### Added

- Add `split.central.matrix` and `split.peripheral.matrix` for split matrix configurations
- Add `clear_storage` option

### Removed

- BREAKING: Removed `split.central.input_pins` and `split.central.output_pins`
- BREAKING: Removed `split.peripheral.input_pins` and `split.peripheral.output_pins`

## [0.3.4] - 2024-11-27

### Changed

- Remove `rmk-config`

## [0.3.3] - 2024-11-27

### Fixed

- Fix link error on Windows

## [0.3.2] - 2024-11-25

### Added

- Add behavior config
- Add tri-layer and one-shot config

## [0.3.1] - 2024-11-13

### Added

- Add voltage divider from toml
- Add TO(n) and DF(n) support
- Add DirectPinMatrix, Including entry, gpio config, matrix config, board type.
- Support no_pin for `DirectPinMatrix`.

## [0.3.0] - 2024-10-28

### Added

- Add default config for chips
- Implemented `keyboard.toml` parsing for the new `WM(key, modifier)` "With Modifier" macro

### Changed

- BREAKING: refactor the whole macro crate, update `keyboard.toml` fields
- Use reference of keymap in `run_rmk`

### Fixed

- Add pull to pins
- Fix reversed input_pins and output_pins

## [0.2.1] - 2024-10-03

### Changed

- Use 0x60000 as the default start addr for nRF52

## [0.2.0] - 2024-09-11

### Added

- Add support for split keyboard(serial/BLE)
- Add support for overriding chip initialization for nrf/rp/esp32

### Changed

- BREAKING: generate new public APIs

### Fixed

- Fix broken link in error message

## [0.1.8] - 2024-08-06

- Update `embassy-nrf` and `embassy-rp` version

## [0.1.7] - 2024-07-25

- Fix keymap doesn't update issue [#41](https://github.com/HaoboGu/rmk/issues/41)

## [0.1.6] - 2024-06-14

### Added

- Support more nRF chips: nRF52833, nRF52810, nRF52811
- Set default GPIO level according to `keybaord.toml`

## [0.1.5] - 2024-06-08

### Added

- Recognize `async_matrix` feature defined in `Cargo.toml` and automatically generate corresponding code.

## [0.1.4] - 2024-06-06

### Added

- Parse keymap from `keyboard.toml`, see https://haobogu.github.io/rmk/keyboard_configuration.html#keymap-config

## [0.1.2] - 2024-06-01

### Added

- Add new CHANGELOG.md
