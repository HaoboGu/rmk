# Agar Mini BLE RMK example

This example migrates the ZMK Agar Mini BLE configuration to RMK.

Hardware covered:

- nRF52840 BLE + USB firmware
- 4x12 matrix scanned through a 74HC595 column shifter
- Vial dynamic keymap support
- Battery reporting through SAADC AIN3/P0.05 with a 2M/820K divider
- ZMK-compatible active-low GPIO RGB indicator on P1.11/P1.10/P0.03
- WS2812 is wired on P0.06, matching the source overlay, but is not used as the status indicator

Build the UF2 firmware with:

```shell
cargo make uf2
```

The output is `agar-mini-ble.uf2`.
