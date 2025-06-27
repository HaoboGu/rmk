# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Updated

- Only the valid macro data is stored in the storage now. **Clearing storage is required to update**

### Fixed

- Light service is wrongly disabled

## [0.7.3] - 2025-06-18

### Added

- [Logging via USB](https://rmk.rs/docs/features/usb_logging.html)
- Events for controllers

### Changed

- Update to TrouBLE v0.2.0

### Fixed

- Fix sdc build error
- Fix cloud build script for ESP32

## [0.7.2] - 2025-06-12

### Fixed

- Fix ADC initialization for splits
- Fix NonusHash parsing error
- Fix wrong state after switching output
- Fix py32 example

### Added

- Use 2M Phy by default

### Changed

- Move `Controller` behind a feature flag 


## [0.7.1] - 2025-06-04

### Fixed

- Fix the error when using matrix_map

## [0.7.0] - 2025-06-04

### Changed

- **BREAKING**: The BLE stack is migrated to [TrouBLE](https://github.com/embassy-rs/trouble/)
- **BREAKING**: Add `rmk-config` and use `[env]` in `.cargo/config.toml` to configure the path of `keyboard.toml`
- Optimize the size of buffer used in USB 
- A new documentation site is released! Check out [rmk.rs](https://rmk.rs)

### Added

- BLE and wireless split support for Pi Pico W, check out [this example](https://github.com/HaoboGu/rmk/tree/main/examples/use_config/pi_pico_w_ble)
- Introduce matrix_map for [nicer keyboard matrix configs](https://rmk.rs/docs/features/configuration/layout.html)
- BLE + USB dual-mode support for esp32s3
- Automatically pair between central and peripheral
- Make constants in RMK [configurable via `keyboard.toml`](https://rmk.rs/docs/features/configuration/rmk_config.html)
- Enable [support for keyboard macros](https://rmk.rs/docs/features/keymap/keyboard_macros.html) (via rust based configuration only for now) (closes issues #308, #284, #303, #313, #170)
- Battery charging state reader
- Sleep timeout when advertising
- Allow disabling the storage feature in `Cargo.toml` to work with `keyboard.toml`

### Fixed

- Wrong connection state between splits
- Issue about first adc polling
- Wrong battery status
- Capslock stuck on macOS
- Wrong BLE address setting in `keyboard.toml`

## [0.6.1] - 2025-04-11

### Added

- Repeat key support
- Basic GraveEscape support
- Internal pull-up config for encoders

### Fixed

- Wrong GPIO pulls for stm32
- Combo cannot be triggered correctly when there's overlap between combos 
- Battery level led indicator failure

## [0.6.0] - 2025-04-06

### Added

- Input device support
- Rotary encoder and joystick are supported
- State fork behavior
- Bootloader jumping for nRF52 and RP2040
- Artificial pull up resistor to pio tx line
- Shifted key and transparent key support in toml config
- Clear the storage by checking build hash after flashing a new firmware
- stm32g4 example without storage feature

### Changed

- Make `storage` a feature, enabled by default
- Documentation improvement
- Remove unnecessary pio-proc dependency
- Improve modifier reporting

### Fixed

- Wrong ESP32 BLE serial number
- Wrong col/row when using direct pin
- Error when there's empty IO pin list in the config
- Many other minor fixes

## [0.5.2] - 2025-01-22

### Added

- `defmt` feature gate
- rp2350 example
- Added `_matrix` functions to allow passing custom matrix implementation

### Changed

- Make more modules public
- Update embassy dependencies to latest
- Improve robustness of serial communication between splits

### Fixed

- Record positions of triggered keys, fix key stuck 
- Remove invalid PHY type setting between splits
- Receive keys from peripheral when there's no connection
- Always sync the connection state to fix the unexpected lost of peripherals
- Fix link scripts which are broken after flip-link updated
- Remove `block_on` to prevent unexpected hang on the periphrals

## [0.5.1] - 2025-01-02

### Added

- Add new [cloud-based project template](https://github.com/haobogu/rmk-project-template)
- Connection state sync between central and peripheral
- Use 2M PHY by default
- `mt!` for modifier tap-hold and `th!` for tap-hold action

### Changed

- Use [`rmkit`](https://github.com/HaoboGu/rmkit) as the default project generator
- Update `sequential-storage` to v4.0.0
- Improve CI speed
- Use `cargo-hex-to-uf2` instead of python script for uf2 firmware generation

### Fixed

- Fix hold-after-tap key loss by allowing multi hold-after-tap keys
- Exchange left and right modifier
- Fix wrong number of waited futs of direct-pin matrix
- Fix wrong peripheral conn param by setting conn param from central, not peripheral

## [0.5.0] - 2024-12-16

### Added

- BREAKING: Support `direct_pin` type matrix for split configurations, split pin config is moved to [split.central/peripheral.matrix]
- Support home row mod(HRM) mode with improved tap hold processing
- Add `clear_storage` option
- Enable USB remote wakeup
- py32f07x use_rust example. py32f07x is a super cheap($0.2) cortex-m0 chip from Puya

### Changed

- Remove `rmk-config`

### Fixed

- Fix slightly lag on peripheral side
- Fix invalid BLE state after reconnection on Windows
- Fix ghosting key on macOS
- Fix direct pin debouncer size error
- Fix esp32 input pin pull configuration
- Fix BLE peripheral lag

## [0.4.4] - 2024-11-27

### Fixed

- Fix link error on Windows

## [0.4.3] - 2024-11-25

### Added

- One-shot layer/modifier support
- Tri-layer support

### Fixed

- Fix connection error when there're multiple peripherals
- Fix keycode converter error

## [0.4.2] - 2024-11-13

### Added

- Layout macro to! and df!
- One-shot layer and one-shot modifier
- Make nRF52840 voltage divider configurable
- ch32v307 example

### Changed

- Use `User11` to manually switch between USB mode and BLE mode

### Fixed

- Fix nRF52840 linker scripts for nice!nano
- Fix broken documentation links

## [0.4.1] - 2024-10-31

### Fixed

- Fix lagging for split peripheral

### Added

- Direct pin mod. Including `DirectPinMatrix`, `run_rmk_direct_pin` functions etc.
- Added pin active level parameter `low_active` to direct pin.
- Support no_pin for `DirectPinMatrix`.

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
