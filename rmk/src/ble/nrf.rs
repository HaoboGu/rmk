use embassy_sync::once_lock::OnceLock;
use heapless::String;

pub(crate) fn get_serial_number() -> &'static str {
    static SERIAL: OnceLock<String<20>> = OnceLock::new();

    let serial = SERIAL.get_or_init(|| {
        let ficr = embassy_nrf::pac::FICR;
        #[cfg(any(feature = "nrf54l15_ble", feature = "nrf54lm20_ble"))]
        let device_id = (u64::from(ficr.deviceaddr(1).read()) << 32) | u64::from(ficr.deviceaddr(0).read());
        #[cfg(not(any(feature = "nrf54l15_ble", feature = "nrf54lm20_ble")))]
        let device_id = (u64::from(ficr.deviceid(1).read()) << 32) | u64::from(ficr.deviceid(0).read());

        let mut result = String::new();
        let _ = result.push_str("vial:f64c2b3c:");

        // Hex lookup table
        const HEX_TABLE: &[u8] = b"0123456789abcdef";
        // Add 6 hex digits to the serial number, as the serial str in BLE Device Information Service is limited to 20 bytes
        for i in 0..6 {
            let digit = (device_id >> (60 - i * 4)) & 0xF;
            // This index access is safe because digit is guaranteed to be in the range of 0-15
            let hex_char = HEX_TABLE[digit as usize] as char;
            let _ = result.push(hex_char);
        }

        result
    });

    serial.as_str()
}
