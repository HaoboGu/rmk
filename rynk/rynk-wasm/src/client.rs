//! Wasm-facing Rynk client handle.
//!
//! JS owns the byte link and hands it to [`connect`], which runs the Rynk
//! handshake and returns a [`RynkClient`] wrapping a `Client<WasmTransport>`.
//! Each method borrows the client for one await — the same way the native
//! serial/BLE transports drive `Client<T>` directly, so JS must await one call
//! before issuing the next. Topic pushes are pulled with
//! [`RynkClient::next_event`], not delivered by callback.

use rynk::rmk_types::action::{EncoderAction, KeyAction};
use rynk::rmk_types::battery::BatteryStatus;
use rynk::rmk_types::ble::BleStatus;
use rynk::rmk_types::combo::Combo;
use rynk::rmk_types::connection::{ConnectionStatus, ConnectionType};
use rynk::rmk_types::fork::Fork;
use rynk::rmk_types::led_indicator::LedIndicator;
use rynk::rmk_types::morse::Morse;
use rynk::rmk_types::protocol::rynk::{
    BehaviorConfig, DeviceCapabilities, GetComboBulkResponse, GetKeymapBulkResponse, GetMorseBulkResponse, MacroData,
    MatrixState, PeripheralStatus, ProtocolVersion, SetComboBulkRequest, SetKeymapBulkRequest, SetMorseBulkRequest,
    StorageResetMode,
};
use rynk::{Client, IncomingTopic, RynkDevice, TopicEvent};
use wasm_bindgen::prelude::*;

use crate::device::WebDevice;
use crate::transport::{JsByteLink, WasmTransport};

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
    let client = WebDevice::new(link, label).connect().await?;
    Ok(RynkClient(client))
}

#[wasm_bindgen]
impl RynkClient {
    /// The display name the page supplied at connect time (WebHID `productName`,
    /// a page-derived string, or the default when none was given).
    pub fn label(&self) -> String {
        self.0.transport().label().to_string()
    }

    /// Pull the next recognized topic push (server→host). Parks until one
    /// arrives; rejects with `Disconnected` at EOF. Unrecognized topics are
    /// skipped. JS drives this in a loop, like the native `next_event()` pull.
    pub async fn next_event(&mut self) -> Result<TopicEvent, JsValue> {
        loop {
            match self.0.next_event().await {
                Ok(IncomingTopic::Topic(t)) => return Ok(t),
                // No JS shape for an unrecognized topic; wait for the next one.
                Ok(IncomingTopic::Unknown(_)) => continue,
                Err(e) => return Err(e.into()),
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

/// Generate the typed wasm request methods from the native client shape. Each row
/// is `name(scalar args ; body: BodyTy) -> RespTy`; bodies and responses are tsify
/// wire types, so wasm-bindgen marshals them across the ABI and emits a precise
/// `.d.ts` (no `JsValue`/`any`). Errors convert to a JS `Error` via `RynkHostError: Into<JsValue>`.
macro_rules! endpoints {
    ($( $name:ident ( $($s:ident : $sty:ty),* $(; $j:ident : $jty:ty)? ) -> $rty:ty ),* $(,)?) => {
        #[wasm_bindgen]
        impl RynkClient {
            $(
                pub async fn $name(&mut self, $($s: $sty,)* $($j: $jty)?) -> Result<$rty, JsValue> {
                    self.0.$name($($s,)* $($j)?).await.map_err(Into::into)
                }
            )*
        }
    };
}

endpoints! {
    // system
    get_version() -> ProtocolVersion,
    get_capabilities() -> DeviceCapabilities,
    reboot() -> (),
    bootloader_jump() -> (),
    storage_reset(; mode: StorageResetMode) -> (),
    // keymap
    get_key(layer: u8, row: u8, col: u8) -> KeyAction,
    set_key(layer: u8, row: u8, col: u8; action: KeyAction) -> (),
    get_default_layer() -> u8,
    set_default_layer(layer: u8) -> (),
    get_encoder(encoder_id: u8, layer: u8) -> EncoderAction,
    set_encoder(encoder_id: u8, layer: u8; action: EncoderAction) -> (),
    get_keymap_bulk(layer: u8, start_row: u8, start_col: u8, count: u8) -> GetKeymapBulkResponse,
    set_keymap_bulk(; request: SetKeymapBulkRequest) -> (),
    // combos / forks / morse / macros
    get_combo(index: u8) -> Combo,
    set_combo(index: u8; config: Combo) -> (),
    get_combo_bulk(start_index: u8, count: u8) -> GetComboBulkResponse,
    set_combo_bulk(; request: SetComboBulkRequest) -> (),
    get_fork(index: u8) -> Fork,
    set_fork(index: u8; config: Fork) -> (),
    get_morse(index: u8) -> Morse,
    set_morse(index: u8; config: Morse) -> (),
    get_morse_bulk(start_index: u8, count: u8) -> GetMorseBulkResponse,
    set_morse_bulk(; request: SetMorseBulkRequest) -> (),
    get_macro(index: u8, offset: u16) -> MacroData,
    set_macro(index: u8, offset: u16; data: MacroData) -> (),
    // behavior
    get_behavior() -> BehaviorConfig,
    set_behavior(; config: BehaviorConfig) -> (),
    // status
    get_current_layer() -> u8,
    get_matrix_state() -> MatrixState,
    get_battery_status() -> BatteryStatus,
    get_led_indicator() -> LedIndicator,
    get_peripheral_status(slot: u8) -> PeripheralStatus,
    get_wpm() -> u16,
    get_sleep_state() -> bool,
    // connection
    get_connection_type() -> ConnectionType,
    get_connection_status() -> ConnectionStatus,
    get_ble_status() -> BleStatus,
    switch_ble_profile(slot: u8) -> (),
    clear_ble_profile(slot: u8) -> (),
}
