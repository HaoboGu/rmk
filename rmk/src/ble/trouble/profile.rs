//! Manage BLE profiles

use core::sync::atomic::Ordering;

use embassy_futures::yield_now;

use crate::{
    ble::trouble::{ACTIVE_PROFILE, BONDED_DEVICE_NUM},
    channel::{BLE_PROFILE_CHANNEL, FLASH_CHANNEL},
    state::CONNECTION_TYPE,
    storage::FlashOperationMessage,
};

/// BLE profile switch action
pub(crate) enum BleProfileAction {
    SwitchProfile(u8),
    PreviousProfile,
    NextProfile,
    ClearProfile,
    ToggleConnection,
}

// Wait for profile switch action and update the active profile
pub(crate) async fn update_profile() {
    // Wait until there's a profile switch action
    loop {
        match BLE_PROFILE_CHANNEL.receive().await {
            BleProfileAction::SwitchProfile(profile) => {
                let current = ACTIVE_PROFILE.load(Ordering::SeqCst);
                if profile == current {
                    // No need to switch to the same profile, just continue waiting
                    continue;
                }
                ACTIVE_PROFILE.store(profile, Ordering::SeqCst);
                FLASH_CHANNEL
                    .send(FlashOperationMessage::ActiveBleProfile(profile))
                    .await;
                info!("Switch to BLE profile: {}", profile);
            }
            BleProfileAction::PreviousProfile => {
                // Get current profile number and plus 1
                let mut profile = ACTIVE_PROFILE.load(Ordering::SeqCst);
                profile = if profile == 0 { 7 } else { profile - 1 };
                ACTIVE_PROFILE.store(profile, Ordering::SeqCst);
                FLASH_CHANNEL
                    .send(FlashOperationMessage::ActiveBleProfile(profile))
                    .await;
                info!("Switch to previous BLE profile");
            }
            BleProfileAction::NextProfile => {
                let mut profile = ACTIVE_PROFILE.load(Ordering::SeqCst) + 1;
                profile = profile % BONDED_DEVICE_NUM as u8;
                ACTIVE_PROFILE.store(profile, Ordering::SeqCst);
                FLASH_CHANNEL
                    .send(FlashOperationMessage::ActiveBleProfile(profile))
                    .await;
                info!("Switch to next BLE profile");
            }
            BleProfileAction::ClearProfile => {
                let profile = ACTIVE_PROFILE.load(Ordering::SeqCst);
                FLASH_CHANNEL
                    .send(FlashOperationMessage::ClearSlot(profile))
                    .await;
                info!("Clear profile");
            }
            BleProfileAction::ToggleConnection => {
                let current = CONNECTION_TYPE.load(Ordering::SeqCst);
                let updated = 1 - current;
                CONNECTION_TYPE.store(updated, Ordering::SeqCst);
                FLASH_CHANNEL
                    .send(FlashOperationMessage::ConnectionType(updated))
                    .await;
            }
        }
        break;
    }
    yield_now().await;
    // Wait for the flash operation to complete
    // A signal could be used here, but for simplicity, just waiting for 1s
    embassy_time::Timer::after_secs(1).await
}
