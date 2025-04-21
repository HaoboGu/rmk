//! Manage BLE profiles and bonding information

use core::sync::atomic::Ordering;

use embassy_futures::select::{select3, Either3};
use embassy_sync::signal::Signal;
use trouble_host::prelude::*;
use trouble_host::{BondInformation, LongTermKey};
#[cfg(feature = "storage")]
use {
    crate::channel::FLASH_CHANNEL,
    crate::storage::{FlashOperationMessage, FLASH_OPERATION_FINISHED},
};

use super::ble_server::CCCD_TABLE_SIZE;
use crate::ble::trouble::{ACTIVE_PROFILE, NUM_BLE_PROFILE};
use crate::channel::BLE_PROFILE_CHANNEL;
use crate::state::CONNECTION_TYPE;

pub(crate) static UPDATED_PROFILE: Signal<crate::RawMutex, ProfileInfo> = Signal::new();
pub(crate) static UPDATED_CCCD_TABLE: Signal<crate::RawMutex, CccdTable<CCCD_TABLE_SIZE>> = Signal::new();

/// BLE profile info
#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ProfileInfo {
    pub(crate) slot_num: u8,
    pub(crate) removed: bool,
    pub(crate) info: BondInformation,
    pub(crate) cccd_table: CccdTable<CCCD_TABLE_SIZE>,
}

impl Default for ProfileInfo {
    fn default() -> Self {
        Self {
            slot_num: 0,
            removed: false,
            info: BondInformation {
                ltk: LongTermKey(0),
                address: BdAddr::default(),
            },
            cccd_table: CccdTable::<CCCD_TABLE_SIZE>::default(),
        }
    }
}

/// BLE profile switch action
pub(crate) enum BleProfileAction {
    SwitchProfile(u8),
    PreviousProfile,
    NextProfile,
    ClearProfile,
    ToggleConnection,
}

/// Manage BLE profiles and bonding information
///
/// ProfileManager is responsible for:
/// 1. Managing multiple BLE profiles, allowing users to switch between multiple devices
/// 2. Storing and loading bonding information for each profile
/// 3. Updating the bonding information of the active profile to the BLE stack
/// 4. Handling profile switch, clear, and save operations
#[cfg(feature = "_ble")]
pub struct ProfileManager<'a, C: Controller> {
    /// List of bonded devices
    bonded_devices: heapless::Vec<ProfileInfo, NUM_BLE_PROFILE>,
    /// BLE stack
    stack: &'a Stack<'a, C>,
}

#[cfg(feature = "_ble")]
impl<'a, C: Controller> ProfileManager<'a, C> {
    /// Create a new profile manager
    pub fn new(stack: &'a Stack<'a, C>) -> Self {
        Self {
            bonded_devices: heapless::Vec::new(),
            stack,
        }
    }

    /// Load stored bonding information
    #[cfg(feature = "storage")]
    pub async fn load_bonded_devices<
        F: embedded_storage_async::nor_flash::NorFlash,
        const ROW: usize,
        const COL: usize,
        const NUM_LAYER: usize,
        const NUM_ENCODER: usize,
    >(
        &mut self,
        storage: &mut crate::storage::Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>,
    ) {
        use crate::read_storage;
        use crate::storage::{StorageData, StorageKeys};

        self.bonded_devices.clear();
        for slot_num in 0..NUM_BLE_PROFILE {
            if let Ok(Some(info)) = storage.read_trouble_bond_info(slot_num as u8).await {
                if !info.removed {
                    if let Err(e) = self.bonded_devices.push(info) {
                        error!("Failed to add bond info: {:?}", e);
                    }
                }
            }
        }
        debug!("Loaded {} bond info", self.bonded_devices.len());

        let mut buf: [u8; 128] = [0; 128];

        // Load current active profile, save to `ACTIVE_PROFILE`
        if let Ok(Some(StorageData::ActiveBleProfile(profile))) =
            read_storage!(storage, &(StorageKeys::ActiveBleProfile as u32), buf)
        {
            debug!("Loaded active profile: {}", profile);
            ACTIVE_PROFILE.store(profile, Ordering::SeqCst);
        } else {
            // If no saved active profile, use 0 as default
            debug!("Loaded default active profile",);
            ACTIVE_PROFILE.store(0, Ordering::SeqCst);
        };
    }

    /// Update bonding information in the stack according to the current active profile
    pub fn update_stack_bonds(&self) {
        let active_profile = ACTIVE_PROFILE.load(core::sync::atomic::Ordering::SeqCst);

        // Remove current bonding information in the stack
        let current_bond_info = self.stack.get_bond_information();
        for bond in current_bond_info {
            if let Err(e) = self.stack.remove_bond_information(bond.address) {
                debug!("Remove bond info error: {:?}", e);
            }
        }

        // Add bonding information for the active profile
        if let Some(info) = self
            .bonded_devices
            .iter()
            .find(|bond_info| !bond_info.removed && bond_info.slot_num == active_profile)
        {
            debug!("Add bond info of profile {}: {:?}", active_profile, info);
            if let Err(e) = self.stack.add_bond_information(info.info.clone()) {
                debug!("Add bond info error: {:?}", e);
            }
        }
    }

    /// Add/update bonding information
    pub async fn add_profile_info(&mut self, profile_info: ProfileInfo) {
        // Update profile information in memory
        if let Some(index) = self
            .bonded_devices
            .iter()
            .position(|info| info.slot_num == profile_info.slot_num)
        {
            if self.bonded_devices[index].info == profile_info.info {
                info!("Skip saving same bonding info");
                return;
            }
            // If the bonding information with the same slot number exists, update it
            self.bonded_devices[index] = profile_info.clone();
        } else {
            // If there is no bonding information with the same slot number, add it
            if let Err(e) = self.bonded_devices.push(profile_info.clone()) {
                error!("Failed to add bond info: {:?}", e);
            }
        }

        #[cfg(feature = "storage")]
        // Send bonding information to the flash task for saving
        FLASH_CHANNEL
            .send(crate::storage::FlashOperationMessage::ProfileInfo(profile_info))
            .await;
    }

    /// Update CCCD table in the stack
    pub async fn update_profile_cccd_table(&mut self, table: CccdTable<CCCD_TABLE_SIZE>) {
        // Get current active profile
        let active_profile = ACTIVE_PROFILE.load(Ordering::SeqCst);

        // Update profile information in memory
        if let Some(index) = self
            .bonded_devices
            .iter()
            .position(|info| info.slot_num == active_profile)
        {
            // Check whether the CCCD table is the same as the current one
            if self.bonded_devices[index].cccd_table.inner() == table.inner() {
                info!("Skip updating same CCCD table");
                return;
            }

            debug!("Updating profile {} CCCD table: {:?}", active_profile, table);
            let mut profile_info = self.bonded_devices[index].clone();
            profile_info.cccd_table = table;
            self.bonded_devices[index] = profile_info.clone();

            #[cfg(feature = "storage")]
            FLASH_CHANNEL
                .send(crate::storage::FlashOperationMessage::ProfileInfo(profile_info))
                .await;
        } else {
            error!("Failed to update profile CCCD table: profile not found");
        }
    }

    /// Clear bonding information of the specified slot
    pub async fn clear_bond(&mut self, slot_num: u8) {
        // Update bonding information in memory
        for bond_info in self.bonded_devices.iter_mut() {
            if bond_info.slot_num == slot_num {
                bond_info.removed = true;
            }
        }

        // Update the active bonding information in the stack
        self.update_stack_bonds();

        #[cfg(feature = "storage")]
        // Send the clear slot message to the flash task
        FLASH_CHANNEL
            .send(crate::storage::FlashOperationMessage::ClearSlot(slot_num))
            .await;
    }

    /// Switch to the specified profile, return true if the profile is switched
    pub async fn switch_profile(&self, profile: u8) -> bool {
        let current = ACTIVE_PROFILE.load(core::sync::atomic::Ordering::SeqCst);
        if profile == current {
            return false;
        }

        ACTIVE_PROFILE.store(profile, core::sync::atomic::Ordering::SeqCst);

        // Update the active bonding information in the stack
        self.update_stack_bonds();

        #[cfg(feature = "storage")]
        FLASH_CHANNEL
            .send(crate::storage::FlashOperationMessage::ActiveBleProfile(profile))
            .await;

        info!("Switched to BLE profile: {}", profile);
        true
    }

    /// Wait for profile switch event and update active profile
    ///
    /// This function will wait for profile switch operation, then update the active profile
    /// based on the operation type. After completing the operation, it will wait for a period
    /// to ensure the flash operation is completed.
    pub async fn update_profile(&mut self) {
        // Wait for profile switch or updated profile event
        loop {
            match select3(
                BLE_PROFILE_CHANNEL.receive(),
                UPDATED_PROFILE.wait(),
                UPDATED_CCCD_TABLE.wait(),
            )
            .await
            {
                Either3::First(action) => {
                    #[cfg(feature = "storage")]
                    if FLASH_OPERATION_FINISHED.signaled() {
                        FLASH_OPERATION_FINISHED.reset();
                    }
                    match action {
                        BleProfileAction::SwitchProfile(profile) => {
                            if !self.switch_profile(profile).await {
                                // If the profile is the same as the current profile, do nothing
                                continue;
                            }
                        }
                        BleProfileAction::PreviousProfile => {
                            let mut profile = ACTIVE_PROFILE.load(Ordering::SeqCst);
                            profile = if profile == 0 { 7 } else { profile - 1 };

                            self.switch_profile(profile).await;
                        }
                        BleProfileAction::NextProfile => {
                            let mut profile = ACTIVE_PROFILE.load(Ordering::SeqCst) + 1;
                            profile = profile % NUM_BLE_PROFILE as u8;

                            self.switch_profile(profile).await;
                        }
                        BleProfileAction::ClearProfile => {
                            let profile = ACTIVE_PROFILE.load(Ordering::SeqCst);
                            self.clear_bond(profile).await;
                        }
                        BleProfileAction::ToggleConnection => {
                            let current = CONNECTION_TYPE.load(Ordering::SeqCst);
                            let updated = 1 - current;
                            CONNECTION_TYPE.store(updated, Ordering::SeqCst);
                            #[cfg(feature = "storage")]
                            FLASH_CHANNEL.send(FlashOperationMessage::ConnectionType(updated)).await;
                        }
                    }
                    #[cfg(feature = "storage")]
                    FLASH_OPERATION_FINISHED.wait().await;
                    info!("Update profile done");
                    break;
                }
                Either3::Second(profile_info) => {
                    self.add_profile_info(profile_info).await;
                }
                Either3::Third(table) => {
                    self.update_profile_cccd_table(table).await;
                }
            }
        }
    }
}
