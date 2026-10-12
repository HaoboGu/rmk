use rmk_types::DEVICE_INFO_STRING_MAX_LEN;

/// Keyboard device identity for USB descriptors and BLE advertising
#[derive(Clone, Debug)]
pub struct DeviceConfig<'a> {
    /// Vender id
    pub vid: u16,
    /// Product id
    pub pid: u16,
    /// Manufacturer
    pub manufacturer: &'a str,
    /// Product name
    pub product_name: &'a str,
    /// Complete serial number reported over USB, BLE, and Rynk, including the protocol marker.
    /// Use [`build_serial_number`] to construct it from a configured ID or hardware UID.
    pub serial_number: heapless::String<DEVICE_INFO_STRING_MAX_LEN>,
}

// Serial number prefix for the enabled host protocol; Vial discovers keyboards by its marker.
#[cfg(feature = "vial")]
const PROTOCOL_MARKER: &str = "vial:f64c2b3c:";
#[cfg(all(not(feature = "vial"), any(feature = "rynk", feature = "dongle")))]
const PROTOCOL_MARKER: &str = rmk_types::protocol::rynk::RYNK_MAGIC;
#[cfg(not(any(feature = "vial", feature = "rynk", feature = "dongle")))]
const PROTOCOL_MARKER: &str = "";

/// Build the complete host-visible serial number from a configured ID or hardware UID.
/// A configured ID takes precedence, including an empty one. Without it, a UID of up to
/// 3 bytes is encoded as six hex digits; a longer UID is hashed. An empty ID falls back
/// to `RMK`. Existing protocol markers are replaced with the marker for this build.
///
/// Panics if the result exceeds [`DEVICE_INFO_STRING_MAX_LEN`] bytes.
pub fn build_serial_number(configured: Option<&str>, uid: &[u8]) -> heapless::String<DEVICE_INFO_STRING_MAX_LEN> {
    let mut generated = heapless::String::<6>::new();
    let id = if let Some(configured) = configured {
        // Accept IDs that still carry a marker, including the older `vial:f64c2b3c;` form.
        match configured.strip_prefix("vial:f64c2b3c") {
            Some(rest) => rest.trim_start_matches([':', ';']),
            None => configured.strip_prefix("rynk:").unwrap_or(configured),
        }
    } else if uid.is_empty() {
        ""
    } else {
        let id = if uid.len() <= 3 {
            uid.iter().fold(0, |id, &byte| id << 8 | u32::from(byte))
        } else {
            // 32-bit FNV-1a, XOR-folded to 24 bits
            let hash = uid.iter().fold(0x811c_9dc5_u32, |hash, &byte| {
                (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
            });
            (hash >> 24) ^ (hash & 0x00ff_ffff)
        };
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for digit in (0..6).rev() {
            generated.push(HEX[(id >> (digit * 4)) as usize & 0xf] as char).unwrap();
        }
        generated.as_str()
    };
    let id = if id.is_empty() { "RMK" } else { id };
    let mut serial = heapless::String::new();
    serial
        .push_str(PROTOCOL_MARKER)
        .and_then(|_| serial.push_str(id))
        .expect("serial_number exceeds DEVICE_INFO_STRING_MAX_LEN including its protocol marker");
    serial
}

/// Build the serial number from the RP2040's external flash UID, falling back to `RMK`
/// with the protocol marker when unavailable. Pass `p.FLASH.reborrow()` before constructing
/// the storage driver.
#[cfg(feature = "rp2040")]
pub fn rp2040_serial_number(
    flash: embassy_rp::Peri<'_, embassy_rp::peripherals::FLASH>,
) -> heapless::String<DEVICE_INFO_STRING_MAX_LEN> {
    let mut flash = embassy_rp::flash::Flash::<_, embassy_rp::flash::Blocking, 0>::new_blocking(flash);
    // Flash chips answer the Winbond UID command with 8 bytes, except Zetta's 16.
    let len = if matches!(flash.blocking_jedec_id(), Ok(0xba6015)) {
        16
    } else {
        8
    };
    let mut uid = [0; 16];
    let uid = &mut uid[..len];
    match flash.blocking_unique_id(uid) {
        // A chip without the command reads back blank.
        Ok(()) if uid.iter().any(|&b| b != 0) && uid.iter().any(|&b| b != 0xff) => build_serial_number(None, uid),
        _ => build_serial_number(None, &[]),
    }
}

impl Default for DeviceConfig<'_> {
    fn default() -> Self {
        #[cfg(feature = "_nrf_ble")]
        let serial_number = {
            #[cfg(any(feature = "nrf54l15_ble", feature = "nrf54lm20_ble"))]
            let id = embassy_nrf::pac::FICR.deviceaddr(1).read();
            #[cfg(not(any(feature = "nrf54l15_ble", feature = "nrf54lm20_ble")))]
            let id = embassy_nrf::pac::FICR.deviceid(1).read();
            // The top 24 bits keep the serial numbers nRF keyboards already report.
            build_serial_number(None, &id.to_be_bytes()[..3])
        };
        // The first three bytes are Espressif's OUI, shared across chips.
        #[cfg(feature = "_esp_ble")]
        let serial_number = build_serial_number(None, &esp_hal::efuse::base_mac_address().as_bytes()[3..]);
        #[cfg(not(any(feature = "_nrf_ble", feature = "_esp_ble")))]
        let serial_number = build_serial_number(None, &[]);
        Self {
            vid: 0x4c4b,
            pid: 0x4643,
            manufacturer: "RMK",
            product_name: "RMK Keyboard",
            serial_number,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DEVICE_INFO_STRING_MAX_LEN, DeviceConfig, PROTOCOL_MARKER, build_serial_number};

    fn marked(id: &str) -> heapless::String<DEVICE_INFO_STRING_MAX_LEN> {
        let mut serial = heapless::String::new();
        serial.push_str(PROTOCOL_MARKER).unwrap();
        serial.push_str(id).unwrap();
        serial
    }

    #[test]
    fn short_uid_is_used_as_given() {
        assert_eq!(build_serial_number(None, &[0x01, 0x23, 0xab]), marked("0123ab"));
        assert_eq!(build_serial_number(None, &[0x7f]), marked("00007f"));
        assert_eq!(build_serial_number(None, &[0, 0, 0]), marked("000000"));
    }

    #[test]
    fn long_uid_hash_is_stable() {
        // Changing the hash changes the serial number of every board that uses it.
        let uid = [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef];
        assert_eq!(build_serial_number(None, &uid), marked("3f1176"));
    }

    #[test]
    fn explicit_id_gets_the_protocol_marker() {
        assert_eq!(build_serial_number(Some("000001"), &[]), marked("000001"));
        assert_eq!(build_serial_number(Some("board-1"), &[0x7f]), marked("board-1"));
    }

    #[test]
    fn existing_markers_are_replaced() {
        assert_eq!(build_serial_number(Some("vial:f64c2b3c:000001"), &[]), marked("000001"));
        assert_eq!(
            build_serial_number(Some("vial:f64c2b3c;board:1"), &[]),
            marked("board:1")
        );
        assert_eq!(build_serial_number(Some("rynk:000001"), &[]), marked("000001"));
    }

    #[test]
    fn missing_id_falls_back_to_rmk() {
        assert_eq!(build_serial_number(None, &[]), marked("RMK"));
        assert_eq!(build_serial_number(Some(""), &[0x7f]), marked("RMK"));
        assert_eq!(build_serial_number(Some("vial:f64c2b3c"), &[]), marked("RMK"));
        assert_eq!(DeviceConfig::default().serial_number, marked("RMK"));
    }

    #[test]
    fn serial_number_fills_the_shared_capacity() {
        let id = "0123456789abcdef0123456789abcdef";
        let id = &id[..DEVICE_INFO_STRING_MAX_LEN - PROTOCOL_MARKER.len()];
        let serial = build_serial_number(Some(id), &[]);
        assert_eq!(serial.len(), DEVICE_INFO_STRING_MAX_LEN);
        assert_eq!(serial, marked(id));
    }

    #[test]
    #[should_panic(expected = "serial_number exceeds DEVICE_INFO_STRING_MAX_LEN including its protocol marker")]
    fn serial_number_rejects_one_byte_over_capacity() {
        let id = "0123456789abcdef0123456789abcdef0";
        build_serial_number(Some(&id[..DEVICE_INFO_STRING_MAX_LEN + 1 - PROTOCOL_MARKER.len()]), &[]);
    }
}
