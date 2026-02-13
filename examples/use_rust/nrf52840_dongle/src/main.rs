#![no_std]
#![no_main]

mod usb_hid;

use defmt::*;
use embassy_executor::Spawner;
use embassy_nrf::{bind_interrupts, peripherals, usb};
use embassy_time::Timer;
use embassy_usb::class::hid::{HidWriter, State};
use embassy_usb::{Builder, Config, UsbDevice};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

use crate::usb_hid::UsbHidKeyboard;

// Import Gazell wireless support
use rmk::wireless::{GazellConfig, GazellTransport, WirelessTransport};

bind_interrupts!(struct Irqs {
    USBD => usb::InterruptHandler<peripherals::USBD>;
    POWER_CLOCK => usb::vbus_detect::InterruptHandler;
});

// USB HID Report Descriptor for keyboard
const KEYBOARD_REPORT_DESC: &[u8] = &[
    0x05, 0x01, // Usage Page (Generic Desktop)
    0x09, 0x06, // Usage (Keyboard)
    0xA1, 0x01, // Collection (Application)
    0x05, 0x07, //   Usage Page (Key Codes)
    0x19, 0xE0, //   Usage Minimum (224)
    0x29, 0xE7, //   Usage Maximum (231)
    0x15, 0x00, //   Logical Minimum (0)
    0x25, 0x01, //   Logical Maximum (1)
    0x75, 0x01, //   Report Size (1)
    0x95, 0x08, //   Report Count (8)
    0x81, 0x02, //   Input (Data, Variable, Absolute)
    0x95, 0x01, //   Report Count (1)
    0x75, 0x08, //   Report Size (8)
    0x81, 0x01, //   Input (Constant)
    0x95, 0x06, //   Report Count (6)
    0x75, 0x08, //   Report Size (8)
    0x15, 0x00, //   Logical Minimum (0)
    0x25, 0x65, //   Logical Maximum (101)
    0x05, 0x07, //   Usage Page (Key Codes)
    0x19, 0x00, //   Usage Minimum (0)
    0x29, 0x65, //   Usage Maximum (101)
    0x81, 0x00, //   Input (Data, Array)
    0xC0,       // End Collection
];

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("RMK nRF52840 Dongle starting...");

    // Initialize nRF52840 peripherals
    let p = embassy_nrf::init(Default::default());

    // Create USB driver
    let driver = usb::Driver::new(p.USBD, Irqs, usb::vbus_detect::HardwareVbusDetect::new(Irqs));

    // Configure USB device
    let mut config = Config::new(0x1209, 0x0001); // TODO: Use proper VID/PID
    config.manufacturer = Some("RMK");
    config.product = Some("RMK Dongle");
    config.serial_number = Some("12345678");
    config.max_power = 100; // 100mA

    // Create USB device builder
    static DEVICE_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
    static HID_STATE: StaticCell<State> = StaticCell::new();

    let mut builder = Builder::new(
        driver,
        config,
        DEVICE_DESC.init([0; 256]),
        CONFIG_DESC.init([0; 256]),
        BOS_DESC.init([0; 256]),
        CONTROL_BUF.init([0; 64]),
    );

    // Create HID class for keyboard
    let hid_config = embassy_usb::class::hid::Config {
        report_descriptor: KEYBOARD_REPORT_DESC,
        request_handler: None,
        poll_ms: 1, // 1ms polling interval
        max_packet_size: 8,
    };

    let hid = HidWriter::<_, 8>::new(&mut builder, HID_STATE.init(State::new()), hid_config);

    // Build USB device
    let mut usb = builder.build();

    // Create USB HID keyboard interface
    let mut keyboard = UsbHidKeyboard::new(hid);

    info!("USB initialized, waiting for host connection...");

    // Initialize 2.4G Gazell receiver (host mode)
    let config = GazellConfig::low_latency();
    let mut gazell = GazellTransport::new(config);

    match gazell.init() {
        Ok(()) => info!("Gazell initialized"),
        Err(e) => {
            error!("Gazell init failed: {:?}", e);
            panic!("Cannot start without Gazell");
        }
    }

    match gazell.set_host_mode() {
        Ok(()) => info!("Gazell set to host mode (receiver)"),
        Err(e) => {
            error!("Failed to set host mode: {:?}", e);
            panic!("Cannot start without host mode");
        }
    }

    info!("Dongle ready! Listening for keyboard packets on 2.4GHz...");

    // Main loop: run USB device in background and handle 2.4G packets
    loop {
        // Run USB device (non-blocking)
        embassy_futures::select::select(usb.run(), async {
            // Poll for 2.4G packets at 1kHz (1ms interval)
            Timer::after_millis(1).await;

            // Receive packets from Gazell
            match gazell.recv_frame() {
                Ok(Some(packet)) => {
                    info!("Received 2.4G packet: {} bytes", packet.len());

                    // Parse Elink frame
                    if let Ok(frame) = elink_core::StandardFrame::parse(&packet) {
                        info!("Elink frame type: 0x{:02X}", frame.frame_type());

                        // TODO: Parse and forward to USB HID
                        // This requires Elink RMK adapter integration
                        // For now, just log the frame
                        //
                        // Example future implementation:
                        // if frame.frame_type() == elink_core::FRAME_TYPE_COMMAND {
                        //     if let Ok(msg) = postcard::from_bytes::<SplitMessage>(frame.data()) {
                        //         // Extract keyboard report
                        //         // keyboard.send_report(&report).await;
                        //     }
                        // }
                    } else {
                        warn!("Invalid Elink frame received");
                    }
                }
                Ok(None) => {
                    // No data available - this is normal
                }
                Err(e) => {
                    warn!("Receive error: {:?}", e);
                }
            }
        })
        .await;
    }
}
