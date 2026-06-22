//! Interactive session API: the `rynk` [`Client`] driven from Rust, exposed to JS
//! as flat async operations mirroring the native client surface, plus a live topic
//! stream. The `Client` never crosses the wasm boundary.
//!
//! A single background actor (spawned by [`connect`]) owns the `Client` and, like
//! the native `select!` app loop, multiplexes two things over the one byte link:
//!
//! - **topic pushes** — read continuously and handed to the JS `on_topic` callback;
//! - **request jobs** — each JS call ([`get_key`], …) ships an async op over an
//!   mpsc channel and awaits its reply over a oneshot.
//!
//! When a job arrives, the actor drops the in-flight topic read (cancel-safe — the
//! transport must not lose delivered bytes), runs the job, then resumes reading.
//! Requests are therefore serialized through the actor; topics arriving during a
//! request are queued by the driver and emitted once it returns. Typed values cross
//! as native JS objects via serde-wasm-bindgen (the rmk-types derive serde).

use core::cell::RefCell;
use core::future::Future;
use core::ops::AsyncFnOnce;
use core::pin::Pin;

use futures_channel::mpsc::{self, UnboundedReceiver, UnboundedSender};
use futures_channel::oneshot;
use futures_util::{FutureExt, StreamExt, pin_mut, select_biased};
use rynk::rmk_types::action::{EncoderAction, KeyAction};
use rynk::rmk_types::combo::Combo;
use rynk::rmk_types::fork::Fork;
use rynk::rmk_types::morse::Morse;
use rynk::rmk_types::protocol::rynk::{
    BehaviorConfig, MacroData, SetComboBulkRequest, SetKeymapBulkRequest, SetMorseBulkRequest, StorageResetMode,
};
use rynk::{Client, ConnectError, IncomingTopic, RequestError, TransportError};
use serde::Serialize;
use serde::de::DeserializeOwned;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::bridge::{BridgeTransport, JsLink};

type JobResult = Result<JsValue, JsValue>;
/// A request job: runs against the actor's `Client`, borrowing it for one await.
type Job = Box<dyn for<'a> FnOnce(&'a mut Client<BridgeTransport>) -> Pin<Box<dyn Future<Output = JobResult> + 'a>>>;

/// The live session for this tab. Holds the actor's job sender plus the snapshot
/// read at connect time (so the sync accessors need no round trip).
struct Session {
    jobs: UnboundedSender<(Job, oneshot::Sender<JobResult>)>,
    caps: JsValue,
    version: JsValue,
}

thread_local! {
    static SESSION: RefCell<Option<Session>> = const { RefCell::new(None) };
}

// ── error / value marshaling ──

/// A JS `Error` whose `name` is the kind (so JS can branch on it) and `message`
/// the detail.
fn js_err(kind: &str, message: &str) -> JsValue {
    let e = js_sys::Error::new(message);
    e.set_name(kind);
    e.into()
}

fn transport_err(e: &TransportError) -> JsValue {
    let kind = match e {
        TransportError::Disconnected => "Disconnected",
        _ => "Transport",
    };
    js_err(kind, &e.to_string())
}

/// Map a request error to a JS error, preserving the native distinction between a
/// firmware rejection, a locally-gated unsupported command, and a dead link.
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

/// Deserialize a JS value argument into a typed request value.
fn parse<T: DeserializeOwned>(value: JsValue) -> Result<T, JsValue> {
    serde_wasm_bindgen::from_value(value).map_err(|e| js_err("Deserialize", &e.to_string()))
}

/// Encode a request result as a JS value. A void reply serializes to `undefined`.
fn encode<T: Serialize>(r: Result<T, RequestError>) -> JobResult {
    match r {
        Ok(v) => serde_wasm_bindgen::to_value(&v).map_err(|e| js_err("Serialize", &e.to_string())),
        Err(e) => Err(request_err(&e)),
    }
}

/// Run one client op on the actor: box it into a job, ship it over the channel, and
/// await + encode the reply. The single point where a request crosses into the
/// `Client`-owning task; every `#[wasm_bindgen]` method below is a one-line wrapper.
async fn run<T, Op>(op: Op) -> JobResult
where
    T: Serialize + 'static,
    Op: AsyncFnOnce(&mut Client<BridgeTransport>) -> Result<T, RequestError> + 'static,
{
    let jobs = SESSION
        .with(|s| s.borrow().as_ref().map(|x| x.jobs.clone()))
        .ok_or_else(|| js_err("NoSession", "no active session"))?;
    let (reply_tx, reply_rx) = oneshot::channel();
    let job: Job = Box::new(move |c| Box::pin(op(c).map(encode)));
    jobs.unbounded_send((job, reply_tx))
        .map_err(|_| js_err("Disconnected", "session ended"))?;
    // A cancelled reply means the actor dropped the job — i.e. the link died.
    reply_rx
        .await
        .unwrap_or_else(|_| Err(js_err("Disconnected", "session ended")))
}

// ── actor ──

/// One actor iteration's winner: a topic read, or a request job (`None` once every
/// sender has dropped, i.e. [`disconnect`] was called).
enum Step {
    Topic(Result<IncomingTopic, TransportError>),
    Job(Option<(Job, oneshot::Sender<JobResult>)>),
}

/// Own the `Client` for the session's life: interleave topic reads with request
/// jobs until the link drops or every job sender is gone, then clear the session.
async fn actor(
    mut client: Client<BridgeTransport>,
    mut jobs: UnboundedReceiver<(Job, oneshot::Sender<JobResult>)>,
    on_topic: js_sys::Function,
) {
    loop {
        // Race a topic read against the next job. Biased to jobs so a queued
        // request runs promptly; the dropped topic read is cancel-safe. The
        // futures (and the `client` borrow) are confined to this block, leaving
        // `client` free to run the job below.
        let step = {
            let topic = client.next_event().fuse();
            let job = jobs.next().fuse();
            pin_mut!(topic, job);
            select_biased! {
                j = job => Step::Job(j),
                t = topic => Step::Topic(t),
            }
        };
        match step {
            Step::Job(Some((job, reply))) => {
                let _ = reply.send(job(&mut client).await);
            }
            Step::Job(None) => break,
            Step::Topic(Ok(IncomingTopic::Topic(t))) => {
                let value = serde_wasm_bindgen::to_value(&t).unwrap_or(JsValue::NULL);
                let _ = on_topic.call1(&JsValue::NULL, &value);
            }
            // Undecodable topic (no decoder for its `cmd`): nothing to surface.
            Step::Topic(Ok(IncomingTopic::Unknown(_))) => {}
            Step::Topic(Err(TransportError::Disconnected)) => break,
            // A transient read error is not fatal; keep serving the session.
            Step::Topic(Err(_)) => {}
        }
    }
    SESSION.with(|s| *s.borrow_mut() = None);
}

// ── session lifecycle ──

/// Handshake over an already-open JS link, spawn the actor, and store the session;
/// returns the device capabilities (a JS object). `on_topic` is called with one
/// JS value per topic push. Replaces any prior session.
#[wasm_bindgen]
pub async fn connect(link: JsLink, on_topic: js_sys::Function) -> Result<JsValue, JsValue> {
    let client = Client::connect(BridgeTransport::new(link))
        .await
        .map_err(|e| connect_err(&e))?;
    let caps = serde_wasm_bindgen::to_value(client.capabilities()).map_err(|e| js_err("Serialize", &e.to_string()))?;
    let version =
        serde_wasm_bindgen::to_value(&client.protocol_version()).map_err(|e| js_err("Serialize", &e.to_string()))?;

    let (tx, rx) = mpsc::unbounded();
    spawn_local(actor(client, rx, on_topic));
    SESSION.with(|s| {
        *s.borrow_mut() = Some(Session {
            jobs: tx,
            caps: caps.clone(),
            version,
        })
    });
    Ok(caps)
}

/// Drop the session: the actor's job senders go away, so it stops and releases the
/// `Client` (and its JS link). Idempotent.
#[wasm_bindgen]
pub fn disconnect() {
    SESSION.with(|s| *s.borrow_mut() = None);
}

/// The capabilities cached at connect time (no wire traffic).
#[wasm_bindgen]
pub fn capabilities() -> Result<JsValue, JsValue> {
    SESSION.with(|s| {
        s.borrow()
            .as_ref()
            .map(|x| x.caps.clone())
            .ok_or_else(|| js_err("NoSession", "no active session"))
    })
}

/// The protocol version negotiated at connect time.
#[wasm_bindgen]
pub fn protocol_version() -> Result<JsValue, JsValue> {
    SESSION.with(|s| {
        s.borrow()
            .as_ref()
            .map(|x| x.version.clone())
            .ok_or_else(|| js_err("NoSession", "no active session"))
    })
}

/// Drop a stalled partial frame so the next request starts clean.
#[wasm_bindgen]
pub async fn resync() -> Result<JsValue, JsValue> {
    run(async move |c| {
        c.resync();
        Ok::<(), RequestError>(())
    })
    .await
}

/// Count of topic pushes the driver dropped (queue full).
#[wasm_bindgen]
pub async fn events_dropped() -> Result<JsValue, JsValue> {
    run(async move |c| Ok::<u64, RequestError>(c.events_dropped())).await
}

// ── request endpoints ──

/// Generate the flat `#[wasm_bindgen]` request wrappers from a table. Each row is
/// `method(scalar: ty, …; value: Ty)`: scalars pass straight to the client method;
/// the optional trailing `; value: Ty` arg arrives as a `JsValue` and is
/// deserialized first. Each row expands to one actor round trip, mirroring `rynk/src/api.rs`.
macro_rules! endpoints {
    ($( $name:ident ( $($s:ident : $sty:ty),* $(; $j:ident : $jty:ty)? ) ),* $(,)?) => {$(
        #[wasm_bindgen]
        pub async fn $name($($s: $sty,)* $($j: JsValue)?) -> Result<JsValue, JsValue> {
            $( let $j: $jty = parse($j)?; )?
            run(async move |c| c.$name($($s,)* $($j)?).await).await
        }
    )*};
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
