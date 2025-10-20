# Roadmap

There are a bunch of things to do with RMK in the near future. I plan to ship 1.0.0 after all the following items are accomplished.

## Roadmap to 1.0.0

| Mark | Description |
| ---- | ----------- |
| 🔴   | important   |
| 🟢   | easy        |
| 🔵   | heavy work  |

#### keyboard feature

- [x] Layer support
- [x] System/media/mouse keys
- [x] LED
- [x] Tap/hold
- [x] Keyboard macros
- [x] Async key detection and report sending
- [x] 🔵 Split keyboard support
- [x] Direct pin
- [x] 🔵 Input device
  - [x] 🟢 Encoder
  - [ ] 🔴 Mouse
  - [ ] 🔴 Trackball
  - [ ] 🔴 Trackpad
- [x] 🔴 Basic Macro support
  - [x] 🔴 Macro definition via Rust
  - [x] 🔴 Unicode and umlaute support
- [ ] Macro support enhancement
  - [ ] 🟢 Macro definition via toml
  - [x] 🟢 Make macro storage space configurable
- [ ] 🔴 Tap dance
- [ ] 🔵 Controller device
  - [ ] External power control(GPIO)
  - [ ] 🔴 RGB
  - [ ] 🔴 Display support

#### Wireless

- [x] BLE support - nRF
- [x] Auto switch between BLE/USB
- [x] Battery service from ADC
- [x] 🔴 BLE support - esp32c3 and esp32s3
- [x] Sleep mode to save battery
- [x] 🔵 Universal BLE wrapper, including BLE management, battery management, supports both nRF and ESP
- [x] Stablizing BLE feature gate/API
- [x] Support more MCUs, such as cyw(used in rp2040w/rp2350w)
- [ ] Deep-sleep mode
- [ ] Better support for dongle
- [ ] Nordic Uart HCI support
- [ ] Send report via Uart/Usart
- [ ] Switch between dongle mode and BLE mode

#### User experience

- [x] vial support
- [x] Easy keyboard configuration with good default, support different MCUs
- [x] CLI and GUI tool for project generation, firmware compilation, etc
- [x] Versioned documentation site, better documentation
- [ ] Making vial and default keymap consistent automatically
- [ ] 🔴🔵 GUI keymap configurator which supports windows/macos/linux/web
- [ ] Default bootloader
- [ ] USB DFU/OTA

If you want to contribute, please feel free to open an issue or PR, or just ping me! Any forms of contribution are welcome :D
