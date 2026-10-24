use core::cell::{Cell, RefCell};
use core::convert::Infallible;

use bt_hci::cmd::{self, AsyncCmd, Cmd, CmdReturnBuf, SyncCmd};
use bt_hci::controller::{Controller, ControllerCmdAsync, ControllerCmdSync};
use bt_hci::data::{AclPacket, IsoPacket, SyncPacket};
use bt_hci::event::{CommandCompleteWithStatus, EventPacket};
use bt_hci::param::{RemainingBytes, Status};
use bt_hci::{ControllerToHostPacket, FromHciBytes, WriteHci};
use embassy_futures::select::{Either3, select3};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;
#[cfg(feature = "_ext_adv")]
use embassy_time::MockDriver;
use embassy_time::{Duration, Timer, with_timeout};
use trouble_host::prelude::*;

use super::profile::BleProfileAction;
use crate::channel::BLE_PROFILE_CHANNEL;
use crate::config::{BleBatteryConfig, DeviceConfig, StorageConfig};
use crate::core_traits::Runnable;
#[cfg(feature = "_ext_adv")]
use crate::event::{Axis, AxisEvent, AxisValType, PointingEvent, publish_event};
use crate::storage::{Storage, StorageKey, StorageValue, async_flash_wrapper, read};
use crate::test_support::{InMemoryFlash, clear_flash_channel, test_block_on};

struct Radio {
    events: Channel<NoopRawMutex, Vec<u8>, 8>,
    address: Cell<[u8; 6]>,
    extended: Cell<bool>,
    advertisements: RefCell<Vec<[u8; 6]>>,
    disconnects: RefCell<Vec<u16>>,
    changed: Signal<NoopRawMutex, ()>,
}

impl Radio {
    #[cfg(feature = "_ext_adv")]
    async fn wait_for_disconnects(&self, expected: &[u16]) {
        with_timeout(Duration::from_secs(1), async {
            while self.disconnects.borrow().len() < expected.len() {
                self.changed.wait().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(self.disconnects.borrow().as_slice(), expected);
    }

    async fn wait_for_advertisements(&self, count: usize) {
        with_timeout(Duration::from_secs(1), async {
            while self.advertisements.borrow().len() < count {
                self.changed.wait().await;
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!(
                "expected {count} advertisements, got {}",
                self.advertisements.borrow().len()
            )
        });
    }

    async fn connect(&self, handle: u8) {
        // LE Enhanced Connection Complete, then Advertising Set Terminated.
        let mut packet = vec![0x3e, 31, 0x0a, 0, handle, 0, 1, 1];
        packet.extend_from_slice(&[handle, 2, 3, 4, 5, 0xc1]);
        packet.extend_from_slice(&[0; 12]);
        packet.extend_from_slice(&[24, 0, 0, 0, 200, 0, 0]);
        self.events.send(packet).await;
        if self.extended.get() {
            self.events.send(vec![0x3e, 6, 0x12, 0, 0, handle, 0, 1]).await;
        }
        Timer::after_millis(2).await;
    }

    async fn disconnect(&self, handle: u8) {
        self.events.send(vec![0x05, 4, 0, handle, 0, 0x13]).await;
    }
}

struct MockController<'a>(&'a Radio);

impl embedded_io_async::ErrorType for MockController<'_> {
    type Error = Infallible;
}

impl Controller for MockController<'_> {
    type Buffer<'a> = [u8; 64];

    fn alloc_buf(&self) -> Result<Self::Buffer<'_>, Self::Error> {
        Ok([0; 64])
    }

    async fn write_acl_data(&self, _: &AclPacket<'_>) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn write_sync_data(&self, _: &SyncPacket<'_>) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn write_iso_data(&self, _: &IsoPacket<'_>) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn read<'a>(&self, buf: &'a mut Self::Buffer<'_>) -> Result<ControllerToHostPacket<'a>, Self::Error> {
        let event = self.0.events.receive().await;
        buf[..event.len()].copy_from_slice(&event);
        if event[0] == 0x02 {
            return Ok(ControllerToHostPacket::Acl(
                AclPacket::from_hci_bytes_complete(&buf[1..event.len()]).unwrap(),
            ));
        }
        Ok(ControllerToHostPacket::Event(
            EventPacket::from_hci_bytes_complete(&buf[..event.len()]).unwrap(),
        ))
    }
}

impl<C: SyncCmd> ControllerCmdSync<C> for MockController<'_> {
    async fn exec(&self, command: &C) -> Result<C::Return, cmd::Error<Self::Error>> {
        let mut params = [0; 255];
        command.params().write_hci(&mut params[..]).unwrap();
        let mut result = C::ReturnBuf::new();
        match C::OPCODE {
            cmd::le::LeReadBufferSize::OPCODE => result.as_mut().copy_from_slice(&[251, 0, 32]),
            cmd::le::LeReadFilterAcceptListSize::OPCODE => result.as_mut()[0] = 8,
            cmd::le::LeReadNumberOfSupportedAdvSets::OPCODE => result.as_mut()[0] = 1,
            cmd::le::LeSetRandomAddr::OPCODE => self.0.address.set(params[..6].try_into().unwrap()),
            cmd::le::LeSetAdvSetRandomAddr::OPCODE => {
                self.0.extended.set(true);
                self.0.address.set(params[1..7].try_into().unwrap());
            }
            cmd::le::LeSetAdvEnable::OPCODE if params[0] != 0 => {
                self.0.advertisements.borrow_mut().push(self.0.address.get());
                self.0.changed.signal(());
            }
            cmd::le::LeSetExtAdvEnable::OPCODE if params[0] != 0 => {
                self.0.advertisements.borrow_mut().push(self.0.address.get());
                self.0.changed.signal(());
            }
            _ => {}
        }
        CommandCompleteWithStatus {
            num_hci_cmd_pkts: 1,
            status: Status::SUCCESS,
            cmd_opcode: C::OPCODE,
            return_param_bytes: RemainingBytes::from_hci_bytes_complete(result.as_ref()).unwrap(),
        }
        .to_result::<C>()
        .map_err(cmd::Error::Hci)
    }
}

impl<C: AsyncCmd> ControllerCmdAsync<C> for MockController<'_> {
    async fn exec(&self, command: &C) -> Result<(), cmd::Error<Self::Error>> {
        if C::OPCODE == cmd::link_control::Disconnect::OPCODE {
            let mut params = [0; 255];
            command.params().write_hci(&mut params[..]).unwrap();
            self.0
                .disconnects
                .borrow_mut()
                .push(u16::from_le_bytes([params[0], params[1]]));
            self.0.changed.signal(());
        }
        Ok(())
    }
}

#[test]
fn workers_wait_for_disconnection_before_releasing_the_profile() {
    test_block_on(async {
        clear_flash_channel();
        BLE_PROFILE_CHANNEL.clear();
        super::rng::seed([7; 32]);
        let radio = Radio {
            events: Channel::new(),
            address: Cell::new([0; 6]),
            extended: Cell::new(false),
            advertisements: RefCell::new(Vec::new()),
            disconnects: RefCell::new(Vec::new()),
            changed: Signal::new(),
        };
        let mut resources = HostResources::<DefaultPacketPool, 4, 16, 1, 4>::new();
        let base = [1, 2, 3, 4, 5, 0xc6];
        let stack = trouble_host::new(MockController(&radio), &mut resources)
            .set_random_address(Address::random(base))
            .build();
        let flash = async_flash_wrapper(InMemoryFlash::<16_384, 4_096, 1>::new());
        let mut storage = Box::pin(Storage::new(flash, &StorageConfig::default())).await;
        let device = DeviceConfig {
            vid: 1,
            pid: 1,
            manufacturer: "RMK",
            product_name: "RMK",
            serial_number: heapless::String::new(),
        };
        let battery = BleBatteryConfig::disabled();
        let run = super::run_ble_keyboard(
            &stack,
            base,
            &device,
            &battery,
            #[cfg(feature = "host")]
            None,
        );
        let scenario = async {
            radio.wait_for_advertisements(1).await;
            radio.connect(0).await;
            assert_eq!(stack.connections().count(), 1);
            BLE_PROFILE_CHANNEL.send(BleProfileAction::ClearSlot(0)).await;
            with_timeout(Duration::from_secs(1), async {
                while radio.disconnects.borrow().is_empty() {
                    radio.changed.wait().await;
                }
            })
            .await
            .unwrap();
            assert_eq!(*radio.disconnects.borrow(), [0]);
            assert_eq!(radio.advertisements.borrow().len(), 1);
            assert!(read(StorageKey::BondInfo(0)).await.unwrap().is_none());
            radio.disconnect(0).await;
            radio.wait_for_advertisements(2).await;
            let Some(StorageValue::BondInfo(info)) = read(StorageKey::BondInfo(0)).await.unwrap() else {
                panic!("clear must be saved after the worker stops");
            };
            assert!(info.removed);
            radio.connect(1).await;
            radio.disconnect(1).await;
            radio.wait_for_advertisements(3).await;
            assert_eq!(stack.connections().count(), 0);
        };
        assert!(matches!(
            select3(run, storage.run(), scenario).await,
            Either3::Third(())
        ));
    });
}

#[cfg(feature = "_ext_adv")]
#[test]
fn runtime_keeps_background_links_and_waits_for_disconnection_before_clearing() {
    if super::HOST_LINKS < 3 {
        return;
    }
    test_block_on(async {
        clear_flash_channel();
        BLE_PROFILE_CHANNEL.clear();
        super::rng::seed([7; 32]);
        let radio = Radio {
            events: Channel::new(),
            address: Cell::new([0; 6]),
            extended: Cell::new(false),
            advertisements: RefCell::new(Vec::new()),
            disconnects: RefCell::new(Vec::new()),
            changed: Signal::new(),
        };
        let mut resources = HostResources::<DefaultPacketPool, 4, 16, 1, 4>::new();
        let base = [1, 2, 3, 4, 5, 0xc6];
        let stack = trouble_host::new(MockController(&radio), &mut resources)
            .set_random_address(Address::random(base))
            .build();
        let flash = async_flash_wrapper(InMemoryFlash::<16_384, 4_096, 1>::new());
        let mut storage = Box::pin(Storage::new(flash, &StorageConfig::default())).await;
        let device = DeviceConfig {
            vid: 1,
            pid: 1,
            manufacturer: "RMK",
            product_name: "RMK",
            serial_number: heapless::String::new(),
        };
        let battery = BleBatteryConfig::disabled();
        let run = super::run_ble_keyboard(
            &stack,
            base,
            &device,
            &battery,
            #[cfg(feature = "host")]
            None,
        );
        let scenario = async {
            for profile in 0..3 {
                if profile != 0 {
                    BLE_PROFILE_CHANNEL.send(BleProfileAction::Switch(profile)).await;
                }
                radio.wait_for_advertisements(profile as usize + 1).await;
                assert_eq!(radio.address.get(), [1, 2, 3, 4 + profile, 5, 0xc6]);
                radio.connect(profile).await;
                assert_eq!(stack.connections().count(), profile as usize + 1);
            }
            BLE_PROFILE_CHANNEL.send(BleProfileAction::Switch(0)).await;
            Timer::after_millis(2).await;
            assert_eq!(radio.advertisements.borrow().len(), 3);
            assert!(radio.disconnects.borrow().is_empty());
            radio.disconnect(2).await;
            Timer::after_millis(2).await;
            assert_eq!(stack.connections().count(), 2);
            assert_eq!(radio.advertisements.borrow().len(), 3);

            radio.disconnect(0).await;
            radio.wait_for_advertisements(4).await;
            assert_eq!(radio.address.get(), base);
            MockDriver::get().advance(Duration::from_secs(301));
            Timer::after_millis(2).await;
            assert_eq!(stack.connections().count(), 1);
            assert_eq!(radio.advertisements.borrow().len(), 4);

            radio.disconnect(1).await;
            // Queue the completion before publishing activity, so completion wins the select.
            Timer::after_micros(1).await;
            publish_event(PointingEvent {
                device_id: 0,
                axes: [
                    AxisEvent {
                        typ: AxisValType::Rel,
                        axis: Axis::X,
                        value: 1,
                    },
                    AxisEvent {
                        typ: AxisValType::Rel,
                        axis: Axis::Y,
                        value: 0,
                    },
                    AxisEvent {
                        typ: AxisValType::Rel,
                        axis: Axis::Z,
                        value: 0,
                    },
                ],
            });
            radio.wait_for_advertisements(5).await;
            Timer::after_millis(2).await;
            assert!(
                radio.advertisements.borrow()[4..]
                    .iter()
                    .all(|address| *address == base)
            );
            radio.connect(3).await;
            let count = radio.advertisements.borrow().len();
            BLE_PROFILE_CHANNEL.send(BleProfileAction::ClearSlot(0)).await;
            while radio.disconnects.borrow().is_empty() {
                radio.changed.wait().await;
            }
            assert_eq!(*radio.disconnects.borrow(), [3]);
            assert_eq!(radio.advertisements.borrow().len(), count);
            assert!(read(StorageKey::BondInfo(0)).await.unwrap().is_none());
            radio.disconnect(3).await;
            radio.wait_for_advertisements(count + 1).await;
            let Some(StorageValue::BondInfo(info)) = read(StorageKey::BondInfo(0)).await.unwrap() else {
                panic!("the cleared profile must be persisted");
            };
            assert!(info.removed);
        };
        assert!(matches!(
            select3(run, storage.run(), scenario).await,
            Either3::Third(())
        ));
    });
}

#[cfg(feature = "_ext_adv")]
#[test]
fn runtime_cancels_pairing_on_switch_and_timeout_and_refuses_bond_loss() {
    use super::profile::ProfileInfo;
    use crate::storage::{StorageItem, store};

    test_block_on(async {
        clear_flash_channel();
        BLE_PROFILE_CHANNEL.clear();
        super::rng::seed([7; 32]);
        let radio = Radio {
            events: Channel::new(),
            address: Cell::new([0; 6]),
            extended: Cell::new(false),
            advertisements: RefCell::new(Vec::new()),
            disconnects: RefCell::new(Vec::new()),
            changed: Signal::new(),
        };
        let mut resources = HostResources::<DefaultPacketPool, 4, 16, 1, 4>::new();
        let base = [1, 2, 3, 4, 5, 0xc6];
        let stack = trouble_host::new(MockController(&radio), &mut resources)
            .set_random_address(Address::random(base))
            .build();
        let flash = async_flash_wrapper(InMemoryFlash::<16_384, 4_096, 1>::new());
        let mut storage = Box::pin(Storage::new(flash, &StorageConfig::default())).await;
        let device = DeviceConfig {
            vid: 1,
            pid: 1,
            manufacturer: "RMK",
            product_name: "RMK",
            serial_number: heapless::String::new(),
        };
        let ready = Signal::<NoopRawMutex, ()>::new();
        let run = async {
            ready.wait().await;
            super::run_ble_keyboard(
                &stack,
                base,
                &device,
                &BleBatteryConfig::disabled(),
                #[cfg(feature = "host")]
                None,
            )
            .await;
        };
        let scenario = async {
            let mut old = ProfileInfo::default();
            old.info.identity.addr = Address::random([0, 2, 3, 4, 5, 0xc1]);
            old.info.is_bonded = true;
            store(StorageItem::BondInfo(old.clone())).await.unwrap();
            #[cfg(feature = "dongle")]
            {
                let mut dongle = old.clone();
                dongle.slot_num = super::profile::DONGLE_PROFILE;
                dongle.info.identity.addr = Address::random([3, 2, 3, 4, 5, 0xc1]);
                store(StorageItem::BondInfo(dongle)).await.unwrap();
            }
            ready.signal(());
            radio.wait_for_advertisements(1).await;
            radio.connect(0).await;
            assert!(!stack.connections().next().unwrap().bondable().unwrap());

            BLE_PROFILE_CHANNEL.send(BleProfileAction::Pair(0)).await;
            radio.wait_for_disconnects(&[0]).await;
            assert!(stack.with_bond_information(|bonds| bonds[0] == old.info));
            assert_eq!(radio.advertisements.borrow().len(), 1);
            radio.disconnect(0).await;
            radio.wait_for_advertisements(2).await;
            assert!(stack.with_bond_information(|bonds| bonds.is_empty()));
            radio.connect(1).await;
            assert!(stack.connections().next().unwrap().bondable().unwrap());

            // Releasing the held key must not cancel pairing on the same profile.
            BLE_PROFILE_CHANNEL.send(BleProfileAction::Switch(0)).await;
            Timer::after_millis(2).await;
            assert_eq!(radio.disconnects.borrow().as_slice(), &[0]);
            BLE_PROFILE_CHANNEL.send(BleProfileAction::Switch(1)).await;
            radio.wait_for_disconnects(&[0, 1]).await;
            radio.disconnect(1).await;
            radio.wait_for_advertisements(3).await;
            BLE_PROFILE_CHANNEL.send(BleProfileAction::Switch(0)).await;
            radio.wait_for_advertisements(4).await;
            assert!(stack.with_bond_information(|bonds| bonds[0] == old.info));

            BLE_PROFILE_CHANNEL.send(BleProfileAction::Pair(0)).await;
            radio.wait_for_advertisements(5).await;
            radio.connect(2).await;
            MockDriver::get().advance(Duration::from_secs(301));
            radio.wait_for_disconnects(&[0, 1, 2]).await;
            radio.disconnect(2).await;
            Timer::after_millis(2).await;
            assert!(stack.with_bond_information(|bonds| bonds[0] == old.info));
            assert_eq!(radio.advertisements.borrow().len(), 5);
            let Some(StorageValue::BondInfo(stored)) = read(StorageKey::BondInfo(0)).await.unwrap() else {
                panic!("cancelled pairing must keep the old bond");
            };
            assert_eq!(stored.info, old.info);

            BLE_PROFILE_CHANNEL.send(BleProfileAction::Switch(1)).await;
            radio.wait_for_advertisements(6).await;
            BLE_PROFILE_CHANNEL.send(BleProfileAction::Switch(0)).await;
            radio.wait_for_advertisements(7).await;
            radio.connect(0).await;
            // An SMP Pairing Request from a bonded peer reports BondLost.
            radio
                .events
                .send(vec![0x02, 0, 0x20, 11, 0, 7, 0, 6, 0, 1, 3, 0, 9, 16, 3, 3])
                .await;
            radio.wait_for_disconnects(&[0, 1, 2, 0]).await;
            assert_eq!(radio.advertisements.borrow().len(), 7);
            radio.disconnect(0).await;
            radio.wait_for_advertisements(8).await;

            // A different peer cannot replace a bonded profile without Pair.
            radio.connect(3).await;
            radio.wait_for_disconnects(&[0, 1, 2, 0, 3]).await;
            radio.disconnect(3).await;
            radio.wait_for_advertisements(9).await;
            #[cfg(feature = "dongle")]
            {
                // The bonded dongle cannot enter even an empty host profile.
                BLE_PROFILE_CHANNEL.send(BleProfileAction::Switch(1)).await;
                radio.wait_for_advertisements(10).await;
                radio.connect(3).await;
                radio.wait_for_disconnects(&[0, 1, 2, 0, 3, 3]).await;
                radio.disconnect(3).await;
                radio.wait_for_advertisements(11).await;
            }
        };
        assert!(matches!(
            select3(run, storage.run(), scenario).await,
            Either3::Third(())
        ));
    });
}

#[cfg(feature = "_ext_adv")]
#[test]
fn runtime_background_policy_preserves_active_and_waits_for_draining_slots() {
    if super::HOST_LINKS < 3 {
        return;
    }
    test_block_on(async {
        clear_flash_channel();
        BLE_PROFILE_CHANNEL.clear();
        super::rng::seed([7; 32]);
        let radio = Radio {
            events: Channel::new(),
            address: Cell::new([0; 6]),
            extended: Cell::new(false),
            advertisements: RefCell::new(Vec::new()),
            disconnects: RefCell::new(Vec::new()),
            changed: Signal::new(),
        };
        let mut resources = HostResources::<DefaultPacketPool, 4, 16, 1, 4>::new();
        let base = [1, 2, 3, 4, 5, 0xc6];
        let stack = trouble_host::new(MockController(&radio), &mut resources)
            .set_random_address(Address::random(base))
            .build();
        let flash = async_flash_wrapper(InMemoryFlash::<16_384, 4_096, 1>::new());
        let mut storage = Box::pin(Storage::new(flash, &StorageConfig::default())).await;
        let device = DeviceConfig {
            vid: 1,
            pid: 1,
            manufacturer: "RMK",
            product_name: "RMK",
            serial_number: heapless::String::new(),
        };
        let battery = BleBatteryConfig::disabled();
        let run = super::run_ble_keyboard(
            &stack,
            base,
            &device,
            &battery,
            #[cfg(feature = "host")]
            None,
        );
        let scenario = async {
            radio.wait_for_advertisements(1).await;
            let count = 1;
            for profile in 0..3 {
                BLE_PROFILE_CHANNEL.send(BleProfileAction::Switch(profile)).await;
                radio.wait_for_advertisements(count + profile as usize).await;
                radio.connect(profile).await;
            }
            assert_eq!(stack.connections().count(), 3);
            BLE_PROFILE_CHANNEL
                .send(BleProfileAction::SetBackgroundConnections(false))
                .await;
            radio.wait_for_disconnects(&[0, 1]).await;
            assert_eq!(crate::state::current_profile(), 2);
            assert!(matches!(
                read(StorageKey::BleBackgroundConnections).await.unwrap(),
                Some(StorageValue::BleBackgroundConnections(false))
            ));
            radio.disconnect(0).await;
            radio.disconnect(1).await;
            Timer::after_millis(2).await;
            assert_eq!(stack.connections().count(), 1);

            BLE_PROFILE_CHANNEL
                .send(BleProfileAction::SetBackgroundConnections(true))
                .await;
            Timer::after_millis(2).await;
            assert_eq!(radio.advertisements.borrow().len(), count + 2);
            assert_eq!(radio.disconnects.borrow().as_slice(), &[0, 1]);
            BLE_PROFILE_CHANNEL.send(BleProfileAction::Switch(0)).await;
            radio.wait_for_advertisements(count + 3).await;
            radio.connect(0).await;
            assert_eq!(stack.connections().count(), 2);

            BLE_PROFILE_CHANNEL
                .send(BleProfileAction::SetBackgroundConnections(false))
                .await;
            radio.wait_for_disconnects(&[0, 1, 2]).await;
            BLE_PROFILE_CHANNEL.send(BleProfileAction::Switch(1)).await;
            radio.wait_for_disconnects(&[0, 1, 2, 0]).await;
            // Single-link mode must wait for every previous slot to be reclaimed.
            assert_eq!(radio.advertisements.borrow().len(), count + 3);
            radio.disconnect(2).await;
            Timer::after_millis(2).await;
            assert_eq!(radio.advertisements.borrow().len(), count + 3);
            radio.disconnect(0).await;
            radio.wait_for_advertisements(count + 4).await;
            assert_eq!(radio.address.get(), [1, 2, 3, 5, 5, 0xc6]);
        };
        assert!(matches!(
            select3(run, storage.run(), scenario).await,
            Either3::Third(())
        ));
    });
}
