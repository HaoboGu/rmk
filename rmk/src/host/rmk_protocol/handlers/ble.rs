//! Handlers for the BLE-only endpoint group.
//!
//! These are gated on `cfg(feature = "_ble")` upstream of registration.

use postcard_rpc::header::VarHeader;
use rmk_types::battery::BatteryStatus;
use rmk_types::ble::BleStatus;
use rmk_types::protocol::rmk::{RmkError, RmkResult};

use super::super::Ctx;
use crate::ble::profile::BleProfileAction;

pub(crate) async fn get_ble_status(_ctx: &mut Ctx<'_>, _hdr: VarHeader, _req: ()) -> BleStatus {
    crate::state::current_ble_status()
}

pub(crate) async fn switch_ble_profile(_ctx: &mut Ctx<'_>, _hdr: VarHeader, profile: u8) -> RmkResult {
    if (profile as usize) >= crate::NUM_BLE_PROFILE {
        return Err(RmkError::InvalidParameter);
    }
    crate::channel::BLE_PROFILE_CHANNEL
        .send(BleProfileAction::Switch(profile))
        .await;
    Ok(())
}

pub(crate) async fn clear_ble_profile(_ctx: &mut Ctx<'_>, _hdr: VarHeader, profile: u8) -> RmkResult {
    if (profile as usize) >= crate::NUM_BLE_PROFILE {
        return Err(RmkError::InvalidParameter);
    }
    crate::channel::BLE_PROFILE_CHANNEL
        .send(BleProfileAction::ClearBond(profile))
        .await;
    Ok(())
}

pub(crate) async fn get_battery_status(_ctx: &mut Ctx<'_>, _hdr: VarHeader, _req: ()) -> BatteryStatus {
    BatteryStatus::Unavailable
}

#[cfg(feature = "split")]
pub(crate) async fn get_peripheral_status(
    _ctx: &mut Ctx<'_>,
    _hdr: VarHeader,
    idx: u8,
) -> rmk_types::protocol::rmk::PeripheralStatus {
    let _ = idx;
    rmk_types::protocol::rmk::PeripheralStatus {
        connected: false,
        battery: BatteryStatus::Unavailable,
    }
}
