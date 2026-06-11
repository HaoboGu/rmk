//! The steady-state app loop a CLI/GUI runs: one task owns the [`Client`] and
//! interleaves topic delivery with periodic requests under `select!`, then
//! rebuilds the client when the link dies.
//!
//! `next_event` is cancel-safe, so the `select!` branch that loses simply drops
//! its future. Requests are issued *after* `select!` returns, never racing it.
//!
//! ```text
//! cargo run --example app_loop      # USB CDC serial
//! ```

use std::time::Duration;

use log::{error, info, warn};
use rynk::{Client, RequestError, RynkDevice, TransportError};
use rynk_serial::SerialDevice;

/// Per-request ceiling. A half-open link (a frame header with no payload behind
/// it) can't stall the request — and so the whole loop — longer than this.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

/// Bounds the handshake so a silent peer can't wedge the connect loop.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(2);

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp(None)
        .format_target(false)
        .init();

    let mut client = connect::<SerialDevice>().await;
    let mut poll = tokio::time::interval(Duration::from_secs(2));

    loop {
        tokio::select! {
            // Topic pushes arrive while we await the link. Cancel-safe: when
            // the poll branch wins, this future is dropped with no ill effect.
            event = client.next_event() => match event {
                Ok(ev) => info!("topic {ev:?}"),
                Err(TransportError::Disconnected) => client = reconnect::<SerialDevice>(client).await,
                Err(e) => warn!("event error: {e}"),
            },
            // Periodic request, issued only after select! returns. Bounded by a
            // timeout: a half-open link would otherwise block the request — and
            // with it the whole loop — indefinitely. On timeout, `resync` drops
            // the stalled partial frame so the next tick starts clean.
            _ = poll.tick() => match tokio::time::timeout(REQUEST_TIMEOUT, client.get_wpm()).await {
                Ok(Ok(wpm)) => info!("wpm = {wpm}"),
                Ok(Err(RequestError::Transport(TransportError::Disconnected))) => client = reconnect::<SerialDevice>(client).await,
                Ok(Err(e)) => warn!("get_wpm failed: {e}"),
                Err(_elapsed) => {
                    warn!("request timed out — resyncing");
                    client.resync();
                }
            },
        }
    }
}

/// Connect, retrying every second until a keyboard answers. Generic over the
/// transport; with several keyboards attached a real app lists `D::discover()` and
/// lets the user pick — this demo just takes the first discovered device.
async fn connect<D: RynkDevice>() -> Client<D::Transport> {
    loop {
        match D::discover().await {
            Ok(devices) if !devices.is_empty() => {
                let device = &devices[0];
                // `connect` is runtime-free and unbounded; cap the handshake so a
                // silent peer can't wedge this loop.
                match tokio::time::timeout(HANDSHAKE_TIMEOUT, device.connect()).await {
                    Ok(Ok(client)) => {
                        info!("connected to {}", device.label());
                        return client;
                    }
                    Ok(Err(e)) => warn!("connect failed ({e}); retrying in 1s"),
                    Err(_) => warn!("handshake timed out; retrying in 1s"),
                }
            }
            Ok(_) => warn!("no Rynk keyboard found; retrying in 1s"),
            Err(e) => warn!("discover failed ({e}); retrying in 1s"),
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

/// The link died — drop the old client and build a fresh one. Re-handshaking is
/// required: the reconnected device may differ.
async fn reconnect<D: RynkDevice>(old: Client<D::Transport>) -> Client<D::Transport> {
    error!("link lost — reconnecting");
    drop(old);
    connect::<D>().await
}
