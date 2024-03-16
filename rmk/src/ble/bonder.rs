use super::BONDED_DEVICE_NUM;
use crate::storage::{FlashOperationMessage, FLASH_CHANNEL};
use core::cell::RefCell;
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

// Error when saving bond info into storage
#[derive(Clone, Copy, Debug, Format)]
pub enum StorageError {
    BufferTooSmall,
    ItemWrongSize,
}

// // `sequential-storage` is used for saving bond info
// // Hence `StorageItem` should be implemented
// impl StorageItem for BondInfo {
//     type Key = u8;

//     type Error = StorageError;

//     fn serialize_into(&self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
//         if buffer.len() < 120 {
//             return Err(StorageError::BufferTooSmall);
//         }

//         // Must be 120
//         // info!("size of BondInfo: {}", size_of_val(self));

//         let buf: [u8; 120] = unsafe { mem::transmute_copy(self) };
//         buffer[0..120].copy_from_slice(&buf);
//         Ok(buf.len())
//     }

//     fn deserialize_from(buffer: &[u8]) -> Result<Self, Self::Error>
//     where
//         Self: Sized,
//     {
//         if buffer.len() != 120 {
//             return Err(StorageError::ItemWrongSize);
//         }
//         // Make `transmute_copy` happy, because the compiler doesn't know the size of buffer
//         let mut buf = [0_u8; 120];
//         buf.copy_from_slice(buffer);

//         let info = unsafe { mem::transmute_copy(&buf) };

//         Ok(info)
//     }

//     fn key(&self) -> Self::Key {
//         self.slot_num
//     }
// }

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

// Bonder aka security handler used in advertising & pairing
pub(crate) struct Bonder {
    // Info of all bonded devices
    // `slot_num` is used as the key, because using peer as key will bring a lot more complexity
    bond_info: RefCell<FnvIndexMap<u8, BondInfo, BONDED_DEVICE_NUM>>,
}

impl Default for Bonder {
    fn default() -> Self {
        Bonder {
            bond_info: RefCell::new(FnvIndexMap::new()),
        }
    }
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
        _conn: &Connection,
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
            match FLASH_CHANNEL.try_send(FlashOperationMessage::Clear(unlucky)) {
                Ok(_) => debug!("Sent clear to flash channel"),
                Err(_e) => error!("Send clear to flash channel error"),
            }
            self.bond_info.borrow_mut().remove(&unlucky);
        } else {
            // Save bond info
            let new_bond_info = BondInfo {
                sys_attr: SystemAttribute::default(),
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
        info!("Saving system attributes for {}", addr);

        let mut bond_info = self.bond_info.borrow_mut();

        // Get bonded peer
        let bonded = bond_info
            .iter_mut()
            .find(|(_, info)| info.peer.peer_id.is_match(addr));

        if let Some((_, info)) = bonded {
            // Get system attr and save to flash
            info.sys_attr.length = match get_sys_attrs(conn, &mut info.sys_attr.data) {
                Ok(length) => length,
                Err(e) => {
                    error!("Get system attr for {} erro: {}", info, e);
                    0
                }
            };

            // Correctly get system attr, save to flash
            match FLASH_CHANNEL.try_send(FlashOperationMessage::BondInfo(info.clone())) {
                Ok(_) => debug!("Sent bond info to flash channel"),
                Err(_e) => error!("Send bond info to flash channel error"),
            };
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

        if let Err(err) = set_sys_attrs(conn, sys_attr) {
            warn!("SecurityHandler failed to set sys attrs: {:?}", err);
        }
    }
}
