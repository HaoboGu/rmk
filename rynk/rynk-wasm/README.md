# rynk-wasm

`rynk`'s host support compiled to a **wasm package** — the `Client` plus the
in-crate JS-bridge transport (`mod bridge`) — together with a minimal browser
example that drives it. wasm-only; built with `wasm-pack`.

The page is the example shell: it owns the WebSerial port, runs the permanent
`get_version` probe itself (the Rynk frame envelope never changes across protocol
versions), then **dynamically loads the version-matched `rynk-wasm` package** and
drives the typed session through its flat `connect(link)` + per-command API over a
JS bridge (`bridge::BridgeTransport` ↔ a `{ send, recv }` object the page
provides). The `Client` lives Rust-side and never crosses into JS.

**BLE configuration is native-only** (the `rynk-ble` transport) — for BLE, use a
native build, e.g. `cargo run -p rynk --example hw_test -- ble`.

## Prerequisites

- A **Chromium** browser (Chrome / Edge / Opera). Web Serial is not in
  Firefox/Safari.
- Tooling: `rustup target add wasm32-unknown-unknown` and `cargo install wasm-pack`.

## Build & serve

```bash
cd rynk/rynk-wasm
wasm-pack build --target web        # emits ./pkg/
python3 -m http.server 8000         # localhost is a secure context for Web Serial
# open Chrome at http://localhost:8000, click "Connect via Serial", pick the port
```

On connect the page logs the probed `protocol vX.Y`, then the wasm package's
status/config sweep (also mirrored to the DevTools console).

## How it maps to the architecture

- **Page (the example shell)** — owns the WebSerial port, runs the permanent
  `get_version` probe, and dynamically loads the wasm. The transport that is
  `tokio-serial` / `bluest` on native is WebSerial here, owned by the page.
- **`rynk_wasm.wasm`** — the wasm package: today's `Client` plus the in-crate JS
  bridge, exposing `connect(link)` and the per-command session functions via
  `#[wasm_bindgen]` (see `mod session`). Its byte link is `BridgeTransport`,
  forwarding bytes to the page's `{ send, recv }`.

When protocol v2 lands, the page's `loadCore(major)` switch gains a case for a
second wasm build — the probe already tells it which to load.

## Troubleshooting

- **`navigator.serial is undefined`** → not a secure context / wrong browser.
  Use `http://localhost:8000` in Chrome/Edge.
- **Port chooser empty / device absent** → the OS may already hold the port;
  close anything using it (native `hw_test`, a serial monitor, Arduino IDE) and
  retry.
- **`port closed` / open failure** → port busy or permissions. On Linux, be in
  the `dialout` group.
- **`bad version reply`** → opened the wrong serial port (a debug-probe VCOM,
  etc.); pick the keyboard's port.
