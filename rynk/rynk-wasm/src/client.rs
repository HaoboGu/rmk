//! Wasm-facing Rynk client handle.
//!
//! JS owns the byte link and hands it to [`connect`], which runs the Rynk
//! handshake and returns a [`RynkClient`] wrapping a `Client<WasmTransport>`.
//! Each method borrows the client for one await — the same way the native
//! serial/BLE transports drive `Client<T>` directly, so JS must await one call
//! before issuing the next. Topic pushes are pulled with
//! [`RynkClient::next_event`], not delivered by callback.

use rynk::rmk_types::action::{EncoderAction, KeyAction};
use rynk::rmk_types::combo::Combo;
use rynk::rmk_types::fork::Fork;
use rynk::rmk_types::morse::Morse;
use rynk::rmk_types::protocol::rynk::{
    BehaviorConfig, MacroData, SetComboBulkRequest, SetKeymapBulkRequest, SetMorseBulkRequest, StorageResetMode,
};
use rynk::{Client, ConnectError, IncomingTopic, RequestError, RynkDevice, TransportError};
use serde::Serialize;
use serde::de::DeserializeOwned;
use wasm_bindgen::prelude::*;

use crate::device::WebDevice;
use crate::transport::{JsByteLink, WasmTransport};

// Error/value marshaling.

/// Build a JS Error with a stable name.
fn js_err(kind: &str, message: &str) -> JsValue {
    let e = js_sys::Error::new(message);
    e.set_name(kind);
    e.into()
}

/// Map EOF to `Disconnected` and other transport errors to `Transport`.
fn transport_err(e: &TransportError) -> JsValue {
    let kind = match e {
        TransportError::Disconnected => "Disconnected",
        _ => "Transport",
    };
    js_err(kind, &e.to_string())
}

/// Preserve the main native request error categories for JS callers.
fn request_err(e: &RequestError) -> JsValue {
    match e {
        RequestError::Transport(t) => transport_err(t),
        RequestError::Rejected(_) => js_err("Rejected", &e.to_string()),
        RequestError::Unsupported(..) => js_err("Unsupported", &e.to_string()),
        _ => js_err("Protocol", &e.to_string()),
    }
}

fn connect_err(e: &ConnectError) -> JsValue {
    match e {
        ConnectError::Transport(t) => transport_err(t),
        ConnectError::Request(r) => request_err(r),
        ConnectError::VersionMismatch { .. } => js_err("VersionMismatch", &e.to_string()),
    }
}

fn parse<T: DeserializeOwned>(value: JsValue) -> Result<T, JsValue> {
    serde_wasm_bindgen::from_value(value).map_err(|e| js_err("Deserialize", &e.to_string()))
}

/// Encode a request result as a JS value.
fn encode<T: Serialize>(r: Result<T, RequestError>) -> Result<JsValue, JsValue> {
    match r {
        Ok(v) => serde_wasm_bindgen::to_value(&v).map_err(|e| js_err("Serialize", &e.to_string())),
        Err(e) => Err(request_err(&e)),
    }
}

/// Live Rynk client handle exposed to JavaScript.
///
/// Wraps a `Client<WasmTransport>`; methods borrow it for one await, so JS must
/// await each call before the next (the single-borrow rule the native
/// transports get from the compiler). Dropping the handle, or closing the JS
/// link, ends the session.
#[wasm_bindgen]
pub struct RynkClient(Client<WasmTransport>);

/// Handshake over an already-open JS link and return a client. Routes through
/// [`WebDevice`] — the web transport's [`RynkDevice`] — so the browser path uses
/// the same connect lifecycle as the native serial/BLE transports. `label` is the
/// display name the page showed in its picker (WebHID `productName`, or a derived
/// string); omit it or pass `null` for a default.
#[wasm_bindgen]
pub async fn connect(link: JsByteLink, label: Option<String>) -> Result<RynkClient, JsValue> {
    let client = WebDevice::new(link, label).connect().await.map_err(|e| connect_err(&e))?;
    Ok(RynkClient(client))
}

#[wasm_bindgen]
impl RynkClient {
    /// The display name the page supplied at connect time (WebHID `productName`,
    /// a page-derived string, or the default when none was given).
    pub fn label(&self) -> String {
        self.0.transport().label().to_string()
    }

    /// Device capabilities from the connect handshake — local read, no wire traffic.
    pub fn capabilities(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(self.0.capabilities()).map_err(|e| js_err("Serialize", &e.to_string()))
    }

    /// Protocol version from the connect handshake — local read, no wire traffic.
    pub fn protocol_version(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.0.protocol_version()).map_err(|e| js_err("Serialize", &e.to_string()))
    }

    /// Pull the next recognized topic push (server→host). Parks until one
    /// arrives; rejects with `Disconnected` at EOF. Unrecognized topics are
    /// skipped. JS drives this in a loop, like the native `next_event()` pull.
    pub async fn next_event(&mut self) -> Result<JsValue, JsValue> {
        loop {
            match self.0.next_event().await {
                Ok(IncomingTopic::Topic(t)) => {
                    return serde_wasm_bindgen::to_value(&t).map_err(|e| js_err("Serialize", &e.to_string()));
                }
                // No JS shape for an unrecognized topic; wait for the next one.
                Ok(IncomingTopic::Unknown(_)) => continue,
                Err(e) => return Err(transport_err(&e)),
            }
        }
    }

    /// Drop a stalled partial frame so the next request starts clean.
    pub fn resync(&mut self) {
        self.0.resync();
    }

    /// Topic pushes the driver dropped (queue full). `f64` so JS gets a `number`.
    pub fn events_dropped(&self) -> f64 {
        self.0.events_dropped() as f64
    }
}

/// Generate wasm request methods from the native client shape.
macro_rules! endpoints {
    ($( $name:ident ( $($s:ident : $sty:ty),* $(; $j:ident : $jty:ty)? ) ),* $(,)?) => {
        #[wasm_bindgen]
        impl RynkClient {
            $(
                pub async fn $name(&mut self, $($s: $sty,)* $($j: JsValue)?) -> Result<JsValue, JsValue> {
                    $( let $j: $jty = parse($j)?; )?
                    encode(self.0.$name($($s,)* $($j)?).await)
                }
            )*
        }
    };
}

endpoints! {
    // system
    get_version(),
    get_capabilities(),
    reboot(),
    bootloader_jump(),
    storage_reset(; mode: StorageResetMode),
    // keymap
    get_key(layer: u8, row: u8, col: u8),
    set_key(layer: u8, row: u8, col: u8; action: KeyAction),
    get_default_layer(),
    set_default_layer(layer: u8),
    get_encoder(encoder_id: u8, layer: u8),
    set_encoder(encoder_id: u8, layer: u8; action: EncoderAction),
    get_keymap_bulk(layer: u8, start_row: u8, start_col: u8, count: u8),
    set_keymap_bulk(; request: SetKeymapBulkRequest),
    // combos / forks / morse / macros
    get_combo(index: u8),
    set_combo(index: u8; config: Combo),
    get_combo_bulk(start_index: u8, count: u8),
    set_combo_bulk(; request: SetComboBulkRequest),
    get_fork(index: u8),
    set_fork(index: u8; config: Fork),
    get_morse(index: u8),
    set_morse(index: u8; config: Morse),
    get_morse_bulk(start_index: u8, count: u8),
    set_morse_bulk(; request: SetMorseBulkRequest),
    get_macro(index: u8, offset: u16),
    set_macro(index: u8, offset: u16; data: MacroData),
    // behavior
    get_behavior(),
    set_behavior(; config: BehaviorConfig),
    // status
    get_current_layer(),
    get_matrix_state(),
    get_battery_status(),
    get_peripheral_status(slot: u8),
    get_wpm(),
    get_sleep_state(),
    get_led_indicator(),
    // connection
    get_connection_type(),
    get_connection_status(),
    get_ble_status(),
    switch_ble_profile(slot: u8),
    clear_ble_profile(slot: u8),
}
