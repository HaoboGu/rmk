//! Manage BLE profiles
//!

use core::sync::atomic::Ordering;

use defmt::info;
use embassy_futures::yield_now;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};

use crate::{
    ble::nrf::{ACTIVE_PROFILE, BONDED_DEVICE_NUM},
    storage::{FlashOperationMessage, FLASH_CHANNEL},
};

pub(crate) static BLE_PROFILE_CHANNEL: Channel<CriticalSectionRawMutex, BleProfileAction, 1> =
    Channel::new();

/// BLE profile switch action
pub(crate) enum BleProfileAction {
    SwitchProfile(u8),
    PreviousProfile,
    NextProfile,
    ClearProfile,
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
                    .send(FlashOperationMessage::ActiveBleProfile(profile))
                    .await;
                info!("Clear profile");
            }
        }
        break;
    }
    yield_now().await;
    // TODO: How to ensure that the flash operations have been completed?
    embassy_time::Timer::after_secs(1).await
}
