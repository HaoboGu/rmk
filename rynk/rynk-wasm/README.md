# rynk-wasm

`rynk-wasm` is the browser-facing Rynk host client. It compiles the typed
`rynk::Client` protocol layer to wasm and exposes a `RynkClient` API to
JavaScript.

The browser page owns browser transports such as Web Serial and WebHID. The wasm
package owns the Rynk protocol state machine.

```text
Web Serial / WebHID / another browser transport
        -> JsByteLink { send, recv, close }
        -> transport::WasmTransport
        -> rynk::Client
        -> RynkClient methods exposed to JavaScript
```

This split is intentional: browser permissions, chooser UI, stream locks, and
hot-plug events stay in JS, while request/response typing, topic handling, and
protocol validation stay in Rust.

## Prerequisites

- A Chromium browser such as Chrome or Edge. Web Serial and WebHID are not
  available in Firefox or Safari.
- Wasm target: `rustup target add wasm32-unknown-unknown`
- Packager: `cargo install wasm-pack`

## Build & Serve

```bash
cd rynk/rynk-wasm
wasm-pack build --target web        # emits ./pkg/
python3 -m http.server 8000         # localhost is a secure context for Web Serial / WebHID
```

Open Chrome or Edge at `http://localhost:8000` and use `index.html` as the
reference shell.

## Minimal Usage

Import the generated wasm package, create a JS byte link, connect it, then call
typed client methods.

```js
import init, { connect } from "./pkg/rynk_wasm.js";

await init();

const link = await openSerialByteLink();
const client = await connect(link);

console.log("protocol", client.protocol_version());
console.log("capabilities", client.capabilities());
console.log("current layer", await client.get_current_layer());

// Pull topic pushes (layer changes, WPM, …) until the link closes.
(async () => {
  try { for (;;) console.log("topic", await client.next_event()); }
  catch (e) { console.log("disconnected:", e.message); }
})();

// Disconnect by closing the byte link; the topic loop above then ends.
await link.close();
```

The object passed to `connect(link)` only needs this shape:

```js
{
  async send(bytes) {
    // Uint8Array from wasm -> browser transport
  },
  async recv() {
    // Browser transport -> Uint8Array for wasm.
    // Return an empty Uint8Array only when the link is closed.
  },
  async close() {
    // Release browser resources. Safe to call more than once.
  },
}
```

`index.html` includes complete Web Serial and WebHID implementations. It also
shows the optional version-probe flow used before dynamically loading a protocol
major-specific wasm package.

## JsByteLink Contract

`JsByteLink` is a byte-stream boundary, not a high-level Rynk API.

- `send(bytes)` receives bytes from Rust and must deliver them in order. It should
  resolve only after the browser transport has accepted the bytes.
- `recv()` must wait until bytes are available or the link is closed. It may
  return any non-empty chunk size; it does not need to return exactly one Rynk
  frame.
- `recv()` returns `new Uint8Array(0)` only for EOF. That becomes
  `Disconnected` in the wasm API.
- `close()` should release locks, close devices, and wake any pending `recv()`.
  It should be idempotent.
- Only `rynk-wasm` should call `recv()` after `connect()`. If your page needs to
  probe the protocol version first, do it before calling `connect()`.
- Transport-specific framing must be hidden below this boundary. For example,
  WebHID report padding must be stripped so wasm sees the same clean Rynk byte
  stream that Web Serial exposes.

## Minimal Web Serial Link

This is the smallest useful shape. The demo has a more complete buffered version
that supports the pre-load version probe.

```js
async function openSerialByteLink() {
  const port = await navigator.serial.requestPort();
  await port.open({ baudRate: 115200 });

  const reader = port.readable.getReader();
  const writer = port.writable.getWriter();
  let closed = false;

  return {
    async send(bytes) {
      await writer.write(bytes);
    },

    async recv() {
      if (closed) return new Uint8Array(0);
      for (;;) {
        const { value, done } = await reader.read();
        if (done) {
          closed = true;
          return new Uint8Array(0);
        }
        if (value && value.length) return value;
      }
    },

    async close() {
      closed = true;
      try { await reader.cancel(); } catch {}
      try { reader.releaseLock(); } catch {}
      try { await writer.close(); } catch {}
      try { writer.releaseLock(); } catch {}
      try { await port.close(); } catch {}
    },
  };
}
```

Call `requestPort()` inside a user gesture such as a button click.

## RynkClient API

`connect()` performs the Rynk handshake and returns a live `RynkClient` that
owns the `rynk::Client` protocol state machine directly. Each method borrows it
for one await, so await one call before issuing the next — the same single-borrow
rule the native serial/BLE consumers get from the compiler.

Topic pushes are pulled, not delivered by callback: drive `next_event()` in a
loop. It parks until the next recognized topic and rejects with `Disconnected`
at EOF, mirroring the native `Client::next_event()` used by `rynk-serial` /
`rynk-ble`. `resync()` and `events_dropped()` round out the housekeeping surface.

Available method groups mirror the native `rynk::Client` API:

- System: `get_version`, `get_capabilities`, `reboot`, `bootloader_jump`,
  `storage_reset`
- Keymap: `get_key`, `set_key`, `get_default_layer`, `set_default_layer`,
  `get_encoder`, `set_encoder`, `get_keymap_bulk`, `set_keymap_bulk`
- Combos/forks/morse/macros: `get_combo`, `set_combo`, `get_combo_bulk`,
  `set_combo_bulk`, `get_fork`, `set_fork`, `get_morse`, `set_morse`,
  `get_morse_bulk`, `set_morse_bulk`, `get_macro`, `set_macro`
- Behavior/status/connection: `get_behavior`, `set_behavior`,
  `get_current_layer`, `get_matrix_state`, `get_battery_status`,
  `get_peripheral_status`, `get_wpm`, `get_sleep_state`,
  `get_led_indicator`, `get_connection_type`, `get_connection_status`,
  `get_ble_status`, `switch_ble_profile`, `clear_ble_profile`

Getter results and topic values are plain JS values produced through
`serde-wasm-bindgen`. Setter payloads are plain JS objects matching the Rust
serde shape for the corresponding `rmk-types` type.

Errors are thrown as JS `Error` objects with stable `name` values such as
`Disconnected`, `Transport`, `Rejected`, `Unsupported`, `Protocol`, and
`VersionMismatch`.

## Browser Transports

Both built-in demo links present the same `JsByteLink` shape to wasm. Only the
JS code that opens and normalizes the browser transport differs.

- **USB - Web Serial.** The page opens the keyboard's serial port and streams raw
  Rynk frame bytes over it.
- **BLE - WebHID over the OS HID link.** A pure browser cannot reach Rynk's
  custom 128-bit GATT service on an OS-bonded keyboard, and Web Bluetooth cannot
  attach a bonded keyboard at all. WebHID can reach the firmware's vendor HID
  report (`RynkHidService`, usage page `0xFF60`) via the existing OS HID link, so
  there is no pairing prompt. The page fragments each Rynk frame into fixed
  32-byte reports and reassembles reports back into the clean byte stream before
  wasm sees them.

Rynk's custom-GATT BLE transport (`rynk-ble`, native `bluest`) is a separate
native-only path, for example:

```bash
cargo run -p rynk --example hw_test -- ble
```

## Versioned Loading

The Rynk frame envelope and `GetVersion` request are intended to stay stable
across protocol majors. `index.html` uses that to probe the device first, then
loads the wasm package for the reported major:

```js
const { major, minor } = await link.probeVersion();
const core = await loadCore(major);
await core.default();
const client = await core.connect(link);
```

Today there is one wasm package. When protocol v2 lands, `loadCore(major)` can
select a second wasm build while keeping the same JS byte-link implementations.

## Troubleshooting

- **`navigator.serial` / `navigator.hid` is undefined**: use a Chromium browser
  over `http://localhost:8000`. Firefox and Safari do not expose these APIs.
- **Serial port chooser empty / device absent**: the OS may already hold the
  port. Close native `hw_test`, serial monitors, Arduino IDE, and other tabs. On
  Linux, make sure the user is in the `dialout` group.
- **WebHID chooser empty**: the firmware must expose `RynkHidService` with usage
  page `0xFF60`; a build without that HID report will not appear.
- **`port closed` / `device closed` / open failure**: the link is busy or lacks
  permission; another tab or app may hold it.
- **`bad version reply`**: the page opened the wrong serial port or HID
  collection. Pick the keyboard's own Rynk serial port or HID device.
