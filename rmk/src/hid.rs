/// Traits and types for HID message reporting and listening.
use core::future::Future;
use core::sync::atomic::Ordering;

use embassy_usb::class::hid::ReadError;
use embassy_usb::driver::EndpointError;
use rmk_types::connection::ConnectionType;
use rmk_types::led_indicator::LedIndicator;
use serde::Serialize;
use usbd_hid::descriptor::generator_prelude::*;
use usbd_hid::descriptor::{AsInputReport, MediaKeyboardReport, MouseReport, SystemControlReport};

use crate::event::{LedIndicatorEvent, publish_event};
use crate::keyboard::LOCK_LED_STATES;
use crate::state::writable_on;
#[cfg(not(feature = "_no_usb"))]
use crate::usb::USB_REMOTE_WAKEUP;

/// KeyboardReport describes a report and its companion descriptor that can be
/// used to send keyboard button presses to a host and receive the status of the
/// keyboard LEDs.
#[gen_hid_descriptor(
    (collection = APPLICATION, usage_page = GENERIC_DESKTOP, usage = KEYBOARD) = {
        (usage_page = KEYBOARD, usage_min = 0xE0, usage_max = 0xE7) = {
            #[packed_bits = 8] #[item_settings(data,variable,absolute)] modifier=input;
        };
        (logical_min = 0,) = {
            #[item_settings(constant,variable,absolute)] reserved=input;
        };
        (usage_page = LEDS, usage_min = 0x01, usage_max = 0x05) = {
            #[packed_bits = 5] #[item_settings(data,variable,absolute)] leds=output;
        };
        (usage_page = KEYBOARD, usage_min = 0x00, usage_max = 0xDD) = {
            #[item_settings(data,array,absolute)] keycodes=input;
        };
    }
)]
#[allow(dead_code)]
#[derive(Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct KeyboardReport {
    pub modifier: u8, // ModifierCombination
    pub reserved: u8,
    pub leds: u8, // LedIndicator
    pub keycodes: [u8; 6],
}

#[gen_hid_descriptor(
    (collection = APPLICATION, usage_page = 0xFF60, usage = 0x61) = {
        (usage = 0x62, logical_min = 0x0) = {
            #[item_settings(data,variable,absolute)] input_data=input;
        };
        (usage = 0x63, logical_min = 0x0) = {
            #[item_settings(data,variable,absolute)] output_data=output;
        };
    }
)]
#[derive(Default)]
pub struct ViaReport {
    pub(crate) input_data: [u8; 32],
    pub(crate) output_data: [u8; 32],
}

/// Predefined report ids for composite hid report.
/// Should be same with `#[gen_hid_descriptor]`
/// DO NOT EDIT
#[repr(u8)]
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Serialize)]

pub enum CompositeReportType {
    #[default]
    None = 0x00,
    Mouse = 0x01,
    Media = 0x02,
    System = 0x03,
}

impl CompositeReportType {
    fn from_u8(report_id: u8) -> Self {
        match report_id {
            0x01 => Self::Mouse,
            0x02 => Self::Media,
            0x03 => Self::System,
            _ => Self::None,
        }
    }
}

/// Plover HID stenography report.
///
/// Plover (v5.1+) enumerates the keyboard as a stenography machine when it
/// finds an HID device exposing usage page `0xFF50` / usage `0x4C56`; the
/// pair encodes the ASCII string `"STN"` (`0xFF`, `'S'`, `'T'`, `'N'`).
/// Once connected, Plover reads 9-byte reports (`[report_id=0x50, k0, k1,
/// ..., k7]`) where the eight payload bytes are a 64-bit big-endian bitmap
/// of the live steno chord, one bit per [`crate::types::steno::StenoKey`],
/// where `StenoKey::S1` (chart index 0) is the most significant bit of `k0`
/// and `StenoKey::X26` (chart index 63) is the least significant bit of
/// `k7`.
///
/// The descriptor is the same as the Plover HID project's reference: a
/// Logical collection containing 64 single-bit Ordinal usages.
///
/// Reference: <https://github.com/dnaq/plover-machine-hid>
#[cfg(feature = "steno")]
#[gen_hid_descriptor(
    (collection = LOGICAL, usage_page = 0xFF50, usage = 0x4C56) = {
        (report_id = 0x50, usage_page = 0x0A, usage_min = 0x0, usage_max = 0x3F, logical_min = 0x0) = {
            #[packed_bits = 64] #[item_settings(data,variable,absolute)] keys=input;
        };
    }
)]
#[derive(Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct StenoReport {
    pub keys: [u8; 8],
}

// `gen_hid_descriptor` skips the `AsInputReport` impl when a `report_id`
// is present, so the wire format must be assembled by hand: byte 0 is the
// Plover HID report ID followed by the eight chord-bitmap bytes.
#[cfg(feature = "steno")]
impl usbd_hid::descriptor::AsInputReport for StenoReport {
    fn serialize(&self, buffer: &mut [u8]) -> Result<usize, usbd_hid::descriptor::BufferOverflow> {
        if buffer.len() < 9 {
            return Err(usbd_hid::descriptor::BufferOverflow);
        }
        buffer[0] = rmk_types::steno::PLOVER_HID_REPORT_ID;
        buffer[1..9].copy_from_slice(&self.keys);
        Ok(9)
    }
}

#[cfg(all(test, feature = "steno"))]
mod steno_tests {
    use usbd_hid::descriptor::SerializedDescriptor;

    use super::StenoReport;

    #[test]
    fn descriptor_advertises_plover_identifiers() {
        let desc = StenoReport::desc();
        fn contains(haystack: &[u8], needle: &[u8]) -> bool {
            haystack.windows(needle.len()).any(|w| w == needle)
        }
        assert!(contains(desc, &[0x06, 0x50, 0xff]), "missing UsagePage 0xFF50");
        assert!(contains(desc, &[0x0a, 0x56, 0x4c]), "missing Usage 0x4C56");
        assert!(contains(desc, &[0xa1, 0x02]), "missing Logical collection");
        assert!(contains(desc, &[0x85, 0x50]), "missing ReportID 0x50");
        assert!(contains(desc, &[0x75, 0x01]), "missing ReportSize 1");
        assert!(contains(desc, &[0x95, 0x40]), "missing ReportCount 64");
        assert!(contains(desc, &[0x05, 0x0a]), "missing Ordinal UsagePage");
        assert!(contains(desc, &[0x19, 0x00]), "missing UsageMin 0");
        assert!(contains(desc, &[0x29, 0x3f]), "missing UsageMax 63");
    }
}

/// A composite hid report which contains mouse, consumer, system reports.
/// Report id is used to distinguish from them.
#[gen_hid_descriptor(
    (collection = APPLICATION, usage_page = GENERIC_DESKTOP, usage = MOUSE) = {
        (collection = PHYSICAL, usage = POINTER) = {
            (report_id = 0x01,) = {
                (usage_page = BUTTON, usage_min = BUTTON_1, usage_max = BUTTON_8) = {
                    #[packed_bits = 8] #[item_settings(data,variable,absolute)] buttons=input;
                };
                (usage_page = GENERIC_DESKTOP,) = {
                    (usage = X,) = {
                        #[item_settings(data,variable,relative)] x=input;
                    };
                    (usage = Y,) = {
                        #[item_settings(data,variable,relative)] y=input;
                    };
                    (usage = WHEEL,) = {
                        #[item_settings(data,variable,relative)] wheel=input;
                    };
                };
                (usage_page = CONSUMER,) = {
                    (usage = AC_PAN,) = {
                        #[item_settings(data,variable,relative)] pan=input;
                    };
                };
            };
        };
    },
    (collection = APPLICATION, usage_page = CONSUMER, usage = CONSUMER_CONTROL) = {
        (report_id = 0x02,) = {
            (usage_page = CONSUMER, usage_min = 0x00, usage_max = 0x514) = {
            #[item_settings(data,array,absolute,not_null)] media_usage_id=input;
            }
        };
    },
    (collection = APPLICATION, usage_page = GENERIC_DESKTOP, usage = SYSTEM_CONTROL) = {
        (report_id = 0x03,) = {
            (usage_min = 0x81, usage_max = 0xB7, logical_min = 1) = {
                #[item_settings(data,array,absolute,not_null)] system_usage_id=input;
            };
        };
    }
)]
#[derive(Default, Serialize)]
pub struct CompositeReport {
    pub(crate) buttons: u8, // MouseButtons
    pub(crate) x: i8,
    pub(crate) y: i8,
    pub(crate) wheel: i8, // Scroll down (negative) or up (positive) this many units
    pub(crate) pan: i8,   // Scroll left (negative) or right (positive) this many units
    pub(crate) media_usage_id: u16,
    pub(crate) system_usage_id: u8,
}

#[derive(Debug, Clone)]
pub enum Report {
    /// Normal keyboard hid report
    KeyboardReport(KeyboardReport),
    /// Mouse hid report
    MouseReport(MouseReport),
    /// Media keyboard report
    MediaKeyboardReport(MediaKeyboardReport),
    /// System control report
    SystemControlReport(SystemControlReport),
    /// Plover HID stenography chord report
    #[cfg(feature = "steno")]
    StenoReport(StenoReport),
}

impl AsInputReport for Report {
    fn serialize(&self, buffer: &mut [u8]) -> Result<usize, usbd_hid::descriptor::BufferOverflow> {
        match self {
            Report::KeyboardReport(r) => r.serialize(buffer),
            Report::MouseReport(r) => r.serialize(buffer),
            Report::MediaKeyboardReport(r) => r.serialize(buffer),
            Report::SystemControlReport(r) => r.serialize(buffer),
            #[cfg(feature = "steno")]
            Report::StenoReport(r) => r.serialize(buffer),
        }
    }
}

#[derive(PartialEq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum HidError {
    UsbReadError(ReadError),
    UsbEndpointError(EndpointError),
    ReportSerializeError,
    BleError,
}

/// HidWriter trait is used for reporting HID messages to the host, via USB, BLE, etc.
pub trait HidWriterTrait {
    /// The report type that the reporter receives from input processors.
    type ReportType: AsInputReport;

    /// Write report to the host, return the number of bytes written if success.
    fn write_report(&mut self, report: &Self::ReportType) -> impl Future<Output = Result<usize, HidError>>;
}

/// Runnable writer. The default `run_writer` gates wire writes on
/// `writable_on(Self::KIND)` so reports queued before an active-output flip
/// are dropped instead of written to the wrong wire.
pub trait RunnableHidWriter: HidWriterTrait {
    /// The transport this writer serves.
    const KIND: ConnectionType;

    /// Get the report to be sent to the host
    fn get_report(&mut self) -> impl Future<Output = Self::ReportType>;

    /// Run the writer task.
    fn run_writer(&mut self) -> impl Future<Output = ()> {
        async {
            loop {
                let report = self.get_report().await;
                if writable_on(Self::KIND)
                    && let Err(e) = self.write_report(&report).await
                {
                    error!("Failed to send report: {:?}", e);
                    #[cfg(not(feature = "_no_usb"))]
                    // If the USB endpoint is disabled, try wakeup and resend the same report.
                    if let HidError::UsbEndpointError(EndpointError::Disabled) = e {
                        USB_REMOTE_WAKEUP.signal(());
                        embassy_time::Timer::after_millis(200).await;
                        if let Err(e) = self.write_report(&report).await {
                            error!("Failed to send report after wakeup: {:?}", e);
                        }
                    }
                };
            }
        }
    }
}

/// HidReader trait is used for listening to HID messages from the host, via USB, BLE, etc.
///
/// HidReader only receives `[u8; READ_N]`, the raw HID report from the host.
/// Then processes the received message, forward to other tasks
pub trait HidReaderTrait {
    /// Report type
    type ReportType;

    /// Read HID report from the host
    fn read_report(&mut self) -> impl Future<Output = Result<Self::ReportType, HidError>>;
}

/// Drain LED indicator OUT reports from `reader` and republish them as
/// [`LedIndicatorEvent`]s whenever `kind` is the active output transport.
pub(crate) async fn run_led_reader<R: HidReaderTrait<ReportType = LedIndicator>>(
    reader: &mut R,
    kind: ConnectionType,
) -> ! {
    loop {
        match reader.read_report().await {
            Ok(led_indicator) => {
                info!("Got led indicator");
                if writable_on(kind) {
                    LOCK_LED_STATES.store(led_indicator.into_bits(), Ordering::Relaxed);
                    publish_event(LedIndicatorEvent::new(led_indicator));
                }
            }
            Err(e) => {
                debug!("Read HID LED indicator error: {:?}", e);
                embassy_time::Timer::after_millis(1000).await;
            }
        }
    }
}

#[cfg(feature = "_nrf_ble")]
pub(crate) fn get_serial_number() -> &'static str {
    use embassy_sync::once_lock::OnceLock;
    use heapless::String;

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
