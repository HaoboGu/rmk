#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_nrf::gpio::{AnyPin, Input, Level, Output, OutputDrive, Pull};
use embassy_time::Timer;
use rmk::wireless::{GazellConfig, GazellTransport, WirelessTransport};
use {defmt_rtt as _, panic_probe as _};

/// Simple test keyboard that sends periodic test packets via Gazell
///
/// This is a minimal example to demonstrate Gazell device mode (transmitter).
/// A real keyboard implementation would:
/// - Scan a key matrix
/// - Encode key states using Elink protocol
/// - Send keyboard reports to the dongle
///
/// For now, this just sends test packets to verify the wireless link works.

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("RMK nRF52840 2.4G Keyboard starting...");

    // Initialize nRF52840 peripherals
    let p = embassy_nrf::init(Default::default());

    // Optional: Setup an LED indicator (P0.13 on most nRF52840 dev boards)
    // Uncomment if your board has an LED
    // let mut led = Output::new(p.P0_13.degrade(), Level::Low, OutputDrive::Standard);

    // Initialize Gazell in device mode (transmitter)
    info!("Initializing Gazell 2.4G wireless...");

    let config = GazellConfig::low_latency();
    let mut gazell = GazellTransport::new(config);

    match gazell.init() {
        Ok(()) => info!("Gazell initialized successfully"),
        Err(e) => {
            error!("Gazell init failed: {:?}", e);
            panic!("Cannot start without Gazell");
        }
    }

    match gazell.set_device_mode() {
        Ok(()) => info!("Gazell set to device mode (transmitter)"),
        Err(e) => {
            error!("Failed to set device mode: {:?}", e);
            panic!("Cannot start without device mode");
        }
    }

    info!("Keyboard ready! Starting test transmission...");
    info!("Connect the dongle to see received packets");

    // Test packet counter
    let mut counter: u8 = 0;

    // Main loop: Send test packets at 10Hz
    loop {
        // Create a simple test packet
        // Format: [0xAA (magic byte), 0xBB (test ID), counter]
        let test_packet = [0xAA, 0xBB, counter];

        // Send packet
        match gazell.send_frame(&test_packet) {
            Ok(()) => {
                info!("Sent test packet #{} successfully", counter);
                // Toggle LED on success if available
                // led.toggle();
            }
            Err(e) => {
                warn!("Send failed (packet #{}): {:?}", counter, e);
                // This might happen if:
                // - Dongle is not powered on
                // - Out of range
                // - Channel interference
            }
        }

        // Increment counter (wraps at 255)
        counter = counter.wrapping_add(1);

        // Wait 100ms before next transmission (10Hz rate)
        Timer::after_millis(100).await;
    }
}

// TODO: Real keyboard implementation would:
//
// 1. Initialize key matrix:
//    let matrix = KeyMatrix::new(/* row pins */, /* col pins */);
//
// 2. In main loop, scan keys:
//    let key_states = matrix.scan().await;
//
// 3. Encode using Elink protocol:
//    let frame = encode_keyboard_report(&key_states);
//
// 4. Send via Gazell:
//    gazell.send_frame(&frame)?;
//
// 5. Add battery monitoring, low-power modes, etc.
