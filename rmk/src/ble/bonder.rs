use core::cell::{Cell, RefCell};
use defmt::{debug, warn};
use heapless::Vec;
use nrf_softdevice::ble::{
    gatt_server::{get_sys_attrs, set_sys_attrs},
    security::{IoCapabilities, SecurityHandler},
    Connection, EncryptionInfo, IdentityKey, MasterId, SecurityMode,
};

#[derive(Clone, Copy)]
struct Peer {
    master_id: MasterId,
    key: EncryptionInfo,
    peer_id: IdentityKey,
}

// TODO: Finish `Bonder`, store keys after pairing, add encryption approach
// FIXME: Reconnection issue
pub(crate) struct Bonder {
    peer: Cell<Option<Peer>>,
    sys_attrs: RefCell<Vec<u8, 62>>,
}

impl Default for Bonder {
    fn default() -> Self {
        Bonder {
            peer: Cell::new(None),
            sys_attrs: Default::default(),
        }
    }
}

impl SecurityHandler for Bonder {
    fn io_capabilities(&self) -> IoCapabilities {
        IoCapabilities::None
    }

    fn can_bond(&self, _conn: &Connection) -> bool {
        true
    }

    // fn display_passkey(&self, passkey: &[u8; 6]) {
    //     info!("[BT_HID] Passkey: {}", Debug2Format(passkey));
    // }

    // fn enter_passkey(&self, _reply: nrf_softdevice::ble::PasskeyReply) {}

    fn on_security_update(&self, _conn: &Connection, security_mode: SecurityMode) {
        debug!("[BT_HID] new security mode: {}", security_mode);
    }

    fn on_bonded(
        &self,
        _conn: &Connection,
        master_id: MasterId,
        key: EncryptionInfo,
        peer_id: IdentityKey,
    ) {
        // First time
        debug!("[BT_HID] storing bond for: id: {}, key: {}", master_id, key);

        // TODO: save keys
        self.sys_attrs.borrow_mut().clear();
        self.peer.set(Some(Peer {
            master_id,
            key,
            peer_id,
        }))
    }

    fn get_key(&self, _conn: &Connection, master_id: MasterId) -> Option<EncryptionInfo> {
        // Reconnecting with an existing bond
        debug!("[BT_HID] getting bond for: id: {}", master_id);

        self.peer
            .get()
            .and_then(|peer| (master_id == peer.master_id).then_some(peer.key))
    }

    fn save_sys_attrs(&self, conn: &Connection) {
        // On disconnect usually
        debug!(
            "[BT_HID] saving system attributes for: {}",
            conn.peer_address()
        );

        if let Some(peer) = self.peer.get() {
            if peer.peer_id.is_match(conn.peer_address()) {
                let mut sys_attrs = self.sys_attrs.borrow_mut();
                let capacity = sys_attrs.capacity();
                sys_attrs.resize(capacity, 0).unwrap();
                let len = get_sys_attrs(conn, &mut sys_attrs).unwrap() as u16;
                sys_attrs.truncate(len as usize);
                // TODO: save sys_attrs for peer
            }
        }
    }

    fn load_sys_attrs(&self, conn: &Connection) {
        let addr = conn.peer_address();
        debug!("[BT_HID] loading system attributes for: {}", addr);

        let attrs = self.sys_attrs.borrow();

        // TODO: search stored peers
        let attrs = if self
            .peer
            .get()
            .map(|peer| peer.peer_id.is_match(addr))
            .unwrap_or(false)
        {
            (!attrs.is_empty()).then_some(attrs.as_slice())
        } else {
            None
        };

        if let Err(err) = set_sys_attrs(conn, attrs) {
            warn!(
                "[BT_HID] SecurityHandler failed to set sys attrs: {:?}",
                err
            );
        }
    }
}
