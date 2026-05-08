//! Topic publisher tasks.
//!
//! One task per active transport. Each task owns a `Sender<Tx>` (cloned from
//! the per-transport `Server`) and a typed `EventSubscriber` per topic, then
//! `select!`s across them in a loop, publishing each event with a wrapping
//! `u32` `VarSeq` minted locally. Per-task seq counters keep USB and BLE
//! sequence spaces independent — no cross-transport coordination needed.
//!
//! BLE-only topics (`BatteryStatus`, `BleStatusChange`) are exposed only by the
//! BLE publisher (`run_ble_topic_publisher`). The USB publisher
//! (`run_usb_topic_publisher`) never subscribes to those — that keeps the
//! event-channel `subs` count exactly 1 per BLE-only topic in dual-transport
//! builds, matching the `[rmk_protocol, _ble]` block in
//! `subscriber_default.toml`.

use embassy_futures::select::{Either, Either4, select, select4};
#[cfg(feature = "_ble")]
use embassy_futures::select::{Either3, select3};
use postcard_rpc::header::VarSeq;
use postcard_rpc::server::{Sender, WireTx};
#[cfg(feature = "_ble")]
use rmk_types::protocol::rmk::{BatteryStatusTopic, BleStatusChangeTopic};
use rmk_types::protocol::rmk::{
    ConnectionChangeTopic, LayerChangeTopic, LedIndicatorTopic, SleepStateTopic, WpmUpdateTopic,
};

#[cfg(feature = "_ble")]
use crate::event::{BatteryStatusEvent, BleStatusChangeEvent};
use crate::event::{
    ConnectionChangeEvent, EventSubscriber, LayerChangeEvent, LedIndicatorEvent, SleepStateEvent, SubscribableEvent,
    WpmUpdateEvent,
};

/// Helper: mint the next wrapping `Seq4`.
fn make_next_seq() -> impl FnMut() -> VarSeq {
    let mut seq: u32 = 0;
    move || {
        let s = seq;
        seq = seq.wrapping_add(1);
        VarSeq::Seq4(s)
    }
}

/// Publish loop for the 5 base topics (USB + BLE). Forever-running.
///
/// `Tx::Error`s on `publish` are dropped — the topic stream is best-effort.
/// A connection-closed error from one event won't terminate the loop; the
/// per-transport `Server::run` wrapper restarts the connection state.
pub async fn run_usb_topic_publisher<Tx: WireTx + Clone>(sender: Sender<Tx>) -> ! {
    let mut next_seq = make_next_seq();
    let mut layer_sub = LayerChangeEvent::subscriber();
    let mut conn_sub = ConnectionChangeEvent::subscriber();
    let mut sleep_sub = SleepStateEvent::subscriber();
    let mut led_sub = LedIndicatorEvent::subscriber();
    let mut wpm_sub = WpmUpdateEvent::subscriber();

    loop {
        // embassy-futures' `select` family caps at four arms; we use a 4+1
        // split for the 5 base topics.
        let base = async {
            select4(
                layer_sub.next_event(),
                conn_sub.next_event(),
                sleep_sub.next_event(),
                led_sub.next_event(),
            )
            .await
        };
        match select(base, wpm_sub.next_event()).await {
            Either::First(Either4::First(e)) => {
                let _ = sender.publish::<LayerChangeTopic>(next_seq(), &e.0).await;
            }
            Either::First(Either4::Second(e)) => {
                let _ = sender.publish::<ConnectionChangeTopic>(next_seq(), &e.0).await;
            }
            Either::First(Either4::Third(e)) => {
                let _ = sender.publish::<SleepStateTopic>(next_seq(), &e.0).await;
            }
            Either::First(Either4::Fourth(e)) => {
                let _ = sender.publish::<LedIndicatorTopic>(next_seq(), &e.0).await;
            }
            Either::Second(e) => {
                let _ = sender.publish::<WpmUpdateTopic>(next_seq(), &e.0).await;
            }
        }
    }
}

/// Publish loop including BLE-only topics. Forever-running.
#[cfg(feature = "_ble")]
pub async fn run_ble_topic_publisher<Tx: WireTx + Clone>(sender: Sender<Tx>) -> ! {
    let mut next_seq = make_next_seq();
    let mut layer_sub = LayerChangeEvent::subscriber();
    let mut conn_sub = ConnectionChangeEvent::subscriber();
    let mut sleep_sub = SleepStateEvent::subscriber();
    let mut led_sub = LedIndicatorEvent::subscriber();
    let mut wpm_sub = WpmUpdateEvent::subscriber();
    let mut battery_sub = BatteryStatusEvent::subscriber();
    let mut ble_status_sub = BleStatusChangeEvent::subscriber();

    loop {
        let base = async {
            select4(
                layer_sub.next_event(),
                conn_sub.next_event(),
                sleep_sub.next_event(),
                led_sub.next_event(),
            )
            .await
        };
        let extra = async {
            select3(
                wpm_sub.next_event(),
                battery_sub.next_event(),
                ble_status_sub.next_event(),
            )
            .await
        };
        match select(base, extra).await {
            Either::First(Either4::First(e)) => {
                let _ = sender.publish::<LayerChangeTopic>(next_seq(), &e.0).await;
            }
            Either::First(Either4::Second(e)) => {
                let _ = sender.publish::<ConnectionChangeTopic>(next_seq(), &e.0).await;
            }
            Either::First(Either4::Third(e)) => {
                let _ = sender.publish::<SleepStateTopic>(next_seq(), &e.0).await;
            }
            Either::First(Either4::Fourth(e)) => {
                let _ = sender.publish::<LedIndicatorTopic>(next_seq(), &e.0).await;
            }
            Either::Second(Either3::First(e)) => {
                let _ = sender.publish::<WpmUpdateTopic>(next_seq(), &e.0).await;
            }
            Either::Second(Either3::Second(e)) => {
                let _ = sender.publish::<BatteryStatusTopic>(next_seq(), &e.0).await;
            }
            Either::Second(Either3::Third(e)) => {
                let _ = sender.publish::<BleStatusChangeTopic>(next_seq(), &e.0).await;
            }
        }
    }
}
