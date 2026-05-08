//! RMK protocol host service.
//!
//! Implements the postcard-rpc-based protocol declared in
//! `rmk-types/src/protocol/rmk/`. Each active transport (USB, BLE) owns its
//! own `Server` instance with a private RX buffer and dispatch table; both
//! servers share a single `&KeyMap` via the `Ctx` struct.
//!
//! ## Why a hand-written `Dispatch` impl, not `define_dispatch!`
//!
//! postcard-rpc's `define_dispatch!` macro expands to a non-generic `Dispatch`
//! impl bound to one concrete `tx_impl` type. RMK's per-transport servers are
//! generic over `D: embassy_usb::driver::Driver<'static>` (USB) and stay
//! generic until the user-side firmware picks a chip-specific Driver, so a
//! single fixed `tx_impl` won't fit. The macro also currently expands to code
//! that uses an unstable `str::as_str()` shim. We therefore write the
//! `Dispatch` impl by hand: one *blanket* impl over `Tx: WireTx` that the
//! Server's concrete Tx selects into.
//!
//! ## Concurrency invariant
//!
//! Handlers must NOT hold a `KeyMap` `RefCell` borrow across `.await`.
//! See `handlers/mod.rs` for the full rule.

pub(crate) mod handlers;
mod service;
pub(crate) mod topics;
#[cfg(feature = "_ble")]
pub(crate) mod wire_ble;
#[cfg(not(feature = "_no_usb"))]
pub(crate) mod wire_usb;

use postcard_rpc::header::{VarHeader, VarKey, VarKeyKind};
use postcard_rpc::server::{Dispatch, Sender, WireTx};
use postcard_rpc::standard_icd::{ERROR_KEY, PingEndpoint, WireError};
use postcard_rpc::{Endpoint, Key};
use rmk_types::protocol::rmk::*;
pub use service::RmkProtocolService;

use crate::keymap::KeyMap;

/// Shared dispatch context. Held by both the USB and BLE Servers.
///
/// The lifetime `'a` ties the borrowed `KeyMap` to the surrounding async-task
/// scope: the orchestrator-generated `main` (or hand-rolled examples) hold
/// `keymap` as a local for the entire program, and every Server future
/// borrows from it for the duration. We don't require `'static` because the
/// keymap data lives on the main task's stack frame, not in a static.
pub struct Ctx<'a> {
    pub(crate) keymap: &'a KeyMap<'a>,
}

impl<'a> Ctx<'a> {
    pub fn new(keymap: &'a KeyMap<'a>) -> Self {
        Self { keymap }
    }
}

/// Top-level dispatch app. Owned by each per-transport `Server`. Both transports
/// instantiate one with the same `&KeyMap`; concurrent dispatch is safe because
/// `KeyMap`'s `RefCell` only borrows within sync method calls (see
/// `handlers/mod.rs`).
///
/// Parameterized on the `Tx` impl so the blanket `Dispatch` impl below can
/// name a concrete `type Tx = Tx`. The `_tx` `PhantomData` only constrains
/// the parameter; the app holds no transport-specific state. Using
/// `PhantomData<fn() -> Tx>` makes the marker invariant in `Tx` without
/// requiring `Tx: Send`.
pub struct RmkProtocolApp<'a, Tx: WireTx> {
    pub ctx: Ctx<'a>,
    _tx: core::marker::PhantomData<fn() -> Tx>,
}

impl<'a, Tx: WireTx> RmkProtocolApp<'a, Tx> {
    pub fn new(ctx: Ctx<'a>) -> Self {
        Self {
            ctx,
            _tx: core::marker::PhantomData,
        }
    }
}

/// Local macro that expands one match arm per endpoint:
/// `<EP as Endpoint>::REQ_KEY => decode → call handler → reply`.
macro_rules! ep_arm {
    ($ep:ty, $handler:path, $self:ident, $tx:ident, $hdr:ident, $body:ident) => {{
        let Ok(req) = postcard::from_bytes::<<$ep as Endpoint>::Request>($body) else {
            return $tx.error($hdr.seq_no, WireError::DeserFailed).await;
        };
        let resp = $handler(&mut $self.ctx, $hdr.clone(), req).await;
        $tx.reply::<$ep>($hdr.seq_no, &resp).await
    }};
}

/// Endpoint registration table. One row per `(EndpointType => handler::path)`.
/// Per-row `#[cfg(...)]` is supported and gates the whole match arm.
///
/// Adding an endpoint is a one-line edit to the table in `handle()`. The
/// generated code is identical to a hand-written chain of
/// `if keyb == <EP as Endpoint>::REQ_KEY { return ep_arm!(...); }` arms.
macro_rules! dispatch_endpoints {
    (
        $self:ident, $tx:ident, $hdr:ident, $body:ident, $keyb:ident,
        { $( $(#[$meta:meta])* $ep:ty => $handler:path ),* $(,)? }
    ) => {
        $(
            $(#[$meta])*
            if $keyb == <$ep as Endpoint>::REQ_KEY {
                return ep_arm!($ep, $handler, $self, $tx, $hdr, $body);
            }
        )*
    };
}

impl<'a, Tx> Dispatch for RmkProtocolApp<'a, Tx>
where
    Tx: WireTx,
{
    type Tx = Tx;

    fn min_key_len(&self) -> VarKeyKind {
        // 8-byte keys are unambiguous; smaller key sizes require a perfect-hash
        // analysis the upstream macro normally provides. v1 trades a few extra
        // bytes per frame for simpler dispatch.
        VarKeyKind::Key8
    }

    async fn handle(&mut self, tx: &Sender<Tx>, hdr: &VarHeader, body: &[u8]) -> Result<(), Tx::Error> {
        // Reduce the wire key to a full 8-byte Key. Smaller forms are valid on
        // the wire, but this v1 server requires Key8 (matches `min_key_len`).
        let Ok(keyb) = Key::try_from(&hdr.key) else {
            return tx.error(hdr.seq_no, WireError::KeyTooSmall).await;
        };

        // Standard ICD: ping endpoint.
        if keyb == <PingEndpoint as Endpoint>::REQ_KEY {
            let Ok(req) = postcard::from_bytes::<<PingEndpoint as Endpoint>::Request>(body) else {
                return tx.error(hdr.seq_no, WireError::DeserFailed).await;
            };
            return tx.reply::<PingEndpoint>(hdr.seq_no, &req).await;
        }

        use handlers::*;

        dispatch_endpoints!(self, tx, hdr, body, keyb, {
            // System
            GetVersion          => system::get_version,
            GetCapabilities     => system::get_capabilities,
            GetLockStatus       => system::get_lock_status,
            UnlockRequest       => system::unlock_request,
            LockRequest         => system::lock_request,
            Reboot              => system::reboot,
            BootloaderJump      => system::bootloader_jump,
            StorageReset        => system::storage_reset,

            // Keymap
            GetKeyAction        => keymap::get_key_action,
            SetKeyAction        => keymap::set_key_action,
            GetDefaultLayer     => keymap::get_default_layer,
            SetDefaultLayer     => keymap::set_default_layer,
            #[cfg(feature = "bulk_transfer")]
            GetKeymapBulk       => keymap::bulk::get_keymap_bulk,
            #[cfg(feature = "bulk_transfer")]
            SetKeymapBulk       => keymap::bulk::set_keymap_bulk,

            // Encoder
            GetEncoderAction    => encoder::get_encoder_action,
            SetEncoderAction    => encoder::set_encoder_action,

            // Macro
            GetMacro            => macro_data::get_macro,
            SetMacro            => macro_data::set_macro,

            // Combo
            GetCombo            => combo::get_combo,
            SetCombo            => combo::set_combo,
            #[cfg(feature = "bulk_transfer")]
            GetComboBulk        => combo::bulk::get_combo_bulk,
            #[cfg(feature = "bulk_transfer")]
            SetComboBulk        => combo::bulk::set_combo_bulk,

            // Morse
            GetMorse            => morse::get_morse,
            SetMorse            => morse::set_morse,
            #[cfg(feature = "bulk_transfer")]
            GetMorseBulk        => morse::bulk::get_morse_bulk,
            #[cfg(feature = "bulk_transfer")]
            SetMorseBulk        => morse::bulk::set_morse_bulk,

            // Fork
            GetFork             => fork::get_fork,
            SetFork             => fork::set_fork,

            // Behavior
            GetBehaviorConfig   => behavior::get_behavior_config,
            SetBehaviorConfig   => behavior::set_behavior_config,

            // Connection
            GetConnectionType   => connection::get_connection_type,
            SetConnectionType   => connection::set_connection_type,

            // Status
            GetCurrentLayer     => status::get_current_layer,
            GetMatrixState      => status::get_matrix_state,

            // BLE
            #[cfg(feature = "_ble")]
            GetBleStatus        => ble::get_ble_status,
            #[cfg(feature = "_ble")]
            SwitchBleProfile    => ble::switch_ble_profile,
            #[cfg(feature = "_ble")]
            ClearBleProfile     => ble::clear_ble_profile,
            #[cfg(feature = "_ble")]
            GetBatteryStatus    => ble::get_battery_status,
        });

        // Unknown key
        let _ = ERROR_KEY;
        let _ = VarKey::Key8(Key::from(keyb));
        tx.error(hdr.seq_no, WireError::UnknownKey).await
    }
}
