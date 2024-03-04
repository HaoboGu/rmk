use core::{
    cell::{Cell, RefCell},
    mem, 
};
use defmt::{debug, error, info, warn, Format};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::Channel;
use nrf_softdevice::ble::{
    gatt_server::{get_sys_attrs, set_sys_attrs},
    security::{IoCapabilities, SecurityHandler},
    Connection, EncryptionInfo, IdentityKey, MasterId, SecurityMode,
};

pub(crate) enum StoredBondInfo {
    Peer(Peer),
    SystemAttribute(SystemAttribute),
}

#[repr(C)]
#[derive(Clone, Copy, Format)]
pub(crate) struct Peer {
    pub(crate) master_id: MasterId,
    pub(crate) key: EncryptionInfo,
    pub(crate) peer_id: IdentityKey,
}

impl Peer {
    pub(crate) fn to_slice(&self) -> [u8; 50] {
        let buf = unsafe { mem::transmute_copy(self) };
        buf
    }

    pub(crate) fn from_slice(s: [u8; 50]) -> Self {
        let data = unsafe { mem::transmute(s) };
        data
    }
}

#[repr(C)]
#[derive(Clone, Copy, Format)]
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

// TODO: Finish `Bonder`, store keys after pairing, add encryption approach
// FIXME: Reconnection issue
pub(crate) struct Bonder {
    peer: Cell<Option<Peer>>,
    pub(crate) sys_attrs: RefCell<SystemAttribute>,
}

impl Default for Bonder {
    fn default() -> Self {
        Bonder {
            peer: Cell::new(None),
            sys_attrs: Default::default(),
        }
    }
}

impl Bonder {
    pub(crate) fn new(sys_attr: RefCell<SystemAttribute>, peer_info: Cell<Option<Peer>>) -> Self {
        Self {
            peer: peer_info,
            sys_attrs: sys_attr,
        }
    }
}

pub(crate) static FLASH_CHANNEL: Channel<ThreadModeRawMutex, StoredBondInfo, 2> = Channel::new();

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
        info!("On bonded: storing bond for: id: {}, key: {}", master_id, key);

        self.sys_attrs.borrow_mut().clear();
        self.peer.set(Some(Peer {
            master_id,
            key,
            peer_id,
        }));

        match FLASH_CHANNEL.try_send(StoredBondInfo::Peer(self.peer.get().clone().unwrap())) {
            Ok(_) => debug!("Sent peer to flash channel"),
            Err(_e) => error!("Send peer to flash channel error"),
        }
    }

    fn get_key(&self, _conn: &Connection, master_id: MasterId) -> Option<EncryptionInfo> {
        // Reconnecting with an existing bond
        info!("Getting bond for: id: {}", master_id);

        self.peer
            .get()
            .and_then(|peer| (master_id == peer.master_id).then_some(peer.key))
    }

    fn save_sys_attrs(&self, conn: &Connection) {
        // On disconnect usually
        info!("Saving system attributes for: {}", conn.peer_address());

        if let Some(peer) = self.peer.get() {
            if peer.peer_id.is_match(conn.peer_address()) {
                info!("Peer {} matched: {}", peer.peer_id, conn.peer_address());

                let mut sys_attrs = self.sys_attrs.borrow_mut();
                let len = get_sys_attrs(conn, &mut sys_attrs.data).unwrap() as u16;
                sys_attrs.length = len as usize;
                let info: StoredBondInfo = StoredBondInfo::SystemAttribute(sys_attrs.clone());
                match FLASH_CHANNEL.try_send(info) {
                    Ok(_) => info!("Send sys attr"),
                    Err(_e) => error!("Send sys attr error:"),
                }
            } else {
                info!(
                    "Peer {} Doesn't match: {}",
                    peer.peer_id,
                    conn.peer_address()
                );
            }
        }
    }

    fn load_sys_attrs(&self, conn: &Connection) {
        let addr = conn.peer_address();
        info!("Loading system attributes for: {}", addr);

        let sys_attrs = self.sys_attrs.borrow();
        info!("peer id: {}", self.peer.get().unwrap().peer_id);
        let attrs = if self
            .peer
            .get()
            .map(|peer| peer.peer_id.is_match(addr))
            .unwrap_or(false)
        {
            if sys_attrs.length == 0 {
                None
            } else {
                Some(&sys_attrs.data[0..sys_attrs.length])
            }
        } else {
            None
        };

        info!("Loaded system attributes: {:#X}", attrs);

        if let Err(err) = set_sys_attrs(conn, attrs) {
            warn!("SecurityHandler failed to set sys attrs: {:?}", err);
        }
    }
}
