# rmk-host-tool

Host-side library + CLI for the **RMK protocol** — the postcard-rpc-based
configuration protocol that replaces Vial.

This is a **standalone Cargo workspace** (no virtual workspace with the
firmware). The firmware tree is `no_std` and embedded-targeted; this tree is
`std` and depends on `tokio` / `nusb`, so they're kept separate.

## Crates

- **`rmk-host`** — async client library wrapping `postcard_rpc::HostClient`.
  Centralizes the protocol handshake (`GetVersion` → version check →
  `GetCapabilities`) in `Client::connect_usb`, then exposes typed
  per-domain wrappers.
- **`rmk-cli`** — `clap` CLI binary. Subcommands: `info`, `dump-keymap`,
  `set-key`, `bootloader`, `reset`, `monitor layers`. **No `lock` /
  `unlock`** — the firmware-side lock gate is deferred to v2 (plan §3.7),
  so a CLI surface for it would silently no-op.

## Building

```bash
cd rmk-host-tool
cargo build --release
```

The CLI binary lands at `target/release/rmk-cli`.

## Running

```bash
# Defaults to VID:PID 0xc0de:0xcafe — RMK's reserved test pair.
./target/release/rmk-cli info
./target/release/rmk-cli dump-keymap
./target/release/rmk-cli --vid 0x303a --pid 0x4001 monitor layers
```

## OS-specific setup

### Linux

The CLI talks to the keyboard's USB vendor-class interface (`bInterfaceClass = 0xFF`).
Add a udev rule so it's accessible without `sudo`:

```text
# /etc/udev/rules.d/99-rmk.rules
SUBSYSTEM=="usb", ATTR{idVendor}=="c0de", ATTR{idProduct}=="cafe", MODE="0660", TAG+="uaccess"
```

Then `sudo udevadm control --reload && sudo udevadm trigger`.

### Windows

The firmware ships WinUSB MSOS 2.0 descriptors with the RMK protocol's
DeviceInterfaceGUID `{C8B9F0E2-9D4A-4B4C-AAFB-1C3F2D10A8E5}` (must match
`rmk/src/usb/mod.rs`). On first plug-in Windows binds the WinUSB driver
automatically — no installer required.

### macOS

No driver setup; macOS ships a generic IOUSBHost driver that exposes vendor
bulk interfaces directly.

## Versioning

The CLI bails on protocol `major` mismatch and refuses unsupported `minor`
versions. Both bounds live in `rmk-host/src/lib.rs::SUPPORTED_MAJOR` /
`SUPPORTED_MINOR` — bump them together with the firmware's
`ProtocolVersion::CURRENT`.
