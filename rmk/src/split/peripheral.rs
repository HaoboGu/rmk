#[cfg(feature = "_ble")]
use bt_hci::{cmd::le::LeSetPhy, controller::ControllerCmdAsync};
use embassy_futures::select::{Either, select};
#[cfg(not(feature = "_ble"))]
use embedded_io_async::{Read, Write};
use futures::FutureExt;
#[cfg(all(feature = "_ble", feature = "storage"))]
use {super::ble::PeerAddress, crate::channel::FLASH_CHANNEL};
#[cfg(feature = "_ble")]
use {
    crate::event::{BatteryStateEvent, ChargingStateEvent, EventSubscriber},
    crate::storage::Storage,
    embedded_storage_async::nor_flash::NorFlash,
    trouble_host::prelude::*,
};

use super::SplitMessage;
use super::driver::{SplitReader, SplitWriter};
use crate::CONNECTION_STATE;
use crate::event::{KeyboardEvent, PointingEvent, SubscribableEvent, TouchpadEvent};

use crate::event::{LayerChangeEvent, LedIndicatorEvent, publish_event};
#[cfg(not(feature = "_ble"))]
use crate::split::serial::SerialSplitDriver;
use crate::state::ConnectionState;

/// Run the split peripheral service.
///
/// # Arguments
///
/// * `id` - (optional) The id of the peripheral
/// * `stack` - (optional) The TrouBLE stack
/// * `serial` - (optional) serial port used to send peripheral split message. This argument is enabled only for serial split now
/// * `storage` - (optional) The storage to save the central address
pub async fn run_rmk_split_peripheral<
    'a,
    #[cfg(feature = "_ble")] C: Controller + ControllerCmdAsync<LeSetPhy>,
    #[cfg(not(feature = "_ble"))] S: Write + Read,
    #[cfg(feature = "_ble")] F: NorFlash,
    #[cfg(feature = "_ble")] const ROW: usize,
    #[cfg(feature = "_ble")] const COL: usize,
    #[cfg(feature = "_ble")] const NUM_LAYER: usize,
    #[cfg(feature = "_ble")] const NUM_ENCODER: usize,
>(
    #[cfg(feature = "_ble")] id: usize,
    #[cfg(feature = "_ble")] stack: &'a Stack<'a, C, DefaultPacketPool>,
    #[cfg(feature = "_ble")] storage: &mut Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>,
    #[cfg(not(feature = "_ble"))] serial: S,
) {
    #[cfg(not(feature = "_ble"))]
    {
        let mut peripheral = SplitPeripheral::new(SerialSplitDriver::new(serial));
        loop {
            peripheral.run().await;
        }
    }

    #[cfg(feature = "_ble")]
    crate::split::ble::peripheral::initialize_nrf_ble_split_peripheral_and_run(id, stack, storage).await;
}

/// The split peripheral instance.
pub(crate) struct SplitPeripheral<S: SplitWriter + SplitReader> {
    split_driver: S,
}

impl<S: SplitWriter + SplitReader> SplitPeripheral<S> {
    pub(crate) fn new(split_driver: S) -> Self {
        Self { split_driver }
    }

    /// Run the peripheral keyboard service.
    ///
    /// The peripheral uses the general matrix, does scanning and send the key events through `SplitWriter`.
    /// If also receives split messages from the central through `SplitReader`.
    pub(crate) async fn run(&mut self) {
        CONNECTION_STATE.store(ConnectionState::Connected.into(), core::sync::atomic::Ordering::Release);

        let key_sub = KeyboardEvent::subscriber();
        #[cfg(feature = "_ble")]
        let charging_state_sub = ChargingStateEvent::subscriber();
        let touch_sub = TouchpadEvent::subscriber();
        let pointing_sub = PointingEvent::subscriber();
        #[cfg(feature = "_ble")]
        let mut battery_sub = BatteryStateEvent::subscriber();

        loop {
            let read_message_to_send = async {
                let message = crate::select_biased_with_feature! {
                    e = key_sub.receive().fuse() => SplitMessage::Key(e),
                    with_feature("_ble"): e = charging_state_sub.receive().fuse() => {
                        if e.charging {
                            SplitMessage::BatteryState(BatteryStateEvent::Charging)
                        } else {
                            SplitMessage::BatteryState(BatteryStateEvent::NotAvailable)
                        }
                    },
                    e = touch_sub.receive().fuse() => SplitMessage::Touchpad(e),
                    e = pointing_sub.receive().fuse() => SplitMessage::Pointing(e),
                    with_feature("_ble"): e = battery_sub.next_event().fuse() => SplitMessage::BatteryState(e),
                };
                message
            };

            match select(self.split_driver.read(), read_message_to_send).await {
                Either::First(m) => match m {
                    // Process split messages from the central
                    // Currently only handle the central state message
                    Ok(split_message) => match split_message {
                        SplitMessage::ConnectionState(state) => {
                            trace!("Received connection state update: {}", state);
                            CONNECTION_STATE.store(state, core::sync::atomic::Ordering::Release);
                        }
                        #[cfg(all(feature = "_ble", feature = "storage"))]
                        SplitMessage::ClearPeer => {
                            // Clear the peer address
                            FLASH_CHANNEL
                                .send(crate::storage::FlashOperationMessage::PeerAddress(PeerAddress::new(
                                    0, false, [0; 6],
                                )))
                                .await;
                        }
                        SplitMessage::KeyboardIndicator(indicator) => {
                            // Publish KeyboardIndicator event
                            publish_event(LedIndicatorEvent {
                                indicator: rmk_types::led_indicator::LedIndicator::from_bits(indicator),
                            });
                        }
                        SplitMessage::Layer(layer) => {
                            // Publish Layer event
                            publish_event(LayerChangeEvent { layer });
                        }
                        _ => (),
                    },
                    Err(e) => {
                        error!("Split message read error: {:?}", e);
                        if let crate::split::driver::SplitDriverError::Disconnected = e {
                            break;
                        }
                    }
                },
                Either::Second(e) => {
                    // Only send the key event if the connection is established
                    if CONNECTION_STATE.load(core::sync::atomic::Ordering::Acquire) {
                        debug!("Writing split message {:?} to central", e);
                        self.split_driver.write(&e).await.ok();
                    } else {
                        debug!("Connection not established, skipping key event");
                    }
                }
            }
        }
    }
}
