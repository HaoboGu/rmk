#[cfg(feature = "_ble")]
use bt_hci::{cmd::le::LeSetPhy, controller::ControllerCmdAsync};
use embassy_futures::select::{Either4, select4};
#[cfg(not(feature = "_ble"))]
use embedded_io_async::{Read, Write};
#[cfg(all(feature = "_ble", feature = "storage"))]
use {super::ble::PeerAddress, crate::channel::FLASH_CHANNEL};
#[cfg(feature = "_ble")]
use {crate::storage::Storage, embedded_storage_async::nor_flash::NorFlash, trouble_host::prelude::*};

use super::SplitMessage;
use super::driver::{SplitReader, SplitWriter};
use crate::CONNECTION_STATE;
use crate::channel::{EVENT_CHANNEL, KEY_EVENT_CHANNEL};
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
        use crate::builtin_events::PowerEvent;
        use crate::event::{ControllerEventTrait, EventSubscriber};

        CONNECTION_STATE.store(ConnectionState::Connected.into(), core::sync::atomic::Ordering::Release);

        let mut power_sub = PowerEvent::subscriber();

        loop {
            match select4(
                self.split_driver.read(),
                KEY_EVENT_CHANNEL.receive(),
                EVENT_CHANNEL.receive(),
                power_sub.next_event(),
            )
            .await
            {
                Either4::First(m) => match m {
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
                            crate::event::publish_controller_event(
                                crate::builtin_events::KeyboardStateEvent::indicator(
                                    rmk_types::led_indicator::LedIndicator::from_bits(indicator),
                                ),
                            );
                        }
                        SplitMessage::Layer(layer) => {
                            // Publish Layer event
                            crate::event::publish_controller_event(crate::builtin_events::KeyboardStateEvent::layer(
                                layer,
                            ));
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
                Either4::Second(e) => {
                    // Only send the key event if the connection is established
                    if CONNECTION_STATE.load(core::sync::atomic::Ordering::Acquire) {
                        debug!("Writing split key event to central");
                        self.split_driver.write(&SplitMessage::Key(e)).await.ok();
                    } else {
                        debug!("Connection not established, skipping key event");
                    }
                }
                Either4::Third(e) => {
                    if CONNECTION_STATE.load(core::sync::atomic::Ordering::Acquire) {
                        debug!("Writing split event to central: {:?}", e);
                        self.split_driver.write(&SplitMessage::Event(e)).await.ok();
                    } else {
                        debug!("Connection not established, skipping event");
                    }
                }
                Either4::Fourth(power_event) => {
                    // Forward battery level to central
                    if let PowerEvent::Battery(level) = power_event {
                        if CONNECTION_STATE.load(core::sync::atomic::Ordering::Acquire) {
                            debug!("Forwarding battery level to central: {}", level);
                            self.split_driver.write(&SplitMessage::BatteryLevel(level)).await.ok();
                        }
                    }
                }
            }
        }
    }
}
