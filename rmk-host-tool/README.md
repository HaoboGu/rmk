# `rmk-host-tool`

Minimal host-side USB tool for the RMK protocol.

## Commands

- `list` — enumerate matching USB devices and interfaces
- `handshake` — connect over raw USB bulk, send `GetVersion`, and attempt `GetCapabilities`

## Examples

```bash
cargo run --manifest-path rmk-host-tool/Cargo.toml -- list
cargo run --manifest-path rmk-host-tool/Cargo.toml -- handshake --vid 0x4C4B --pid 0x4643
cargo run --manifest-path rmk-host-tool/Cargo.toml -- handshake --serial ABC123
```

## Notes

- The current firmware Phase 3 implementation guarantees `GetVersion`.
- `GetCapabilities` is attempted too, but current firmware may still reply with `WireError::UnknownKey` until Phase 4 lands.
- On Windows, `nusb` may not expose interface metadata for all composite-driver combinations; if needed, pass `--interface-number` explicitly.
