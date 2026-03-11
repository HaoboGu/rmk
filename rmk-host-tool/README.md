# `rmk-host-tool`

Minimal host-side USB tool for the RMK protocol.

## Commands

- `list` — enumerate matching USB devices and interfaces
- `ping` — send postcard-rpc standard `PingEndpoint`
- `schema` — dump the postcard-rpc standard schema report
- `handshake` — connect over raw USB bulk, send `GetVersion` and `GetCapabilities`
- `get-key` — get a single key action from the keymap
- `set-key` — set a single key action (simple HID keycode only)
- `dump-keymap` — dump the entire keymap from the device
- `get-lock-status` — query the device lock state
- `get-default-layer` — get the current default layer
- `set-default-layer` — set the default layer
- `reboot` — reboot the device
- `reset-keymap` — reset keymap to defaults (erases layout storage and reboots)
- `storage-reset` — reset storage (full or layout-only erase and reboot)

## Examples

```bash
cargo run --manifest-path rmk-host-tool/Cargo.toml -- list
cargo run --manifest-path rmk-host-tool/Cargo.toml -- ping --vid 0x4C4B --pid 0x4643 --value 123
cargo run --manifest-path rmk-host-tool/Cargo.toml -- schema --vid 0x4C4B --pid 0x4643
cargo run --manifest-path rmk-host-tool/Cargo.toml -- handshake --vid 0x4C4B --pid 0x4643
cargo run --manifest-path rmk-host-tool/Cargo.toml -- get-key --vid 0x4C4B --pid 0x4643 --layer 0 --row 0 --col 0
cargo run --manifest-path rmk-host-tool/Cargo.toml -- dump-keymap --vid 0x4C4B --pid 0x4643
```

## Notes

- On Windows, `nusb` may not expose interface metadata for all composite-driver combinations; if needed, pass `--interface-number` explicitly.
