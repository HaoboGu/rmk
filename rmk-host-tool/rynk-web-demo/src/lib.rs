//! The version-matched `rynk-core` wasm artifact, driven by a TS shell.
//!
//! In this deployment the **TS layer owns the device**: it opens the WebSerial
//! port, runs the permanent `get_version` probe, then dynamically loads this
//! wasm. This core never touches the port directly — it talks through a
//! [`JsLink`] the shell provides (a `{ send, recv }` object over the same
//! port), via [`BridgeTransport`].
//!
//! BLE configuration stays native-only (the `rmk-host-ble` transport).
//!
//! The crate is gated on `wasm32` — on any other target it compiles to an empty
//! library. Build it with `wasm-pack build --target web` and drive it from
//! `index.html`.
#![cfg(target_arch = "wasm32")]

use core::fmt::Debug;

use rmk_host::{Client, ConnectError, RequestError, Transport};
use rmk_host_bridge::{BridgeTransport, JsLink};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Debug);
}

/// Run the command sweep over a TS-provided link. The shell has already opened
/// the port and probed the version; `link` is a `{ send, recv }` object over
/// that port. Returns a formatted, one-line-per-command summary; rejects with
/// the error string if the handshake fails.
#[wasm_bindgen]
pub async fn run(link: JsLink) -> Result<String, JsValue> {
    report(Client::connect(BridgeTransport::new(link)).await).await
}

/// Read every (capability-gated) Rynk status/config command into a
/// one-line-per-command summary.
async fn report<T: Transport>(connected: Result<Client<T>, ConnectError>) -> Result<String, JsValue> {
    let mut client = match connected {
        Ok(c) => c,
        Err(e) => return Err(JsValue::from_str(&format!("handshake failed: {e:?}"))),
    };

    let caps = *client.capabilities();
    let mut out = format!(
        "✓ connected — {}×{}×{} keymap, {} combos, {} forks, {} morse, {} macros, ble={}\n\n",
        caps.num_layers,
        caps.num_rows,
        caps.num_cols,
        caps.max_combos,
        caps.max_forks,
        caps.max_morse,
        caps.max_macros,
        caps.ble_enabled,
    );

    out += &line("version", &client.get_version().await);
    out += &line("default layer", &client.get_default_layer().await);
    out += &line("current layer", &client.get_current_layer().await);
    out += &line("key L0(0,0)", &client.get_key(0, 0, 0).await);
    out += &line("key L0(0,1)", &client.get_key(0, 0, 1).await);
    out += &line("behavior", &client.get_behavior().await);
    if caps.max_combos > 0 {
        out += &line("combo 0", &client.get_combo(0).await);
    }
    if caps.max_forks > 0 {
        out += &line("fork 0", &client.get_fork(0).await);
    }
    if caps.max_morse > 0 {
        out += &line("morse 0", &client.get_morse(0).await);
    }
    out += &line("wpm", &client.get_wpm().await);
    out += &line("sleep state", &client.get_sleep_state().await);
    out += &line("led indicator", &client.get_led_indicator().await);
    out += &line("connection type", &client.get_connection_type().await);
    if caps.ble_enabled {
        out += &line("battery", &client.get_battery_status().await);
        out += &line("ble status", &client.get_ble_status().await);
    }

    Ok(out)
}

/// Format one command result as a status line: `✓` value, `⚠` device rejected,
/// `✗` transport error.
fn line<T: Debug>(label: &str, r: &Result<T, RequestError>) -> String {
    match r {
        Ok(v) => format!("✓ {label:<16} {v:?}\n"),
        Err(RequestError::Rejected(e)) => format!("⚠ {label:<16} rejected: {e:?}\n"),
        Err(e) => format!("✗ {label:<16} {e}\n"),
    }
}
