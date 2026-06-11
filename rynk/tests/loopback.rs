//! Cross-stack end-to-end test: the real [`rynk::Client`] driven against the
//! real firmware [`rmk::host::HostService::run_session`] over an in-memory duplex.
//!
//! Every other Rynk test exercises only one half against a hand-written mock of
//! the other (the firmware's `rynk_loopback` re-implements the host; the client's
//! unit tests use a `MockTransport`). This is the one test that runs BOTH
//! production halves against each other, so it locks the protocol conventions
//! that the shared `rmk-types` codec alone does not pin:
//!
//! - the version handshake (the only cross-build compatibility signal),
//! - the negotiated `max_payload_size` (firmware-advertised, host-consumed),
//! - seq correlation + cmd echo on responses,
//! - the `Result<T, RynkError>` response envelope (incl. a device rejection),
//! - server→host topic push decoding.
//!
//! It runs on tokio: `run_session` is executor-agnostic (only `embassy_futures`),
//! and the rynk path reads no clock, so no embassy-time MockDriver pump is needed.
//! The duplex is two `embassy_sync::pipe::Pipe`s — the same kind the in-firmware
//! harness (`rmk/tests/common/rynk_link.rs`) uses. Both halves consume the pipes
//! through the same embedded-io traits; the host side only needs [`Duplex`] to
//! pair the two directions into one `Read + Write` value.

use embassy_futures::select::{Either, select};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::pipe::Pipe;
use embedded_io_async::{ErrorType, Read, Write};
use rmk::config::{BehaviorConfig, PositionalConfig, RmkConfig};
use rmk::event::{LayerChangeEvent, publish_event};
use rmk::host::HostService as RynkService;
use rmk::keymap::{KeyMap, KeymapData};
use rmk_types::action::KeyAction;
use rmk_types::constants::RYNK_BUFFER_SIZE;
use rmk_types::protocol::rynk::{ProtocolVersion, RYNK_HEADER_SIZE, RynkError, StorageResetMode};
use rynk::{Client, Event, RequestError};

/// One direction of the in-memory link. Sized to a full Rynk buffer so any
/// single legal frame fits without the writer blocking on an un-polled reader.
type Link = Pipe<NoopRawMutex, RYNK_BUFFER_SIZE>;

/// Host-side `Read + Write` over the two pipes — reads device→host, writes
/// host→device. `&Pipe` already implements the embedded-io traits; this only
/// pairs the directions.
struct Duplex<'p> {
    rx: &'p Link,
    tx: &'p Link,
}

impl ErrorType for Duplex<'_> {
    type Error = core::convert::Infallible;
}

impl Read for Duplex<'_> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        // Qualified: `Pipe`'s inherent infallible `read` would shadow the trait.
        let mut rx: &Link = self.rx;
        Read::read(&mut rx, buf).await
    }
}

impl Write for Duplex<'_> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        let mut tx: &Link = self.tx;
        Write::write(&mut tx, buf).await
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[tokio::test(flavor = "current_thread")]
async fn client_against_run_session() {
    // ── firmware side: a tiny 2-layer × 2-row × 2-col keymap + service ──
    let mut behavior = BehaviorConfig::default();
    let positional: PositionalConfig<2, 2> = PositionalConfig::default();
    let mut data: KeymapData<2, 2, 2, 0> = KeymapData::new([[[KeyAction::No; 2]; 2]; 2]);
    let keymap = KeyMap::new(&mut data, &mut behavior, &positional).await;
    let config: RmkConfig<'static> = RmkConfig::default();
    let service = RynkService::new(&keymap, &config);

    // ── in-memory duplex: h2d carries requests, d2h carries responses + topics ──
    let h2d = Link::new();
    let d2h = Link::new();
    let mut dev_rx: &Link = &h2d;
    let mut dev_tx: &Link = &d2h;

    let transport = Duplex { rx: &d2h, tx: &h2d };

    // ── host side: the real Client, exercising the full seam ──
    let script = async {
        let mut client = Client::connect(transport).await.expect("handshake should succeed");

        // Handshake: both halves must agree on the protocol version.
        assert_eq!(client.protocol_version(), ProtocolVersion::CURRENT);

        // Capabilities reflect the live keymap …
        let caps = *client.capabilities();
        assert_eq!((caps.num_layers, caps.num_rows, caps.num_cols), (2, 2, 2));
        // … and the negotiated payload limit the client consumes equals the
        // firmware's own buffer floor (header + max_payload == RYNK_BUFFER_SIZE).
        assert_eq!(caps.max_payload_size as usize, RYNK_BUFFER_SIZE - RYNK_HEADER_SIZE);

        // A Get round-trip: seq correlation + cmd echo + Ok envelope.
        assert_eq!(client.get_current_layer().await.unwrap(), 0);

        // A Get with a request payload + typed decode of the response.
        assert_eq!(client.get_key(0, 0, 0).await.unwrap(), KeyAction::No);

        // A Set + readback through the real persistence path (the flash channel
        // is drained concurrently below). The 2-layer keymap lets the default
        // move off layer 0 so the readback observes the write.
        client.set_default_layer(1).await.unwrap();
        assert_eq!(client.get_default_layer().await.unwrap(), 1);

        // A device rejection must flatten to RequestError::Rejected end to end:
        // the firmware implements only StorageResetMode::Full.
        let rejected = client.storage_reset(StorageResetMode::LayoutOnly).await;
        assert!(
            matches!(rejected, Err(RequestError::Rejected(RynkError::Unimplemented))),
            "expected Rejected(Unimplemented), got {rejected:?}"
        );

        // A server→host topic push, decoded into a typed Event.
        publish_event(LayerChangeEvent::new(3));
        let ev = client.next_event().await.unwrap();
        assert!(
            matches!(ev, Event::LayerChange(3)),
            "expected LayerChange(3), got {ev:?}"
        );
    };

    // Drive the session + flash-channel drainer concurrently with the script. The
    // pipes never EOF, so the session would loop forever; it is dropped once the
    // script returns. If the session resolves first, that's a framing bug.
    let device = select(
        service.run_session(&mut dev_rx, &mut dev_tx),
        rmk::channel::drain_flash_channel_for_test(),
    );
    match select(device, script).await {
        Either::First(_) => panic!("run_session ended before the client script finished"),
        Either::Second(()) => {}
    }
}
