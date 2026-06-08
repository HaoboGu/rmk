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
use rmk_host::{Client, RequestError, TransportError};
use rmk_host_serial::{SerialTransport, connect_serial};

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp(None)
        .format_target(false)
        .init();

    let mut client = connect().await;
    info!("connected to {}", client.transport().path());
    let mut poll = tokio::time::interval(Duration::from_secs(2));

    loop {
        tokio::select! {
            // Topic pushes arrive while we await the link. Cancel-safe: when
            // the poll branch wins, this future is dropped with no ill effect.
            event = client.next_event() => match event {
                Ok(frame) => info!("topic {:?} ({} bytes)", frame.cmd, frame.payload.len()),
                Err(TransportError::Disconnected) => client = reconnect(client).await,
                Err(e) => warn!("event error: {e}"),
            },
            // Periodic request, issued only after select! returns.
            _ = poll.tick() => match client.get_wpm().await {
                Ok(wpm) => info!("wpm = {wpm}"),
                Err(RequestError::Transport(TransportError::Disconnected)) => client = reconnect(client).await,
                Err(e) => warn!("get_wpm failed: {e}"),
            },
        }
    }
}

/// Connect, retrying every second until a keyboard answers.
async fn connect() -> Client<SerialTransport> {
    loop {
        match connect_serial().await {
            Ok(client) => return client,
            Err(e) => {
                warn!("connect failed ({e}); retrying in 1s");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

/// The link died — drop the old client and build a fresh one. Re-handshaking is
/// required: the reconnected device may differ.
async fn reconnect(old: Client<SerialTransport>) -> Client<SerialTransport> {
    error!("link lost — reconnecting");
    drop(old);
    let client = connect().await;
    info!("reconnected to {}", client.transport().path());
    client
}
