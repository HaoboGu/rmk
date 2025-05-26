# Roadmap

There are a bunch of things to do with RMK in the near future. I plan to ship 1.0.0 after all the following items are accomplished.

## Roadmap to 1.0.0

| Mark | Description |
| ---- | ----------- |
| 🔴   | important   |
| 🟢   | easy        |
| 🔵   | heavy work  |

#### keyboard feature

- [x] layer support
- [x] system/media/mouse keys
- [x] LED
- [x] tap/hold
- [x] keyboard macros
- [x] async key detection and report sending
- [x] 🔵 split keyboard support
- [x] Direct pin
- [x] 🟢 encoder
- [x] 🔵 Input device
- [x] 🔴 Basic Macro support
  - [x] 🔴 Macro definition via Rust
  - [x] add KeyAction::TriggerMacro so more than 32 macros can be triggered
  - [x] 🔴 Support umlaute
  - [x] 🔵 Support unicode
- [ ] Macro support enhancement
  - [ ] 🟢 Macro definition via toml
  - [ ] 🟢 make macro storage space configurable
- [ ] 🔴 RGB
- [ ] 🔵 display support

#### Wireless

- [x] BLE support - nRF
- [x] auto switch between BLE/USB
- [x] battery service from ADC
- [x] 🔴 BLE support - esp32c3 and esp32s3
- [x] sleep mode to save battery
- [x] 🔵 universal BLE wrapper, including BLE management, battery management, supports both nRF and ESP
- [x] stablizing BLE feature gate/API
- [ ] support more MCUs, such as cyw(used in rp2040w/rp2350w)

#### User experience

- [x] vial support
- [x] easy keyboard configuration with good default, support different MCUs
- [x] CLI and GUI tool for project generation, firmware compilation, etc
- [ ] Versioned documentation site, better documentation
- [ ] making vial and default keymap consistent automatically
- [ ] 🔴🔵 GUI keymap configurator which supports windows/macos/linux/web
- [ ] default bootloader
- [ ] USB DFU/OTA

If you want to contribute, please feel free to open an issue or PR, or just ping me! Any forms of contribution are welcome :D
