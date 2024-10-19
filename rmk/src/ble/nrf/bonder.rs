use super::BONDED_DEVICE_NUM;
use crate::{
    ble::nrf::ACTIVE_PROFILE,
    storage::{FlashOperationMessage, FLASH_CHANNEL},
};
use core::{cell::RefCell, sync::atomic::Ordering};
use defmt::{debug, error, info, warn, Format};
use heapless::FnvIndexMap;
use nrf_softdevice::ble::{
    gatt_server::{get_sys_attrs, set_sys_attrs},
    security::{IoCapabilities, SecurityHandler},
    Address, AddressType, Connection, EncryptionInfo, IdentityKey, IdentityResolutionKey, MasterId,
    SecurityMode,
};

// Bond info which will be stored in flash
#[derive(Clone, Copy, Debug, Format, Default)]
pub(crate) struct BondInfo {
    pub(crate) slot_num: u8,
    pub(crate) peer: Peer,
    sys_attr: SystemAttribute,
    pub(crate) removed: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Format)]
pub(crate) struct Peer {
    pub(crate) master_id: MasterId,
    pub(crate) key: EncryptionInfo,
    pub(crate) peer_id: IdentityKey,
}

impl Default for Peer {
    fn default() -> Self {
        Self {
            master_id: Default::default(),
            key: Default::default(),
            peer_id: IdentityKey {
                addr: Address::new(AddressType::Public, [0; 6]),
                irk: IdentityResolutionKey::default(),
            },
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Format)]
pub(crate) struct SystemAttribute {
    pub(crate) length: usize,
    pub(crate) data: [u8; 62],
}

impl Default for SystemAttribute {
    fn default() -> Self {
        Self {
            length: 0,
            data: [0; 62],
        }
    }
}

// Bonder that manages multiple profiles
pub(crate) struct MultiBonder {
    // Info of all bonded devices
    // `slot_num` is used as the key, because using peer as key will bring a lot more complexity
    bond_info: RefCell<FnvIndexMap<u8, BondInfo, BONDED_DEVICE_NUM>>,
}

impl MultiBonder {
    pub(crate) fn new(bond_info: RefCell<FnvIndexMap<u8, BondInfo, BONDED_DEVICE_NUM>>) -> Self {
        Self { bond_info }
    }

    pub(crate) fn get_bonded_peer(&self, profile: u8) -> Option<Peer> {
        if let Some(info) = self.bond_info.borrow().get(&profile) {
            if !info.removed {
                return Some(info.peer);
            }
        }
        None
    }

    pub(crate) fn check_connection(&self, conn: &Connection) -> bool {
        let addr = conn.peer_address();
        let current_profile = ACTIVE_PROFILE.load(Ordering::Acquire);
        let saved_slot_num_for_conn = self
            .bond_info
            .borrow()
            .iter()
            .find(|(_, info)| {
                info.peer.peer_id.is_match(addr)
                    && info.removed == false
            })
            .map(|(i, _)| *i);

        if let Some(slot_num) = saved_slot_num_for_conn {
            if slot_num != current_profile {
                return false;
            }
        }
        true
    }
}

impl SecurityHandler for MultiBonder {
    fn io_capabilities(&self) -> IoCapabilities {
        IoCapabilities::None
    }

    fn can_bond(&self, _conn: &Connection) -> bool {
        true
    }

    fn display_passkey(&self, passkey: &[u8; 6]) {
        info!("BLE passkey: {:#X}", passkey);
    }

    fn on_security_update(&self, _conn: &Connection, security_mode: SecurityMode) {
        info!("on_security_update, new security mode: {}", security_mode);
    }

    fn on_bonded(
        &self,
        conn: &Connection,
        master_id: MasterId,
        key: EncryptionInfo,
        peer_id: IdentityKey,
    ) {
        // First time
        debug!("On bonded: storing bond for {}", master_id);

        // Get slot num, if the device has been bonded, reuse the slot num. Otherwise get a new slot num
        let slot_num = ACTIVE_PROFILE.load(Ordering::Acquire);

        // Save bond info
        let mut sys_attr_data: [u8; 62] = [0; 62];
        let sys_attr_length = get_sys_attrs(conn, &mut sys_attr_data).unwrap();

        let new_bond_info = BondInfo {
            sys_attr: SystemAttribute {
                length: sys_attr_length,
                data: sys_attr_data,
            },
            peer: Peer {
                master_id,
                key,
                peer_id,
            },
            slot_num,
            removed: false,
        };

        match FLASH_CHANNEL.try_send(FlashOperationMessage::BondInfo(new_bond_info)) {
            Ok(_) => {
                // Update self.bond_info as well
                debug!("Sent bond info to flash channel");
                self.bond_info
                    .borrow_mut()
                    .insert(slot_num, new_bond_info)
                    .ok();
            }
            Err(_) => error!("Send bond info to flash channel error"),
        }
    }

    fn get_key(&self, _conn: &Connection, master_id: MasterId) -> Option<EncryptionInfo> {
        // Reconnecting with an existing bond
        debug!("Getting bond for {}", master_id);

        self.bond_info
            .borrow()
            .iter()
            .find(|(_, info)| {
                // Reconnect to device on actived slot
                let slot_num = ACTIVE_PROFILE.load(Ordering::Acquire);
                info.slot_num == slot_num
                    && info.peer.master_id == master_id
                    && info.removed == false
            })
            .and_then(|(_, d)| Some(d.peer.key))
    }

    fn save_sys_attrs(&self, conn: &Connection) {
        // On disconnect usually
        let addr = conn.peer_address();

        let mut bond_info = self.bond_info.borrow_mut();

        // Get bonded peer
        let bonded = bond_info
            .iter_mut()
            .filter(|(_, b)| b.removed == false)
            .find(|(_, info)| info.peer.peer_id.is_match(addr));

        if let Some((_, info)) = bonded {
            let mut buf = [0_u8; 64];

            match get_sys_attrs(conn, &mut buf) {
                Ok(sys_attr_len) => {
                    if sys_attr_len > 0 {
                        // Get sys_attrs correctly, check whether it's same with saved bond info.
                        // If not, update bond info
                        if !(info.sys_attr.length == sys_attr_len
                            && info.sys_attr.data[0..sys_attr_len] == buf[0..sys_attr_len])
                        {
                            debug!(
                                "Updating sys_attr:\nnew: {},{}\nold: {},{}",
                                buf, sys_attr_len, info.sys_attr.data, info.sys_attr.length
                            );
                            // Update bond info
                            info.sys_attr.data[0..sys_attr_len]
                                .copy_from_slice(&buf[0..sys_attr_len]);
                            info.sys_attr.length = sys_attr_len;

                            // Save new bond info to flash
                            match FLASH_CHANNEL
                                .try_send(FlashOperationMessage::BondInfo(info.clone()))
                            {
                                Ok(_) => debug!("Sent bond info to flash channel"),
                                Err(_e) => error!("Send bond info to flash channel error"),
                            };
                        }
                    } else {
                        error!("Got empty system attr");
                    }
                }
                Err(e) => {
                    error!("Get system attr for {} erro: {}", info, e);
                }
            }
        } else {
            info!("Peer doesn't match: {}", conn.peer_address());
        }
    }

    fn load_sys_attrs(&self, conn: &Connection) {
        let addr = conn.peer_address();
        info!("Loading system attributes for {}", addr);

        let bond_info = self.bond_info.borrow();

        let sys_attr = bond_info
            .iter()
            .filter(|(_, b)| b.sys_attr.length != 0 && b.removed == false)
            .find(|(_, b)| b.peer.peer_id.is_match(addr))
            .map(|(_, b)| &b.sys_attr.data[0..b.sys_attr.length]);

        // info!("call set_sys_attrs in load_sys_attrs: {}", sys_attr);
        if let Err(err) = set_sys_attrs(conn, sys_attr) {
            warn!("SecurityHandler failed to set sys attrs: {:?}", err);
        }
    }
}

// Bonder aka security handler used in advertising & pairing
pub(crate) struct Bonder {
    // Info of all bonded devices
    // `slot_num` is used as the key, because using peer as key will bring a lot more complexity
    bond_info: RefCell<FnvIndexMap<u8, BondInfo, BONDED_DEVICE_NUM>>,
}

impl Bonder {
    pub(crate) fn new(bond_info: RefCell<FnvIndexMap<u8, BondInfo, BONDED_DEVICE_NUM>>) -> Self {
        Self { bond_info }
    }
}

impl SecurityHandler for Bonder {
    fn io_capabilities(&self) -> IoCapabilities {
        IoCapabilities::None
    }

    fn can_bond(&self, _conn: &Connection) -> bool {
        true
    }

    fn display_passkey(&self, passkey: &[u8; 6]) {
        info!("BLE passkey: {:#X}", passkey);
    }

    fn on_security_update(&self, _conn: &Connection, security_mode: SecurityMode) {
        info!("on_security_update, new security mode: {}", security_mode);
    }

    fn on_bonded(
        &self,
        conn: &Connection,
        master_id: MasterId,
        key: EncryptionInfo,
        peer_id: IdentityKey,
    ) {
        // First time
        debug!("On bonded: storing bond for {}", master_id);

        // Get slot num, if the device has been bonded, reuse the slot num. Otherwise get a new slot num
        let slot_num = self
            .bond_info
            .borrow()
            .iter()
            .find(|(_, b)| b.peer.peer_id.addr == peer_id.addr && b.removed == false)
            .map(|(i, _)| *i)
            .unwrap_or(self.bond_info.borrow().len() as u8);

        // Check whether all slots are full, if so randomly remove one
        if (slot_num as usize) == self.bond_info.borrow().capacity() {
            warn!("Reach maximum number of bonded devices, a device which is not lucky today will be removed:(");
            // The unlucky number is 4
            let unlucky: u8 = 4;
            match FLASH_CHANNEL.try_send(FlashOperationMessage::ClearSlot(unlucky)) {
                Ok(_) => debug!("Sent clear to flash channel"),
                Err(_e) => error!("Send clear to flash channel error"),
            }
            self.bond_info.borrow_mut().remove(&unlucky);
        } else {
            // Save bond info
            let mut sys_attr_data: [u8; 62] = [0; 62];
            let sys_attr_length = get_sys_attrs(conn, &mut sys_attr_data).unwrap();

            let new_bond_info = BondInfo {
                sys_attr: SystemAttribute {
                    length: sys_attr_length,
                    data: sys_attr_data,
                },
                peer: Peer {
                    master_id,
                    key,
                    peer_id,
                },
                slot_num,
                removed: false,
            };

            match FLASH_CHANNEL.try_send(FlashOperationMessage::BondInfo(new_bond_info)) {
                Ok(_) => {
                    // Update self.bond_info as well
                    debug!("Sent bond info to flash channel");
                    self.bond_info
                        .borrow_mut()
                        .insert(slot_num, new_bond_info)
                        .ok();
                }
                Err(_) => error!("Send bond info to flash channel error"),
            }
        }
    }

    fn get_key(&self, _conn: &Connection, master_id: MasterId) -> Option<EncryptionInfo> {
        // Reconnecting with an existing bond
        debug!("Getting bond for {}", master_id);

        self.bond_info
            .borrow()
            .iter()
            .find(|(_, info)| info.peer.master_id == master_id && info.removed == false)
            .and_then(|(_, d)| Some(d.peer.key))
    }

    fn save_sys_attrs(&self, conn: &Connection) {
        // On disconnect usually
        let addr = conn.peer_address();

        let mut bond_info = self.bond_info.borrow_mut();

        // Get bonded peer
        let bonded = bond_info
            .iter_mut()
            .find(|(_, info)| info.peer.peer_id.is_match(addr));

        if let Some((_, info)) = bonded {
            let mut buf = [0_u8; 64];

            match get_sys_attrs(conn, &mut buf) {
                Ok(sys_attr_len) => {
                    if sys_attr_len > 0 {
                        // Get sys_attrs correctly, check whether it's same with saved bond info.
                        // If not, update bond info
                        if !(info.sys_attr.length == sys_attr_len
                            && info.sys_attr.data[0..sys_attr_len] == buf[0..sys_attr_len])
                        {
                            debug!(
                                "Updating sys_attr:\nnew: {},{}\nold: {},{}",
                                buf, sys_attr_len, info.sys_attr.data, info.sys_attr.length
                            );
                            // Update bond info
                            info.sys_attr.data[0..sys_attr_len]
                                .copy_from_slice(&buf[0..sys_attr_len]);
                            info.sys_attr.length = sys_attr_len;

                            // Save new bond info to flash
                            match FLASH_CHANNEL
                                .try_send(FlashOperationMessage::BondInfo(info.clone()))
                            {
                                Ok(_) => debug!("Sent bond info to flash channel"),
                                Err(_e) => error!("Send bond info to flash channel error"),
                            };
                        }
                    } else {
                        error!("Got empty system attr");
                    }
                }
                Err(e) => {
                    error!("Get system attr for {} erro: {}", info, e);
                }
            }
        } else {
            info!("Peer doesn't match: {}", conn.peer_address());
        }
    }

    fn load_sys_attrs(&self, conn: &Connection) {
        let addr = conn.peer_address();
        info!("Loading system attributes for {}", addr);

        let bond_info = self.bond_info.borrow();

        let sys_attr = bond_info
            .iter()
            .filter(|(_, b)| b.sys_attr.length != 0 && b.removed == false)
            .find(|(_, b)| b.peer.peer_id.is_match(addr))
            .map(|(_, b)| &b.sys_attr.data[0..b.sys_attr.length]);

        // info!("call set_sys_attrs in load_sys_attrs: {}", sys_attr);
        if let Err(err) = set_sys_attrs(conn, sys_attr) {
            warn!("SecurityHandler failed to set sys attrs: {:?}", err);
        }
    }
}
