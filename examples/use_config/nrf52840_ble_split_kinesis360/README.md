# Kinesis Advantage 360 Pro - RMK Split BLE Example

RMK firmware for the [Kinesis Advantage 360 Pro](https://kinesis-ergo.com/keyboards/advantage360/), a wireless split ergonomic keyboard with nRF52840 MCUs and Adafruit nRF52 bootloader.

## Hardware

- **MCU**: nRF52840 (both halves)
- **Bootloader**: Adafruit nRF52 (UF2-compatible)
- **Connection**: BLE split (central = left, peripheral = right)
- **Matrix**: 5 rows x 20 columns (10 per half)
- **LEDs**: WS2812 RGB via P0.13 power MOSFET (MOSFET enabled, RGB control not yet supported in RMK TOML config)

## Prerequisites

- Rust nightly with `thumbv7em-none-eabihf` target
- `cargo-make`: `cargo install cargo-make`
- `flip-link`: `cargo install flip-link`
- `cargo-binutils`: `cargo install cargo-binutils`
- `cargo-hex-to-uf2`: `cargo install cargo-hex-to-uf2`
- LLVM tools: `rustup component add llvm-tools`
- For build.rs: `BINDGEN_EXTRA_CLANG_ARGS` may need to point to your ARM GCC sysroot, e.g.:
  ```shell
  export BINDGEN_EXTRA_CLANG_ARGS="--sysroot=/usr/lib/arm-none-eabi"
  ```

## Build

```shell
cd examples/use_config/nrf52840_ble_split_kinesis360
cargo make uf2
```

This produces two UF2 files:
- `kinesis360-central.uf2` (left half)
- `kinesis360-peripheral.uf2` (right half)

## Flash

1. **Flash the right half (peripheral) first**:
   - Connect the right half via USB
   - Double-tap the reset button to enter bootloader mode (a USB drive named `KINESIS360` or `NRF52BOOT` appears)
   - Copy `kinesis360-peripheral.uf2` to the drive
   - The board will reboot automatically

2. **Flash the left half (central)**:
   - Connect the left half via USB
   - Double-tap the reset button to enter bootloader mode
   - Copy `kinesis360-central.uf2` to the drive
   - The board will reboot automatically

3. **Remove USB cables** — RMK switches to USB mode when a cable is connected. Disconnect both halves after flashing so BLE split communication activates.

## Storage Overlap Warning

**This is critical.** The Kinesis Adv360 Pro firmware (central binary) is approximately 426KB. RMK's default nRF BLE storage address is `0x60000` (384KB), which means the firmware code overlaps with the storage region. On first boot, the storage initialization erases flash pages that contain running code, causing an instant crash loop.

This example sets `start_addr = 0xEC000` in `keyboard.toml` to place storage safely past the firmware end and below the bootloader at `0xF4000`. **Do not remove this setting.**

If your firmware grows, verify the binary size stays below the storage address:
```shell
cargo size --release --bin central
```

## Important: No `async_matrix`

This example does **not** use the `async_matrix` feature. The Kinesis Adv360 Pro's matrix pin configuration is incompatible with `async_matrix` on nRF52840 (GPIOTE channel conflicts). Use the default polling matrix instead.

## BLE Pairing

The keymap includes a BLE control layer (Layer 3, activated by holding `MO(3)` in the top-right position):

| Key | Function |
|-----|----------|
| Layer 3 + `1` | BLE profile 0 |
| Layer 3 + `2` | BLE profile 1 |
| Layer 3 + `3` | BLE profile 2 |
| Layer 3 + `4` | Next BLE profile |
| Layer 3 + `5` | Previous BLE profile |
| Layer 3 + `LGui` | Clear current BLE bond |

## Recovery: Switching Back to ZMK/Clique

If you need to return to the stock ZMK or Clique firmware:

1. Enter bootloader mode (double-tap reset)
2. Flash the Kinesis `settings-reset` UF2 to clear nRF storage
3. Flash the SoftDevice s140 UF2 (required for ZMK, not present after RMK)
4. Flash the ZMK/Clique firmware UF2
5. Repeat for both halves

The `settings-reset` step is important because RMK uses different storage addresses than ZMK, so leftover data can cause pairing issues.
