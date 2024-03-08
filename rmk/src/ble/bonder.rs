use core::{borrow::BorrowMut, cell::RefCell, mem};
use defmt::{debug, error, info, warn, Format};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::Channel;
use futures::future::ok;
use heapless::{FnvIndexMap, Vec};
use nrf_softdevice::ble::{
    gatt_server::{get_sys_attrs, set_sys_attrs},
    security::{IoCapabilities, SecurityHandler},
    Connection, EncryptionInfo, IdentityKey, MasterId, SecurityMode,
};
use sequential_storage::map::StorageItem;

use super::BONDED_DEVICE_NUM;

// Sync messages from server to flash
pub(crate) static FLASH_CHANNEL: Channel<ThreadModeRawMutex, FlashOperationMessage, 2> =
    Channel::new();

// Bond info which will be stored in flash
#[derive(Clone, Copy, Debug, Format)]
pub(crate) struct BondInfo {
    pub(crate) slot_num: u8,
    pub(crate) peer: Peer,
    sys_attr: SystemAttribute,
}

// `sequential-storage` is used for saving bond info
// Hence `StorageItem` should be implemented
impl StorageItem for BondInfo {
    type Key = u8;

    type Error = StorageError;

    fn serialize_into(&self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        if buffer.len() < 120 {
            return Err(StorageError::BufferTooSmall);
        }

        // Must be 120
        // info!("size of BondInfo: {}", size_of_val(self));

        let buf: [u8; 120] = unsafe { mem::transmute_copy(self) };
        buffer[0..120].copy_from_slice(&buf);
        Ok(buf.len())
    }

    fn deserialize_from(buffer: &[u8]) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        if buffer.len() != 120 {
            return Err(StorageError::ItemWrongSize);
        }
        // Make `transmute_copy` happy, because the compiler doesn't know the size of buffer
        let mut buf = [0_u8; 120];
        buf.copy_from_slice(buffer);

        let info = unsafe { mem::transmute_copy(&buf) };

        Ok(info)
    }

    fn key(&self) -> Self::Key {
        self.slot_num
    }
}

#[derive(Clone, Copy, Debug, Format)]
pub(crate) enum FlashOperationMessage {
    BondInfo(BondInfo),
    // Clear info of given slot number
    Clear(u8),
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Format)]
pub(crate) struct Peer {
    pub(crate) master_id: MasterId,
    pub(crate) key: EncryptionInfo,
    pub(crate) peer_id: IdentityKey,
}

// Error when saving bond info into storage
#[derive(Clone, Copy, Debug, Format)]
pub enum StorageError {
    BufferTooSmall,
    ItemWrongSize,
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

impl SystemAttribute {
    pub(crate) const fn new() -> Self {
        Self {
            length: 0,
            data: [0; 62],
        }
    }

    pub(crate) fn clear(&mut self) {
        self.length = 0;
        self.data.fill(0);
    }

    pub(crate) fn to_slice(&self) -> [u8; 64] {
        let mut serialized = [0; 64];
        serialized[0] = self.length as u8;
        serialized[2..64].copy_from_slice(&self.data);
        serialized
    }

    pub(crate) fn from_slice(s: [u8; 64]) -> Self {
        let mut data: [u8; 62] = [0; 62];
        data.copy_from_slice(&s[2..64]);
        Self {
            length: s[0] as usize,
            data,
        }
    }
}

pub(crate) struct Bonder {
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
        info!(
            "On bonded: storing bond for: id: {}, key: {}",
            master_id, key
        );

        // Get slot num first
        // Still use iterator here, because using peer as key will lead to a lot more complexity
        let slot_num = self
            .bond_info
            .borrow()
            .iter()
            .find(|(i, b)| b.peer.peer_id.addr == peer_id.addr)
            .map(|(i, b)| *i)
            .unwrap_or(self.bond_info.borrow().len() as u8);

        // Check free-slot first
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
            let new_bond_info = BondInfo {
                sys_attr: SystemAttribute::new(),
                peer: Peer {
                    master_id,
                    key,
                    peer_id,
                },
                slot_num: slot_num as u8,
            };

            match FLASH_CHANNEL.try_send(FlashOperationMessage::BondInfo(new_bond_info)) {
                Ok(_) => {
                    // Update self.bond_info as well
                    debug!("Sent bond info to flash channel");
                    self.bond_info.borrow_mut().insert(slot_num, new_bond_info);
                }
                Err(_e) => error!("Send bond info to flash channel error"),
            }
        }
    }

    fn get_key(&self, _conn: &Connection, master_id: MasterId) -> Option<EncryptionInfo> {
        // Reconnecting with an existing bond
        info!("Getting bond for: id: {}", master_id);

        self.bond_info
            .borrow()
            .iter()
            .find(|(i, info)| info.peer.master_id == master_id)
            .and_then(|(_, d)| Some(d.peer.key))
    }

    fn save_sys_attrs(&self, conn: &Connection) {
        // On disconnect usually
        info!("Saving system attributes for: {}", conn.peer_address());

        // self.bond_info
        //     .borrow()
        //     .iter()
        //     .for_each(|i| info!("Saved bond info: {}", i));

        let mut bond_info = self.bond_info.borrow_mut();

        let bonded_info = bond_info
            .iter_mut()
            .find(|(_, info)| info.peer.peer_id.is_match(conn.peer_address()));

        match bonded_info {
            Some((idx, info)) => {
                // Find a match, get sys attr and save
                // TODO: Check data 是否已经更新
                info!("Before: {}", info);
                info.sys_attr.length = get_sys_attrs(conn, &mut info.sys_attr.data).unwrap();
                info!("After: {}", info);

                match FLASH_CHANNEL.try_send(FlashOperationMessage::BondInfo(info.clone())) {
                    Ok(_) => {
                        debug!("Sent bond info to flash channel");
                        // bond_info.insert(*idx, info.clone());
                    }
                    Err(_e) => error!("Send bond info to flash channel error"),
                };
            }
            None => {
                info!("Peer doesn't match: {}", conn.peer_address());
            }
        }

        bond_info.iter().for_each(|(idx, info)| {
            info!("Double Check: {}: {}", idx, info);
        })
    }

    fn load_sys_attrs(&self, conn: &Connection) {
        let addr = conn.peer_address();
        info!("Loading system attributes for: {}", addr);

        let bond_info = self.bond_info.borrow();

        let sys_attr = bond_info
            .iter()
            .find(|(i, b)| b.peer.peer_id.is_match(addr))
            // .filter(|(i, b)| b.sys_attr.length != 0)
            .map(|(i, b)| &b.sys_attr.data[0..b.sys_attr.length]);

        info!(
            "System attributes found for peer with address {}: {:?}",
            addr, sys_attr
        );

        if let Err(err) = set_sys_attrs(conn, sys_attr) {
            warn!("SecurityHandler failed to set sys attrs: {:?}", err);
        }
    }
}
