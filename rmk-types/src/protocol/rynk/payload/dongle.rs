//! Dongle-relay protocol types.
//!
//! Payloads of the `0x09xx` command segment, answered by a tri-mode dongle
//! itself (never forwarded to a keyboard). That routing rule is also the probe:
//! a host asks `GetDongleSlots`, a keyboard answers `UnknownCmd`, a dongle
//! answers [`DongleSlots`].

use heapless::{String, Vec};
use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};

use crate::constants::MAX_DONGLE_SLOTS;

/// Maximum byte length of a slot's stored keyboard name — sized to hold the
/// `DeviceInfo::product_name` captured during the pairing handshake.
pub const DONGLE_SLOT_NAME_SIZE: usize = super::system::DEVICE_INFO_STRING_SIZE;

/// One bonded keyboard in the dongle's slot table. Carries only what survives
/// the keyboard being off: its identity and whether it is linked right now.
/// Everything else — battery, layer, keymap — is the keyboard's own protocol
/// surface, which the host reaches by forwarding to the selected target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct DongleSlot {
    pub connected: bool,
    /// Keyboard name captured from `GetDeviceInfo` during the pairing handshake.
    #[cfg_attr(feature = "wasm", tsify(type = "string"))]
    pub name: String<DONGLE_SLOT_NAME_SIZE>,
}

// A str encodes as varint length + UTF-8 bytes — the same wire shape as
// `Vec<u8, N>`, so the Vec bound covers the name field.
impl MaxSize for DongleSlot {
    const POSTCARD_MAX_SIZE: usize =
        bool::POSTCARD_MAX_SIZE + crate::heapless_vec_max_size::<u8, DONGLE_SLOT_NAME_SIZE>();
}

/// Slot-table snapshot, returned by `GetDongleSlots` and pushed as the
/// `DongleSlotsChange` topic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct DongleSlots {
    /// Index is the slot number — the argument `SelectDongleTarget` and
    /// `ForgetDongleSlot` take — and `None` is an unbonded slot. The length is
    /// the producing dongle's own slot count; the `Vec` bound is the protocol
    /// ceiling every decoder must hold, not that count.
    #[cfg_attr(feature = "wasm", tsify(type = "(DongleSlot | null)[]"))]
    pub slots: Vec<Option<DongleSlot>, MAX_DONGLE_SLOTS>,
    /// The slot configuration traffic is routed to; `None` when ambiguous
    /// (multiple bonded slots and no `SelectDongleTarget` yet).
    pub target: Option<u8>,
}

impl MaxSize for DongleSlots {
    const POSTCARD_MAX_SIZE: usize =
        crate::heapless_vec_max_size::<Option<DongleSlot>, MAX_DONGLE_SLOTS>() + <Option<u8>>::POSTCARD_MAX_SIZE;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::rynk::tests::{assert_max_size_bound, round_trip};

    #[test]
    fn round_trip_dongle_slots() {
        round_trip(&DongleSlots {
            slots: Vec::new(),
            target: None,
        });

        // Max-capacity case: full name and all fields at their widest so the
        // hand-written `MaxSize` bounds are genuinely exercised.
        let full_name: String<DONGLE_SLOT_NAME_SIZE> = String::try_from("🦀🦀🦀🦀🦀🦀🦀🦀").unwrap();
        assert_eq!(full_name.len(), DONGLE_SLOT_NAME_SIZE);
        let slot = DongleSlot {
            connected: true,
            name: full_name,
        };
        round_trip(&slot);
        assert_max_size_bound(&slot);

        let mut slots = Vec::new();
        while slots.push(Some(slot.clone())).is_ok() {}
        let table = DongleSlots {
            slots,
            target: Some(u8::MAX),
        };
        round_trip(&table);
        assert_max_size_bound(&table);
    }
}
