//! Manage BLE profiles and bonding information

#[cfg(feature = "_ble")]
use bt_hci::{cmd::le::LeSetPhy, controller::ControllerCmdAsync};
use embassy_futures::select::{Either3, select3};
use embassy_sync::signal::Signal;
use trouble_host::prelude::*;
use trouble_host::{BondInformation, LongTermKey};
#[cfg(feature = "storage")]
use {crate::channel::FLASH_CHANNEL, crate::storage::FLASH_OPERATION_FINISHED};

use super::ble_server::CCCD_TABLE_SIZE;
use crate::NUM_BLE_PROFILE;
use crate::channel::BLE_PROFILE_CHANNEL;
use crate::state::{current_profile, set_ble_profile};

pub(crate) static UPDATED_PROFILE: Signal<crate::RawMutex, ProfileInfo> = Signal::new();
pub(crate) static UPDATED_CCCD_TABLE: Signal<crate::RawMutex, CccdTable<CCCD_TABLE_SIZE>> = Signal::new();

/// BLE profile info
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ProfileInfo {
    pub(crate) slot_num: u8,
    pub(crate) removed: bool,
    #[serde(with = "bond_info_serde")]
    pub(crate) info: BondInformation,
    #[serde(with = "cccd_table_serde")]
    pub(crate) cccd_table: CccdTable<CCCD_TABLE_SIZE>,
}

// Custom serde module for BondInformation
pub(crate) mod bond_info_serde {
    use serde::{Deserializer, Serialize, Serializer};

    use super::*;

    pub fn serialize<S>(info: &BondInformation, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let tuple = (
            info.ltk.to_le_bytes(),
            info.identity.bd_addr.into_inner(),
            info.identity.irk.map(|k| k.to_le_bytes()),
            match info.security_level {
                SecurityLevel::NoEncryption => 0u8,
                SecurityLevel::Encrypted => 1u8,
                SecurityLevel::EncryptedAuthenticated => 2u8,
            },
            info.is_bonded,
        );
        tuple.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<BondInformation, D::Error>
    where
        D: Deserializer<'de>,
    {
        let (ltk, bd_addr, irk, security_level, is_bonded): ([u8; 16], [u8; 6], Option<[u8; 16]>, u8, bool) =
            serde::Deserialize::deserialize(deserializer)?;

        Ok(BondInformation::new(
            Identity {
                bd_addr: BdAddr::new(bd_addr),
                irk: irk.map(IdentityResolvingKey::from_le_bytes),
            },
            LongTermKey::from_le_bytes(ltk),
            match security_level {
                0 => SecurityLevel::NoEncryption,
                1 => SecurityLevel::Encrypted,
                _ => SecurityLevel::EncryptedAuthenticated,
            },
            is_bonded,
        ))
    }
}

// Custom serde module for CccdTable
pub(crate) mod cccd_table_serde {
    use serde::{Deserializer, Serialize, Serializer};

    use super::*;

    pub fn serialize<S>(table: &CccdTable<CCCD_TABLE_SIZE>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut entries = [(0u16, 0u16); CCCD_TABLE_SIZE];

        for (i, entry) in table.inner().iter().enumerate() {
            entries[i] = (entry.0, entry.1.raw());
        }
        entries.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<CccdTable<CCCD_TABLE_SIZE>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let entries: [(u16, u16); CCCD_TABLE_SIZE] = serde::Deserialize::deserialize(deserializer)?;
        let mut cccd_values = [(0u16, CCCD::default()); CCCD_TABLE_SIZE];
        for i in 0..CCCD_TABLE_SIZE {
            cccd_values[i] = (entries[i].0, entries[i].1.into());
        }
        Ok(CccdTable::new(cccd_values))
    }
}

/// Returns the maximum number of bytes required to encode T.
pub const fn varint_max<T: Sized>() -> usize {
    const BITS_PER_BYTE: usize = 8;
    const BITS_PER_VARINT_BYTE: usize = 7;

    // How many data bits do we need for this type?
    let bits = core::mem::size_of::<T>() * BITS_PER_BYTE;

    // We add (BITS_PER_VARINT_BYTE - 1), to ensure any integer divisions
    // with a remainder will always add exactly one full byte, but
    // an evenly divided number of bits will be the same
    let roundup_bits = bits + (BITS_PER_VARINT_BYTE - 1);

    // Apply division, using normal "round down" integer division
    roundup_bits / BITS_PER_VARINT_BYTE
}

// Manual MaxSize implementation
impl postcard::experimental::max_size::MaxSize for ProfileInfo {
    const POSTCARD_MAX_SIZE: usize = varint_max::<Self>();
}

impl Default for ProfileInfo {
    fn default() -> Self {
        Self {
            slot_num: 0,
            removed: false,
            info: BondInformation::new(
                Identity {
                    bd_addr: BdAddr::default(),
                    irk: None,
                },
                LongTermKey(0),
                SecurityLevel::NoEncryption,
                false,
            ),
            cccd_table: CccdTable::<CCCD_TABLE_SIZE>::default(),
        }
    }
}

/// BLE profile switch action.
///
/// `ClearBond` carries an explicit slot index; pass `current_profile()` from
/// the caller when the user means "clear the active profile".
pub(crate) enum BleProfileAction {
    Switch(u8),
    Previous,
    Next,
    ClearBond(u8),
}

/// Manage BLE profiles and bonding information
///
/// ProfileManager is responsible for:
/// 1. Managing multiple BLE profiles, allowing users to switch between multiple devices
/// 2. Storing and loading bonding information for each profile
/// 3. Updating the bonding information of the active profile to the BLE stack
/// 4. Handling profile switch, clear, and save operations
#[cfg(feature = "_ble")]
pub(crate) struct ProfileManager<'a, C: Controller + ControllerCmdAsync<LeSetPhy>, P: PacketPool> {
    /// List of bonded devices
    bonded_devices: heapless::Vec<ProfileInfo, NUM_BLE_PROFILE>,
    /// BLE stack
    stack: &'a Stack<'a, C, P>,
}

#[cfg(feature = "_ble")]
impl<'a, C: Controller + ControllerCmdAsync<LeSetPhy>, P: PacketPool> ProfileManager<'a, C, P> {
    /// Create a new profile manager
    pub(crate) fn new(stack: &'a Stack<'a, C, P>) -> Self {
        Self {
            bonded_devices: heapless::Vec::new(),
            stack,
        }
    }

    /// Load stored bonding information
    #[cfg(feature = "storage")]
    pub(crate) async fn load_bonded_devices(&mut self) {
        use crate::storage::{read_active_ble_profile, read_bond_info};

        self.bonded_devices.clear();
        for slot_num in 0..NUM_BLE_PROFILE {
            if let Some(info) = read_bond_info(slot_num as u8).await
                && !info.removed
                && let Err(e) = self.bonded_devices.push(info)
            {
                error!("Failed to add bond info: {:?}", e);
            }
        }
        debug!("Loaded {} bond info", self.bonded_devices.len());

        let profile = if let Some(profile) = read_active_ble_profile().await {
            debug!("Loaded active profile: {}", profile);
            profile
        } else {
            debug!("Loaded default active profile",);
            0
        };
        set_ble_profile(profile);
    }

    /// Cached bond info for the currently active profile, cloned to free the
    /// caller from borrow conflicts with concurrent `update_profile()`.
    pub(crate) fn active_bond_info(&self) -> Option<ProfileInfo> {
        let active_profile = current_profile();
        self.bonded_devices
            .iter()
            .find(|bond_info| !bond_info.removed && bond_info.slot_num == active_profile)
            .cloned()
    }

    /// Update bonding information in the stack according to the current active profile
    pub(crate) fn update_stack_bonds(&self) {
        let current_bond_info = self.stack.get_bond_information();
        for bond in current_bond_info {
            if let Err(e) = self.stack.remove_bond_information(bond.identity) {
                debug!("Remove bond info error: {:?}", e);
            }
        }

        if let Some(info) = self.active_bond_info() {
            debug!("Add bond info of profile {}: {:?}", info.slot_num, info);
            if let Err(e) = self.stack.add_bond_information(info.info) {
                debug!("Add bond info error: {:?}", e);
            }
        }
    }

    /// Add/update bonding information
    pub(crate) async fn add_profile_info(&mut self, profile_info: ProfileInfo) {
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

        self.update_stack_bonds();

        #[cfg(feature = "storage")]
        // Send bonding information to the flash task for saving
        FLASH_CHANNEL
            .send(crate::storage::FlashOperationMessage::ProfileInfo(profile_info))
            .await;
    }

    /// Update CCCD table in the stack
    pub(crate) async fn update_profile_cccd_table(&mut self, table: CccdTable<CCCD_TABLE_SIZE>) {
        // Get current active profile
        let active_profile = current_profile();

        // Update profile information in memory
        if let Some(index) = self
            .bonded_devices
            .iter()
            .position(|info| info.slot_num == active_profile)
        {
            if self.bonded_devices[index].cccd_table.inner() == table.inner() {
                debug!("Skip updating same CCCD table");
                return;
            }

            debug!("Updating profile {} CCCD table: {:?}", active_profile, table);
            self.bonded_devices[index].cccd_table = table;

            #[cfg(feature = "storage")]
            FLASH_CHANNEL
                .send(crate::storage::FlashOperationMessage::ProfileInfo(
                    self.bonded_devices[index].clone(),
                ))
                .await;
        } else {
            error!("Failed to update profile CCCD table: profile not found");
        }
    }

    /// Clear bonding information of the specified slot
    pub(crate) async fn clear_bond(&mut self, slot_num: u8) {
        info!("Clearing bonding information on profile: {}", slot_num);

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
    pub(crate) async fn switch_profile(&mut self, profile: u8) -> bool {
        let current = current_profile();
        if profile == current {
            return false;
        }

        set_ble_profile(profile);

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
    pub(crate) async fn update_profile(&mut self) {
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
                    FLASH_OPERATION_FINISHED.reset();
                    match action {
                        BleProfileAction::Switch(profile) => {
                            if !self.switch_profile(profile).await {
                                // If the profile is the same as the current profile, do nothing
                                continue;
                            }
                        }
                        BleProfileAction::Previous => {
                            let mut profile = current_profile();
                            profile = if profile == 0 {
                                NUM_BLE_PROFILE as u8 - 1
                            } else {
                                profile - 1
                            };

                            self.switch_profile(profile).await;
                        }
                        BleProfileAction::Next => {
                            let mut profile = current_profile() + 1;
                            profile %= NUM_BLE_PROFILE as u8;

                            self.switch_profile(profile).await;
                        }
                        BleProfileAction::ClearBond(slot) => {
                            self.clear_bond(slot).await;
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
