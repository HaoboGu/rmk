# Roadmap

There are a bunch of things to do with RMK in the near future. I plan to ship 1.0.0 after all the following items are accomplished.

## Roadmap to 1.0.0

| Mark | Description |
| ---- | ----------- |
|  🔴  | important   |
|  🟢  | easy        |
|  🔵  | massive work|


#### keyboard feature
  - [x] layer support
  - [x] system/media/mouse keys
  - [x] LED
  - [ ] full async key detection and report sending
  - [ ] 🔴 RGB
  - [ ] keyboard macros
  - [ ] 🟢 encoder
  - [ ] 🔵 display support
  - [ ] tap/hold
  - [ ] 🔵 split keyboard support

#### Wireless
  - [x] BLE support - nRF
  - [x] auto switch between BLE/USB
  - [x] battery service from ADC
  - [x] 🔴 BLE support - esp32c3
  - [ ] 🔴 BLE support - all esp32 chips
  - [ ] 🔵 universal BLE wrapper, including BLE management, battery management, supports both nRF and ESP at least
  - [ ] stablizing BLE feature gate/API
  - [ ] sleep mode to save battery
  - [ ] BLE support - ch58x/ch59x

#### User experience
  - [x] vial support
  - [ ] 🔴🔵 GUI configurator which supports windos/macos/linux/web
  - [ ] default bootloader
  - [ ] USB DFU/OTA
  - [ ] 🔴 easy keyboard configuration with good default, support different MCUs
  - [ ] making vial and default keymap consistent automatically
  - [ ] good documentation

If you want to contribute, please feel free to open an issue or PR, or just ping me! Any forms of contribution are welcome :D

