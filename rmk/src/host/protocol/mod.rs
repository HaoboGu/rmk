//! RMK protocol service.
//!
//! Handles incoming postcard-rpc frames and dispatches to endpoint handlers.
//! Currently implements system and keymap endpoints; remaining endpoints
//! (encoder, macro, combo, etc.) are stubbed and return `UnknownKey`.
//!
//! The USB transport uses raw bulk transfer boundaries (short-packet
//! termination) for framing, not COBS.

pub(crate) mod transport;

use core::cell::RefCell;

use postcard_rpc::header::{VarHeader, VarKey, VarKeyKind};
use postcard_rpc::server::{
    AsWireRxErrorKind, AsWireTxErrorKind, Sender, WireRx, WireRxErrorKind, WireTx, WireTxErrorKind, min_key_needed,
};
use postcard_rpc::standard_icd::{self, WireError};
use postcard_rpc::{Endpoint, Key, Topic};
use rmk_types::protocol::rmk::*;

use crate::event::KeyboardEventPos;
use crate::keymap::KeyMap;
#[cfg(feature = "storage")]
use crate::channel::FLASH_CHANNEL;
#[cfg(feature = "storage")]
use crate::storage::FlashOperationMessage;
#[cfg(feature = "storage")]
use crate::host::storage::{KeymapData, KeymapKey};

/// Deserialize the request body or reply with `DeserFailed` and return early.
macro_rules! deser_body {
    ($body:expr, $sender:expr, $seq:expr) => {
        match postcard::from_bytes($body) {
            Ok(v) => v,
            Err(_) => return $sender.error($seq, WireError::DeserFailed).await,
        }
    };
}

// RX buffer must fit the largest possible incoming frame:
// SetKeymapBulkRequest = BulkRequest(4 bytes) + up to MAX_BULK(32) KeyAction values.
// Each KeyAction can be up to ~10 bytes postcard-serialized, so worst case is
// ~4 + 32*10 + VarHeader(~6) ≈ 330 bytes. 512 provides comfortable headroom.
const RX_BUF_SIZE: usize = 512;

// All endpoint request keys (inbound).
const REQ_KEYS: &[Key] = &[
    // System
    GetVersion::REQ_KEY,
    GetCapabilities::REQ_KEY,
    GetLockStatus::REQ_KEY,
    UnlockRequest::REQ_KEY,
    LockRequest::REQ_KEY,
    Reboot::REQ_KEY,
    BootloaderJump::REQ_KEY,
    StorageReset::REQ_KEY,
    // Keymap
    GetKeyAction::REQ_KEY,
    SetKeyAction::REQ_KEY,
    GetKeymapBulk::REQ_KEY,
    SetKeymapBulk::REQ_KEY,
    GetLayerCount::REQ_KEY,
    GetDefaultLayer::REQ_KEY,
    SetDefaultLayer::REQ_KEY,
    ResetKeymap::REQ_KEY,
    // Encoder
    GetEncoderAction::REQ_KEY,
    SetEncoderAction::REQ_KEY,
    // Macro
    GetMacroInfo::REQ_KEY,
    GetMacro::REQ_KEY,
    SetMacro::REQ_KEY,
    ResetMacros::REQ_KEY,
    // Combo
    GetCombo::REQ_KEY,
    SetCombo::REQ_KEY,
    ResetCombos::REQ_KEY,
    // Morse
    GetMorse::REQ_KEY,
    SetMorse::REQ_KEY,
    ResetMorse::REQ_KEY,
    // Fork
    GetFork::REQ_KEY,
    SetFork::REQ_KEY,
    ResetForks::REQ_KEY,
    // Behavior
    GetBehaviorConfig::REQ_KEY,
    SetBehaviorConfig::REQ_KEY,
    // Connection
    GetConnectionInfo::REQ_KEY,
    SetConnectionType::REQ_KEY,
    SwitchBleProfile::REQ_KEY,
    ClearBleProfile::REQ_KEY,
    // Status
    GetBatteryStatus::REQ_KEY,
    GetCurrentLayer::REQ_KEY,
    GetMatrixState::REQ_KEY,
    GetSplitStatus::REQ_KEY,
];

// All endpoint response keys, topic keys, and error key (outbound).
const RESP_KEYS: &[Key] = &[
    // Error key
    standard_icd::ERROR_KEY,
    // System
    GetVersion::RESP_KEY,
    GetCapabilities::RESP_KEY,
    GetLockStatus::RESP_KEY,
    UnlockRequest::RESP_KEY,
    LockRequest::RESP_KEY,
    Reboot::RESP_KEY,
    BootloaderJump::RESP_KEY,
    StorageReset::RESP_KEY,
    // Keymap
    GetKeyAction::RESP_KEY,
    SetKeyAction::RESP_KEY,
    GetKeymapBulk::RESP_KEY,
    SetKeymapBulk::RESP_KEY,
    GetLayerCount::RESP_KEY,
    GetDefaultLayer::RESP_KEY,
    SetDefaultLayer::RESP_KEY,
    ResetKeymap::RESP_KEY,
    // Encoder
    GetEncoderAction::RESP_KEY,
    SetEncoderAction::RESP_KEY,
    // Macro
    GetMacroInfo::RESP_KEY,
    GetMacro::RESP_KEY,
    SetMacro::RESP_KEY,
    ResetMacros::RESP_KEY,
    // Combo
    GetCombo::RESP_KEY,
    SetCombo::RESP_KEY,
    ResetCombos::RESP_KEY,
    // Morse
    GetMorse::RESP_KEY,
    SetMorse::RESP_KEY,
    ResetMorse::RESP_KEY,
    // Fork
    GetFork::RESP_KEY,
    SetFork::RESP_KEY,
    ResetForks::RESP_KEY,
    // Behavior
    GetBehaviorConfig::RESP_KEY,
    SetBehaviorConfig::RESP_KEY,
    // Connection
    GetConnectionInfo::RESP_KEY,
    SetConnectionType::RESP_KEY,
    SwitchBleProfile::RESP_KEY,
    ClearBleProfile::RESP_KEY,
    // Status
    GetBatteryStatus::RESP_KEY,
    GetCurrentLayer::RESP_KEY,
    GetMatrixState::RESP_KEY,
    GetSplitStatus::RESP_KEY,
    // Topics
    LayerChangeTopic::TOPIC_KEY,
    WpmUpdateTopic::TOPIC_KEY,
    BatteryStatusTopic::TOPIC_KEY,
    BleStatusChangeTopic::TOPIC_KEY,
    ConnectionChangeTopic::TOPIC_KEY,
    SleepStateTopic::TOPIC_KEY,
    LedIndicatorTopic::TOPIC_KEY,
];

// Compute minimum key length separately for inbound (requests) and outbound
// (responses + topics + error), then take the max. Request and response keys
// don't need to be distinguishable from each other (they travel in opposite
// directions), and endpoints with `() -> ()` share the same key hash.
const MIN_KEY_LEN_IN: usize = min_key_needed(&[REQ_KEYS]);
const MIN_KEY_LEN_OUT: usize = min_key_needed(&[RESP_KEYS]);
const MIN_KEY_LEN: usize = if MIN_KEY_LEN_IN > MIN_KEY_LEN_OUT {
    MIN_KEY_LEN_IN
} else {
    MIN_KEY_LEN_OUT
};

// Map the minimum key length to the next VarKeyKind that can hold it.
// VarKeyKind only supports 1, 2, 4, and 8 byte keys — round up accordingly.
const MIN_KEY_KIND: VarKeyKind = match MIN_KEY_LEN {
    0 | 1 => VarKeyKind::Key1,
    2 => VarKeyKind::Key2,
    3 | 4 => VarKeyKind::Key4,
    _ => VarKeyKind::Key8,
};

pub(crate) struct ProtocolService<
    'a,
    Tx: WireTx + Clone,
    Rx: WireRx,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
> {
    keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
    /// Kept for `wait_connection()` in the run loop. The `Sender` does not
    /// expose the inner `WireTx`, so we store a clone separately.
    tx: Tx,
    sender: Sender<Tx>,
    rx: Rx,
    rx_buf: [u8; RX_BUF_SIZE],
    locked: bool,
}

impl<
    'a,
    Tx: WireTx + Clone,
    Rx: WireRx,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
> ProtocolService<'a, Tx, Rx, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    pub(crate) fn new(
        keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
        tx: Tx,
        rx: Rx,
    ) -> Self {
        Self {
            keymap,
            sender: Sender::new(tx.clone(), MIN_KEY_KIND),
            tx,
            rx,
            rx_buf: [0u8; RX_BUF_SIZE],
            // Device starts locked. UnlockRequest currently unlocks immediately
            // without a physical key challenge (TODO: implement challenge).
            locked: true,
        }
    }

    pub(crate) async fn run(&mut self) {
        let Self {
            keymap,
            tx,
            sender,
            rx,
            rx_buf,
            locked,
        } = self;

        loop {
            // Re-lock on every new USB connection so a reconnecting host
            // must complete the unlock handshake again.
            *locked = true;

            embassy_futures::join::join(rx.wait_connection(), tx.wait_connection()).await;

            loop {
                let frame = match rx.receive(rx_buf).await {
                    Ok(f) => f,
                    Err(e) => match e.as_kind() {
                        WireRxErrorKind::ConnectionClosed => break,
                        WireRxErrorKind::ReceivedMessageTooLarge => {
                            // Cannot reply: the frame was too large to read so we
                            // don't have a sequence number. Log and drop.
                            warn!("Dropped oversize frame (>{} bytes)", RX_BUF_SIZE);
                            continue;
                        }
                        _ => continue,
                    },
                };

                let Some((hdr, body)) = VarHeader::take_from_slice(frame) else {
                    continue;
                };

                if let Err(e) = Self::dispatch(&hdr, body, sender, keymap, locked).await {
                    match e.as_kind() {
                        WireTxErrorKind::ConnectionClosed | WireTxErrorKind::Timeout => break,
                        _ => continue,
                    }
                }
            }
        }
    }

    #[inline(never)]
    async fn dispatch(
        hdr: &VarHeader,
        body: &[u8],
        sender: &Sender<Tx>,
        keymap: &RefCell<KeyMap<'_, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
        locked: &mut bool,
    ) -> Result<(), Tx::Error> {
        let key = &hdr.key;
        let seq = hdr.seq_no;

        // Linear if-chain: VarKey::PartialEq performs cross-variant XOR-fold
        // comparison, so wrapping REQ_KEY in Key8 works regardless of wire key size.

        // --- System (8 endpoints) ---
        if *key == VarKey::Key8(GetVersion::REQ_KEY) {
            return sender.reply::<GetVersion>(seq, &ProtocolVersion::CURRENT).await;
        }
        if *key == VarKey::Key8(GetCapabilities::REQ_KEY) {
            // Compile-time assertions: these const generics must fit in u8.
            const { assert!(ROW <= 255, "ROW exceeds u8 range") };
            const { assert!(COL <= 255, "COL exceeds u8 range") };
            const { assert!(NUM_LAYER <= 255, "NUM_LAYER exceeds u8 range") };
            const { assert!(NUM_ENCODER <= 255, "NUM_ENCODER exceeds u8 range") };

            let caps = DeviceCapabilities {
                protocol_version: ProtocolVersion::CURRENT,
                num_layers: NUM_LAYER as u8,
                num_rows: ROW as u8,
                num_cols: COL as u8,
                num_encoders: NUM_ENCODER as u8,
                max_combos: crate::COMBO_MAX_NUM as u8,
                max_macros: crate::MACRO_MAX_NUM as u8,
                macro_space_size: crate::MACRO_SPACE_SIZE as u16,
                max_morse: crate::MORSE_MAX_NUM as u8,
                max_forks: crate::FORK_MAX_NUM as u8,
                has_storage: cfg!(feature = "storage"),
                has_split: cfg!(feature = "split"),
                num_split_peripherals: crate::SPLIT_PERIPHERALS_NUM as u8,
                has_ble: cfg!(feature = "_ble"),
                num_ble_profiles: crate::NUM_BLE_PROFILE as u8,
                has_lighting: false,
                max_payload_size: RX_BUF_SIZE as u16,
            };
            return sender.reply::<GetCapabilities>(seq, &caps).await;
        }
        if *key == VarKey::Key8(GetLockStatus::REQ_KEY) {
            let status = LockStatus {
                locked: *locked,
                awaiting_keys: false,
                remaining_keys: 0,
            };
            return sender.reply::<GetLockStatus>(seq, &status).await;
        }
        if *key == VarKey::Key8(UnlockRequest::REQ_KEY) {
            // TODO: implement physical key challenge. Currently unlocks immediately.
            *locked = false;
            return sender
                .reply::<UnlockRequest>(seq, &UnlockChallenge { key_positions: heapless::Vec::new() })
                .await;
        }
        if *key == VarKey::Key8(LockRequest::REQ_KEY) {
            *locked = true;
            return sender.reply::<LockRequest>(seq, &()).await;
        }

        // --- Device Control (3 endpoints, all Dangerous — require unlock) ---
        if *key == VarKey::Key8(Reboot::REQ_KEY) {
            if *locked {
                return sender.reply::<Reboot>(seq, &Err(RmkError::Locked)).await;
            }
            sender.reply::<Reboot>(seq, &Ok(())).await?;
            crate::boot::reboot_keyboard();
            return Ok(()); // unreachable on embedded
        }
        if *key == VarKey::Key8(BootloaderJump::REQ_KEY) {
            if *locked {
                return sender.reply::<BootloaderJump>(seq, &Err(RmkError::Locked)).await;
            }
            sender.reply::<BootloaderJump>(seq, &Ok(())).await?;
            crate::boot::jump_to_bootloader();
            return Ok(()); // unreachable on embedded
        }
        if *key == VarKey::Key8(StorageReset::REQ_KEY) {
            if *locked {
                return sender.reply::<StorageReset>(seq, &Err(RmkError::Locked)).await;
            }
            // Validate the request body even when storage is disabled, so
            // malformed requests are rejected consistently.
            let _mode: StorageResetMode = deser_body!(body, sender, seq);
            sender.reply::<StorageReset>(seq, &Ok(())).await?;
            #[cfg(feature = "storage")]
            {
                let msg = match _mode {
                    StorageResetMode::Full => FlashOperationMessage::ResetAndReboot,
                    StorageResetMode::LayoutOnly => FlashOperationMessage::ResetLayout,
                    // Future variants — treat as full reset for safety
                    _ => FlashOperationMessage::ResetAndReboot,
                };
                FLASH_CHANNEL.send(msg).await;
            }
            #[cfg(not(feature = "storage"))]
            crate::boot::reboot_keyboard();
            core::future::pending::<()>().await;
            return Ok(()); // unreachable
        }

        // --- Keymap (8 endpoints) ---
        if *key == VarKey::Key8(GetKeyAction::REQ_KEY) {
            let pos: KeyPosition = deser_body!(body, sender, seq);
            if pos.row as usize >= ROW || pos.col as usize >= COL || pos.layer as usize >= NUM_LAYER {
                // Out of bounds. WireError::DeserFailed is the closest wire error
                // available; the host should check bounds via GetCapabilities.
                return sender.error(seq, WireError::DeserFailed).await;
            }
            // key_pos takes (col, row) — note the reversed order
            let event_pos = KeyboardEventPos::key_pos(pos.col, pos.row);
            let action = keymap.borrow().get_action_at(event_pos, pos.layer as usize);
            return sender.reply::<GetKeyAction>(seq, &action).await;
        }
        if *key == VarKey::Key8(SetKeyAction::REQ_KEY) {
            if *locked {
                return sender.reply::<SetKeyAction>(seq, &Err(RmkError::Locked)).await;
            }
            let req: SetKeyRequest = deser_body!(body, sender, seq);
            let pos = &req.position;
            if pos.row as usize >= ROW || pos.col as usize >= COL || pos.layer as usize >= NUM_LAYER {
                return sender
                    .reply::<SetKeyAction>(seq, &Err(RmkError::InvalidParameter))
                    .await;
            }
            // key_pos takes (col, row) — note the reversed order
            let event_pos = KeyboardEventPos::key_pos(pos.col, pos.row);
            keymap
                .borrow_mut()
                .set_action_at(event_pos, pos.layer as usize, req.action);
            #[cfg(feature = "storage")]
            FLASH_CHANNEL
                .send(FlashOperationMessage::HostMessage(KeymapData::KeymapKey(
                    KeymapKey {
                        row: pos.row,
                        col: pos.col,
                        layer: pos.layer,
                        action: req.action,
                    },
                )))
                .await;
            return sender.reply::<SetKeyAction>(seq, &Ok(())).await;
        }
        if *key == VarKey::Key8(GetKeymapBulk::REQ_KEY) {
            let req: BulkRequest = deser_body!(body, sender, seq);
            let mut actions: BulkKeyActions = heapless::Vec::new();
            let mut row = req.start_row as usize;
            let mut col = req.start_col as usize;
            let layer = req.layer as usize;
            let count = (req.count as usize).min(MAX_BULK);
            if layer < NUM_LAYER && row < ROW && col < COL {
                let km = keymap.borrow();
                for _ in 0..count {
                    if row >= ROW {
                        break;
                    }
                    // key_pos takes (col, row)
                    let action = km.get_action_at(KeyboardEventPos::key_pos(col as u8, row as u8), layer);
                    if actions.push(action).is_err() {
                        break;
                    }
                    col += 1;
                    if col >= COL {
                        col = 0;
                        row += 1;
                    }
                }
            }
            return sender.reply::<GetKeymapBulk>(seq, &actions).await;
        }
        if *key == VarKey::Key8(SetKeymapBulk::REQ_KEY) {
            if *locked {
                return sender.reply::<SetKeymapBulk>(seq, &Err(RmkError::Locked)).await;
            }
            let req: SetKeymapBulkRequest = deser_body!(body, sender, seq);
            let layer = req.request.layer as usize;
            if layer >= NUM_LAYER {
                return sender
                    .reply::<SetKeymapBulk>(seq, &Err(RmkError::InvalidParameter))
                    .await;
            }
            let mut row = req.request.start_row as usize;
            let mut col = req.request.start_col as usize;
            if row >= ROW || col >= COL {
                return sender
                    .reply::<SetKeymapBulk>(seq, &Err(RmkError::InvalidParameter))
                    .await;
            }
            for action in req.actions.iter() {
                if row >= ROW {
                    break;
                }
                // borrow_mut() must stay as a temporary (not bound to a variable)
                // to avoid holding the borrow across the .await below.
                keymap
                    .borrow_mut()
                    .set_action_at(KeyboardEventPos::key_pos(col as u8, row as u8), layer, *action);
                #[cfg(feature = "storage")]
                FLASH_CHANNEL
                    .send(FlashOperationMessage::HostMessage(KeymapData::KeymapKey(
                        KeymapKey {
                            row: row as u8,
                            col: col as u8,
                            layer: layer as u8,
                            action: *action,
                        },
                    )))
                    .await;
                col += 1;
                if col >= COL {
                    col = 0;
                    row += 1;
                }
            }
            return sender.reply::<SetKeymapBulk>(seq, &Ok(())).await;
        }
        if *key == VarKey::Key8(GetLayerCount::REQ_KEY) {
            return sender.reply::<GetLayerCount>(seq, &(NUM_LAYER as u8)).await;
        }
        if *key == VarKey::Key8(GetDefaultLayer::REQ_KEY) {
            let layer = keymap.borrow().get_default_layer();
            return sender.reply::<GetDefaultLayer>(seq, &layer).await;
        }
        if *key == VarKey::Key8(SetDefaultLayer::REQ_KEY) {
            if *locked {
                return sender.reply::<SetDefaultLayer>(seq, &Err(RmkError::Locked)).await;
            }
            let layer: u8 = deser_body!(body, sender, seq);
            if layer as usize >= NUM_LAYER {
                return sender
                    .reply::<SetDefaultLayer>(seq, &Err(RmkError::InvalidParameter))
                    .await;
            }
            keymap.borrow_mut().set_default_layer(layer);
            #[cfg(feature = "storage")]
            FLASH_CHANNEL
                .send(FlashOperationMessage::DefaultLayer(layer))
                .await;
            return sender.reply::<SetDefaultLayer>(seq, &Ok(())).await;
        }
        if *key == VarKey::Key8(ResetKeymap::REQ_KEY) {
            if *locked {
                return sender.reply::<ResetKeymap>(seq, &Err(RmkError::Locked)).await;
            }
            sender.reply::<ResetKeymap>(seq, &Ok(())).await?;
            #[cfg(feature = "storage")]
            FLASH_CHANNEL
                .send(FlashOperationMessage::ResetLayout)
                .await;
            // Storage task handles erase + reboot; don't race it
            #[cfg(not(feature = "storage"))]
            crate::boot::reboot_keyboard();
            core::future::pending::<()>().await;
            return Ok(()); // unreachable
        }

        // Unimplemented endpoints (encoder, macro, combo, morse, fork,
        // behavior, connection, status) fall through here. Their keys are
        // declared in REQ_KEYS/RESP_KEYS for key-length computation but
        // handlers will be added in later phases.
        sender.error(seq, WireError::UnknownKey).await
    }
}

impl<
    'a,
    Tx: WireTx + Clone,
    Rx: WireRx,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
> crate::host::HostService for ProtocolService<'a, Tx, Rx, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    async fn run(&mut self) {
        ProtocolService::run(self).await;
    }
}
