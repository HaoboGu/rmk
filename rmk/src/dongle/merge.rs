//! HID merge: several keyboards, one host-facing device.
//!
//! Only keyboard reports need real merging — the merger keeps each link's last
//! boot report and unions them. Pointer/media/system reports pass through and
//! are parsed back into typed reports here.

use core::cell::RefCell;

use embassy_sync::blocking_mutex::Mutex;
use usbd_hid::descriptor::{MediaKeyboardReport, MouseReport, SystemControlReport};

use crate::hid::KeyboardReport;
use crate::{DONGLE_LINKS_NUM, RawMutex};

static KB_SNAPSHOTS: Mutex<RawMutex, RefCell<[Option<[u8; 8]>; DONGLE_LINKS_NUM]>> =
    Mutex::new(RefCell::new([None; DONGLE_LINKS_NUM]));

/// Record `raw` as link `link`'s latest boot report and return the new merge.
pub(crate) fn update_keyboard(link: u8, raw: [u8; 8]) -> KeyboardReport {
    KB_SNAPSHOTS.lock(|s| {
        let mut snaps = s.borrow_mut();
        snaps[link as usize] = Some(raw);
        merge(&*snaps)
    })
}

/// Drop link `link`'s snapshot (it disconnected) and return the new merge, so
/// keys it held are released on the host.
pub(crate) fn clear_link(link: u8) -> KeyboardReport {
    KB_SNAPSHOTS.lock(|s| {
        let mut snaps = s.borrow_mut();
        snaps[link as usize] = None;
        merge(&*snaps)
    })
}

/// Union of the per-link boot reports: modifiers OR'd, keycodes unioned into
/// the 6 boot slots, overflow dropped (design §4.6).
fn merge(snaps: &[Option<[u8; 8]>]) -> KeyboardReport {
    let mut report = KeyboardReport::default();
    let mut n = 0;
    for snap in snaps.iter().flatten() {
        report.modifier |= snap[0];
        for &kc in &snap[2..8] {
            if kc != 0 && n < 6 && !report.keycodes[..n].contains(&kc) {
                report.keycodes[n] = kc;
                n += 1;
            }
        }
    }
    report
}

/// Parse the 5-byte BLE mouse report back into a typed report (pass-through).
pub(crate) fn parse_mouse(raw: &[u8]) -> MouseReport {
    MouseReport {
        buttons: raw[0],
        x: raw[1] as i8,
        y: raw[2] as i8,
        wheel: raw[3] as i8,
        pan: raw[4] as i8,
    }
}

pub(crate) fn parse_media(raw: &[u8]) -> MediaKeyboardReport {
    MediaKeyboardReport {
        usage_id: u16::from_le_bytes([raw[0], raw[1]]),
    }
}

pub(crate) fn parse_system(raw: &[u8]) -> SystemControlReport {
    SystemControlReport { usage_id: raw[0] }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kb(modifier: u8, keys: &[u8]) -> [u8; 8] {
        let mut raw = [0u8; 8];
        raw[0] = modifier;
        raw[2..2 + keys.len()].copy_from_slice(keys);
        raw
    }

    #[test]
    fn merges_modifiers_and_unions_keycodes() {
        let merged = merge(&[Some(kb(0x02, &[4, 5])), Some(kb(0x01, &[5, 6]))]);
        assert_eq!(merged.modifier, 0x03);
        assert_eq!(merged.keycodes, [4, 5, 6, 0, 0, 0], "5 deduplicated");
    }

    #[test]
    fn overflow_beyond_six_keys_is_dropped() {
        let merged = merge(&[Some(kb(0, &[1, 2, 3, 4])), Some(kb(0, &[5, 6, 7, 8]))]);
        assert_eq!(merged.keycodes, [1, 2, 3, 4, 5, 6], "keys 7 and 8 dropped");
    }

    #[test]
    fn clearing_a_link_releases_its_keys() {
        update_keyboard(0, kb(0x02, &[4]));
        update_keyboard(1, kb(0x04, &[5]));
        let merged = clear_link(0);
        assert_eq!(merged.modifier, 0x04);
        assert_eq!(merged.keycodes, [5, 0, 0, 0, 0, 0]);
        let merged = clear_link(1);
        assert_eq!(merged.modifier, 0);
        assert_eq!(merged.keycodes, [0; 6]);
    }
}
