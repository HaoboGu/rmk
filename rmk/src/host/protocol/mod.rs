//! RMK protocol service.
//!
//! Handles incoming postcard-rpc frames and dispatches to endpoint handlers.
//! Real endpoint handlers will be added in Phases 4-6; transport implementations
//! come in Phase 3 (USB) and Phase 8 (BLE).

pub(crate) mod transport;

use core::cell::RefCell;

use postcard_rpc::header::{VarHeader, VarKey, VarKeyKind};
use postcard_rpc::server::{
    AsWireRxErrorKind, AsWireTxErrorKind, Sender, WireRx, WireRxErrorKind, WireTx, WireTxErrorKind, min_key_needed,
};
use postcard_rpc::standard_icd::WireError;
use postcard_rpc::{Endpoint, Key, Topic};
use rmk_types::protocol::rmk::*;

use crate::keymap::KeyMap;

const RX_BUF_SIZE: usize = 256;

// All endpoint and topic keys for minimum key length computation.
const ALL_KEYS: &[Key] = &[
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
    // Topics
    LayerChangeTopic::TOPIC_KEY,
    WpmUpdateTopic::TOPIC_KEY,
    BatteryStatusTopic::TOPIC_KEY,
    BleStatusChangeTopic::TOPIC_KEY,
    ConnectionChangeTopic::TOPIC_KEY,
    SleepStateTopic::TOPIC_KEY,
    LedIndicatorTopic::TOPIC_KEY,
];

const MIN_KEY_LEN: usize = min_key_needed(&[ALL_KEYS]);

const MIN_KEY_KIND: VarKeyKind = match MIN_KEY_LEN {
    1 => VarKeyKind::Key1,
    2 => VarKeyKind::Key2,
    4 => VarKeyKind::Key4,
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
            locked: cfg!(feature = "host_security"),
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
            rx.wait_connection().await;
            tx.wait_connection().await;

            loop {
                let frame = match rx.receive(rx_buf).await {
                    Ok(f) => f,
                    Err(e) => match e.as_kind() {
                        WireRxErrorKind::ConnectionClosed => break,
                        WireRxErrorKind::ReceivedMessageTooLarge | WireRxErrorKind::Other => continue,
                        _ => continue,
                    },
                };

                let Some((hdr, body)) = VarHeader::take_from_slice(frame) else {
                    continue;
                };

                if let Err(e) = Self::dispatch(&hdr, body, sender, keymap, locked).await {
                    match e.as_kind() {
                        WireTxErrorKind::ConnectionClosed | WireTxErrorKind::Timeout => break,
                        WireTxErrorKind::Other => continue,
                        _ => continue,
                    }
                }
            }
        }
    }

    #[allow(unused_variables)]
    async fn dispatch(
        hdr: &VarHeader,
        body: &[u8],
        sender: &Sender<Tx>,
        keymap: &RefCell<KeyMap<'_, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
        locked: &mut bool,
    ) -> Result<(), Tx::Error> {
        let key = &hdr.key;
        let seq = hdr.seq_no;

        // --- System (8 endpoints) ---
        if *key == VarKey::Key8(GetVersion::REQ_KEY) {
            let resp = ProtocolVersion { major: 1, minor: 0 };
            return sender.reply::<GetVersion>(seq, &resp).await;
        }
        if *key == VarKey::Key8(GetCapabilities::REQ_KEY) {
            // TODO: Phase 4 — construct from const generics + build.rs constants
            return sender.error(seq, WireError::UnknownKey).await;
        }
        if *key == VarKey::Key8(GetLockStatus::REQ_KEY) {
            // TODO: Phase 4
            return sender.error(seq, WireError::UnknownKey).await;
        }
        if *key == VarKey::Key8(UnlockRequest::REQ_KEY) {
            // TODO: Phase 4
            return sender.error(seq, WireError::UnknownKey).await;
        }
        if *key == VarKey::Key8(LockRequest::REQ_KEY) {
            // TODO: Phase 4
            return sender.error(seq, WireError::UnknownKey).await;
        }
        if *key == VarKey::Key8(Reboot::REQ_KEY) {
            // TODO: Phase 4
            return sender.error(seq, WireError::UnknownKey).await;
        }
        if *key == VarKey::Key8(BootloaderJump::REQ_KEY) {
            // TODO: Phase 4
            return sender.error(seq, WireError::UnknownKey).await;
        }
        if *key == VarKey::Key8(StorageReset::REQ_KEY) {
            // TODO: Phase 4
            return sender.error(seq, WireError::UnknownKey).await;
        }

        // --- Keymap (8 endpoints) ---
        if *key == VarKey::Key8(GetKeyAction::REQ_KEY) {
            // TODO: Phase 5
            return sender.error(seq, WireError::UnknownKey).await;
        }
        if *key == VarKey::Key8(SetKeyAction::REQ_KEY) {
            // TODO: Phase 5
            return sender.error(seq, WireError::UnknownKey).await;
        }
        if *key == VarKey::Key8(GetKeymapBulk::REQ_KEY) {
            // TODO: Phase 5
            return sender.error(seq, WireError::UnknownKey).await;
        }
        if *key == VarKey::Key8(SetKeymapBulk::REQ_KEY) {
            // TODO: Phase 5
            return sender.error(seq, WireError::UnknownKey).await;
        }
        if *key == VarKey::Key8(GetLayerCount::REQ_KEY) {
            // TODO: Phase 5
            return sender.error(seq, WireError::UnknownKey).await;
        }
        if *key == VarKey::Key8(GetDefaultLayer::REQ_KEY) {
            // TODO: Phase 5
            return sender.error(seq, WireError::UnknownKey).await;
        }
        if *key == VarKey::Key8(SetDefaultLayer::REQ_KEY) {
            // TODO: Phase 5
            return sender.error(seq, WireError::UnknownKey).await;
        }
        if *key == VarKey::Key8(ResetKeymap::REQ_KEY) {
            // TODO: Phase 5
            return sender.error(seq, WireError::UnknownKey).await;
        }

        // --- Encoder (2 endpoints) ---
        if *key == VarKey::Key8(GetEncoderAction::REQ_KEY) {
            // TODO: Phase 5
            return sender.error(seq, WireError::UnknownKey).await;
        }
        if *key == VarKey::Key8(SetEncoderAction::REQ_KEY) {
            // TODO: Phase 5
            return sender.error(seq, WireError::UnknownKey).await;
        }

        // --- Macro (4 endpoints) ---
        if *key == VarKey::Key8(GetMacroInfo::REQ_KEY) {
            // TODO: Phase 5
            return sender.error(seq, WireError::UnknownKey).await;
        }
        if *key == VarKey::Key8(GetMacro::REQ_KEY) {
            // TODO: Phase 5
            return sender.error(seq, WireError::UnknownKey).await;
        }
        if *key == VarKey::Key8(SetMacro::REQ_KEY) {
            // TODO: Phase 5
            return sender.error(seq, WireError::UnknownKey).await;
        }
        if *key == VarKey::Key8(ResetMacros::REQ_KEY) {
            // TODO: Phase 5
            return sender.error(seq, WireError::UnknownKey).await;
        }

        // --- Combo (3 endpoints) ---
        if *key == VarKey::Key8(GetCombo::REQ_KEY) {
            // TODO: Phase 6
            return sender.error(seq, WireError::UnknownKey).await;
        }
        if *key == VarKey::Key8(SetCombo::REQ_KEY) {
            // TODO: Phase 6
            return sender.error(seq, WireError::UnknownKey).await;
        }
        if *key == VarKey::Key8(ResetCombos::REQ_KEY) {
            // TODO: Phase 6
            return sender.error(seq, WireError::UnknownKey).await;
        }

        // --- Morse (3 endpoints) ---
        if *key == VarKey::Key8(GetMorse::REQ_KEY) {
            // TODO: Phase 6
            return sender.error(seq, WireError::UnknownKey).await;
        }
        if *key == VarKey::Key8(SetMorse::REQ_KEY) {
            // TODO: Phase 6
            return sender.error(seq, WireError::UnknownKey).await;
        }
        if *key == VarKey::Key8(ResetMorse::REQ_KEY) {
            // TODO: Phase 6
            return sender.error(seq, WireError::UnknownKey).await;
        }

        // --- Fork (3 endpoints) ---
        if *key == VarKey::Key8(GetFork::REQ_KEY) {
            // TODO: Phase 6
            return sender.error(seq, WireError::UnknownKey).await;
        }
        if *key == VarKey::Key8(SetFork::REQ_KEY) {
            // TODO: Phase 6
            return sender.error(seq, WireError::UnknownKey).await;
        }
        if *key == VarKey::Key8(ResetForks::REQ_KEY) {
            // TODO: Phase 6
            return sender.error(seq, WireError::UnknownKey).await;
        }

        // --- Behavior (2 endpoints) ---
        if *key == VarKey::Key8(GetBehaviorConfig::REQ_KEY) {
            // TODO: Phase 6
            return sender.error(seq, WireError::UnknownKey).await;
        }
        if *key == VarKey::Key8(SetBehaviorConfig::REQ_KEY) {
            // TODO: Phase 6
            return sender.error(seq, WireError::UnknownKey).await;
        }

        // --- Connection (4 endpoints) ---
        if *key == VarKey::Key8(GetConnectionInfo::REQ_KEY) {
            // TODO: Phase 6
            return sender.error(seq, WireError::UnknownKey).await;
        }
        if *key == VarKey::Key8(SetConnectionType::REQ_KEY) {
            // TODO: Phase 6
            return sender.error(seq, WireError::UnknownKey).await;
        }
        if *key == VarKey::Key8(SwitchBleProfile::REQ_KEY) {
            // TODO: Phase 6
            return sender.error(seq, WireError::UnknownKey).await;
        }
        if *key == VarKey::Key8(ClearBleProfile::REQ_KEY) {
            // TODO: Phase 6
            return sender.error(seq, WireError::UnknownKey).await;
        }

        // --- Status (4 endpoints) ---
        if *key == VarKey::Key8(GetBatteryStatus::REQ_KEY) {
            // TODO: Phase 6
            return sender.error(seq, WireError::UnknownKey).await;
        }
        if *key == VarKey::Key8(GetCurrentLayer::REQ_KEY) {
            // TODO: Phase 6
            return sender.error(seq, WireError::UnknownKey).await;
        }
        if *key == VarKey::Key8(GetMatrixState::REQ_KEY) {
            // TODO: Phase 6
            return sender.error(seq, WireError::UnknownKey).await;
        }
        if *key == VarKey::Key8(GetSplitStatus::REQ_KEY) {
            // TODO: Phase 6
            return sender.error(seq, WireError::UnknownKey).await;
        }

        // No match
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
