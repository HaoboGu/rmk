# nRF54LM20A BLE example

This example targets the nRF54LM20A DK and uses the local `embassy`, `nrf-sdc`, and `trouble`
trees from the sibling `../../../../../rust/` directory for nRF54 support.

It uses the DK's four onboard buttons as a 2x2 direct-pin keyboard matrix:

- Button 1 (`P1.26`) -> row 0 / col 0
- Button 2 (`P1.09`) -> row 0 / col 1
- Button 3 (`P1.08`) -> row 1 / col 0
- Button 4 (`P0.05`) -> row 1 / col 1

Storage is backed by the internal flash via `nrf-mpsl`, so keymap/profile changes persist.

## Running

1. Enter the example directory:

   ```shell
   cd examples/use_rust/nrf54lm20_ble
   ```

2. Build, flash, and run it:

   ```shell
   cargo run --release
   ```

If `probe-rs` fails to flash because of the current nRF54LM20A bug, run the same command again.
It can take a second or third attempt.

When the DK is attached to a USB host, RMK will prefer USB mode by default until you switch the
saved connection mode. If you want to test BLE first, power it without enumerating USB or toggle to
BLE mode from RMK once the board is running.
