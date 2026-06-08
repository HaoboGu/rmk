# rynk-web-demo

Minimal browser harness for the **TS-shell** deployment of `rmk-host`. wasm-only;
built with `wasm-pack`.

The page is the shell: it owns the WebSerial port, runs the permanent
`get_version` probe itself (the Rynk frame envelope never changes across protocol
versions), then **dynamically loads the version-matched `rynk-core` wasm** and
lets it run the typed session over a JS bridge (`rmk_host_bridge::BridgeTransport`
↔ a `{ send, recv }` object the page provides). The wasm core never touches the
port directly.

**BLE configuration is native-only** (the `rmk-host-ble` transport) — use the
`rmk-host` CLI for BLE.

## Prerequisites

- A **Chromium** browser (Chrome / Edge / Opera). Web Serial is not in
  Firefox/Safari.
- Tooling: `rustup target add wasm32-unknown-unknown` and `cargo install wasm-pack`.

## Build & serve

```bash
cd rmk-host-tool/rynk-web-demo
wasm-pack build --target web        # emits ./pkg/
python3 -m http.server 8000         # localhost is a secure context for Web Serial
# open Chrome at http://localhost:8000, click "Connect via Serial", pick the port
```

On connect the page logs the probed `protocol vX.Y`, then the wasm core's
status/config sweep (also mirrored to the DevTools console).

## How it maps to the architecture

- **Page (TS shell)** — WebSerial port + permanent `get_version` probe + dynamic
  wasm load. The transport (`bluest` / `tokio_serial` on native) is WebSerial
  here, owned by JS.
- **`rynk_web_demo.wasm`** — the `rynk-core` artifact: today's `Client` plus
  `rmk-host-bridge`, exposing `run(link)` via `#[wasm_bindgen]`. Its `Transport`
  is `BridgeTransport`, forwarding bytes to the page's `{ send, recv }`.

When protocol v2 lands, the page's `loadCore(major)` switch gains a case and a
second `rynk_core_v2.wasm` is built — the probe already tells it which to load.

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
