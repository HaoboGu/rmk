use embassy_sync::once_lock::OnceLock;
use rmk_types::DEVICE_INFO_STRING_MAX_LEN;

/// Keyboard device identity for USB descriptors and BLE advertising
#[derive(Clone, Copy, Debug)]
pub struct DeviceConfig<'a> {
    /// Vender id
    pub vid: u16,
    /// Product id
    pub pid: u16,
    /// Manufacturer
    pub manufacturer: &'a str,
    /// Product name
    pub product_name: &'a str,
    /// Stable per-device ID. Hosts see it after the host protocol's marker (`vial:f64c2b3c:` or
    /// `rynk:`), which replaces any marker already in it, or `RMK` when it is empty. The default
    /// derives it from the hardware UID on nRF and ESP32 BLE builds.
    /// The ID and protocol marker together must fit in [`DEVICE_INFO_STRING_MAX_LEN`] bytes.
    pub serial_number: &'a str,
}

// Serial number prefix for the enabled host protocol; Vial discovers keyboards by its marker.
#[cfg(feature = "vial")]
const PROTOCOL_MARKER: &str = "vial:f64c2b3c:";
#[cfg(all(not(feature = "vial"), any(feature = "rynk", feature = "dongle")))]
const PROTOCOL_MARKER: &str = rmk_types::protocol::rynk::RYNK_MAGIC;
#[cfg(not(any(feature = "vial", feature = "rynk", feature = "dongle")))]
const PROTOCOL_MARKER: &str = "";

impl DeviceConfig<'_> {
    /// The serial number hosts see: the protocol marker, then the device ID or `RMK`.
    pub(crate) fn host_serial_number(&self) -> heapless::String<DEVICE_INFO_STRING_MAX_LEN> {
        // Accept IDs that still carry a marker, including the older `vial:f64c2b3c;` form.
        let id = match self.serial_number.strip_prefix("vial:f64c2b3c") {
            Some(rest) => rest.trim_start_matches([':', ';']),
            None => self.serial_number.strip_prefix("rynk:").unwrap_or(self.serial_number),
        };
        let id = if id.is_empty() { "RMK" } else { id };
        let mut serial = heapless::String::new();
        serial
            .push_str(PROTOCOL_MARKER)
            .and_then(|_| serial.push_str(id))
            .expect("serial_number exceeds DEVICE_INFO_STRING_MAX_LEN including its protocol marker");
        serial
    }
}

/// A 6-digit hex device ID for [`DeviceConfig::serial_number`]: a UID of up to 3 bytes as
/// given, a longer one hashed. Firmware has one UID, so later calls return the first ID.
pub fn serial_number_from_uid(uid: &[u8]) -> &'static str {
    static SERIAL: OnceLock<heapless::String<6>> = OnceLock::new();
    SERIAL.get_or_init(|| device_id(uid)).as_str()
}

fn device_id(uid: &[u8]) -> heapless::String<6> {
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
    (0..6)
        .rev()
        .map(|digit| HEX[(id >> (digit * 4)) as usize & 0xf] as char)
        .collect()
}

/// Read a device ID from the RP2040's external flash UID, or an empty one when the flash
/// returns no usable UID. Pass `p.FLASH.reborrow()` before constructing the storage driver.
#[cfg(feature = "rp2040")]
pub fn rp2040_serial_number(flash: embassy_rp::Peri<'_, embassy_rp::peripherals::FLASH>) -> &'static str {
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
        Ok(()) if uid.iter().any(|&b| b != 0) && uid.iter().any(|&b| b != 0xff) => serial_number_from_uid(uid),
        _ => "",
    }
}

impl Default for DeviceConfig<'_> {
    fn default() -> Self {
        #[cfg(any(feature = "_nrf_ble", feature = "_esp_ble"))]
        let serial_number = {
            // Kept apart from `serial_number_from_uid`, so a UID passed there is never shadowed.
            static SERIAL: OnceLock<heapless::String<6>> = OnceLock::new();
            SERIAL
                .get_or_init(|| {
                    #[cfg(feature = "_nrf_ble")]
                    {
                        #[cfg(any(feature = "nrf54l15_ble", feature = "nrf54lm20_ble"))]
                        let id = embassy_nrf::pac::FICR.deviceaddr(1).read();
                        #[cfg(not(any(feature = "nrf54l15_ble", feature = "nrf54lm20_ble")))]
                        let id = embassy_nrf::pac::FICR.deviceid(1).read();
                        // The top 24 bits keep the serial numbers nRF keyboards already report.
                        device_id(&id.to_be_bytes()[..3])
                    }
                    // The first three bytes are Espressif's OUI, shared across chips.
                    #[cfg(feature = "_esp_ble")]
                    device_id(&esp_hal::efuse::base_mac_address().as_bytes()[3..])
                })
                .as_str()
        };
        #[cfg(not(any(feature = "_nrf_ble", feature = "_esp_ble")))]
        let serial_number = "";
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
    use super::{DEVICE_INFO_STRING_MAX_LEN, DeviceConfig, PROTOCOL_MARKER, device_id};

    fn host_serial(serial_number: &str) -> heapless::String<DEVICE_INFO_STRING_MAX_LEN> {
        DeviceConfig {
            serial_number,
            ..DeviceConfig::default()
        }
        .host_serial_number()
    }

    fn marked(id: &str) -> heapless::String<DEVICE_INFO_STRING_MAX_LEN> {
        let mut serial = heapless::String::new();
        serial.push_str(PROTOCOL_MARKER).unwrap();
        serial.push_str(id).unwrap();
        serial
    }

    #[test]
    fn short_uid_is_used_as_given() {
        assert_eq!(device_id(&[0x01, 0x23, 0xab]), "0123ab");
        assert_eq!(device_id(&[0x7f]), "00007f");
    }

    #[test]
    fn long_uid_hash_is_stable() {
        // Changing the hash changes the serial number of every board that uses it.
        let uid = [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef];
        assert_eq!(device_id(&uid), "3f1176");
    }

    #[test]
    fn explicit_id_gets_the_protocol_marker() {
        assert_eq!(host_serial("000001"), marked("000001"));
    }

    #[test]
    fn existing_markers_are_replaced() {
        assert_eq!(host_serial("vial:f64c2b3c:000001"), marked("000001"));
        assert_eq!(host_serial("vial:f64c2b3c;board:1"), marked("board:1"));
        assert_eq!(host_serial("rynk:000001"), marked("000001"));
    }

    #[test]
    fn missing_id_falls_back_to_rmk() {
        assert_eq!(host_serial(""), marked("RMK"));
        assert_eq!(host_serial("vial:f64c2b3c"), marked("RMK"));
    }

    #[test]
    fn serial_number_fills_the_shared_capacity() {
        let id = "0123456789abcdef0123456789abcdef";
        let id = &id[..DEVICE_INFO_STRING_MAX_LEN - PROTOCOL_MARKER.len()];
        let serial = host_serial(id);
        assert_eq!(serial.len(), DEVICE_INFO_STRING_MAX_LEN);
        assert_eq!(serial, marked(id));
    }

    #[test]
    #[should_panic(expected = "serial_number exceeds DEVICE_INFO_STRING_MAX_LEN including its protocol marker")]
    fn serial_number_rejects_one_byte_over_capacity() {
        let id = "0123456789abcdef0123456789abcdef0";
        host_serial(&id[..DEVICE_INFO_STRING_MAX_LEN + 1 - PROTOCOL_MARKER.len()]);
    }
}
