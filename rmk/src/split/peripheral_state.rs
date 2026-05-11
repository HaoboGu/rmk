//! Live tracker for per-peripheral split connection + battery state.
//!
//! `PeripheralConnectedEvent` / `PeripheralBatteryEvent` are published as
//! transient events, but Rynk's `Cmd::GetPeripheralStatus` is a synchronous
//! poll-style read — it needs the *latest* snapshot, not a stream. This
//! module bridges the two: every publish site mirrors the value into a
//! per-slot static, and the handler reads it back.
//!
//! Updates happen on the publishing thread (no race with handlers — the
//! `RawMutex` is mutex over `Cell`, identical to the pattern in
//! `state.rs::CONNECTION_STATUS`).

use core::cell::Cell;

use embassy_sync::blocking_mutex::Mutex;
use rmk_types::battery::BatteryStatus;
#[cfg(all(feature = "rynk", feature = "_ble", feature = "split"))]
use rmk_types::protocol::rynk::PeripheralStatus;

use crate::RawMutex;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct Slot {
    connected: bool,
    battery: BatteryStatus,
}

impl Slot {
    const fn new() -> Self {
        Self {
            connected: false,
            battery: BatteryStatus::Unavailable,
        }
    }
}

/// One slot per peripheral. `SPLIT_PERIPHERALS_NUM` is a build-constant
/// emitted from `keyboard.toml`, so the array sizes itself to the user's
/// configured split layout.
static SLOTS: Mutex<RawMutex, [Cell<Slot>; crate::SPLIT_PERIPHERALS_NUM]> =
    Mutex::new([const { Cell::new(Slot::new()) }; crate::SPLIT_PERIPHERALS_NUM]);

/// Record the latest connected state for peripheral `id`. Silently ignores
/// out-of-range ids so call sites can publish unconditionally.
pub(crate) fn record_connected(id: usize, connected: bool) {
    SLOTS.lock(|slots| {
        if let Some(cell) = slots.get(id) {
            let mut s = cell.get();
            s.connected = connected;
            cell.set(s);
        }
    });
}

/// Record the latest battery status for peripheral `id`. Silently ignores
/// out-of-range ids.
pub(crate) fn record_battery(id: usize, battery: BatteryStatus) {
    SLOTS.lock(|slots| {
        if let Some(cell) = slots.get(id) {
            let mut s = cell.get();
            s.battery = battery;
            cell.set(s);
        }
    });
}

/// Look up the latest snapshot for peripheral `id`, packaged in the wire
/// shape. Returns `None` when `id` is out of range — the handler maps that
/// to `RynkError::InvalidParameter`.
#[cfg(all(feature = "rynk", feature = "_ble", feature = "split"))]
pub(crate) fn peripheral_status(id: usize) -> Option<PeripheralStatus> {
    SLOTS.lock(|slots| {
        slots.get(id).map(|cell| {
            let s = cell.get();
            PeripheralStatus {
                connected: s.connected,
                battery: s.battery,
            }
        })
    })
}
