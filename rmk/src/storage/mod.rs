use core::fmt::Debug;

use embassy_embedded_hal::adapter::BlockingAsync;
use embassy_sync::signal::Signal;
use embassy_time::Duration;
use embedded_storage::nor_flash::NorFlash;
use embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash;
use postcard::experimental::max_size::MaxSize;
use rmk_types::morse::MorseProfile;
use sequential_storage::Error as SSError;
use sequential_storage::cache::NoCache;
use sequential_storage::map::{Key, MapConfig, MapStorage, PostcardValue, SerializationError};
#[cfg(feature = "_ble")]
use {
    crate::ble::{ble_server::CCCD_TABLE_SIZE, profile::ProfileInfo},
    trouble_host::prelude::CccdTable,
};
#[cfg(feature = "host")]
use {
    crate::{MACRO_SPACE_SIZE, combo::ComboConfig, morse::Morse},
    rmk_types::action::{EncoderAction, KeyAction},
    rmk_types::fork::Fork,
};

use crate::channel::FLASH_CHANNEL;
use crate::config::StorageConfig;
#[cfg(all(feature = "_ble", feature = "split"))]
use crate::split::ble::PeerAddress;
use crate::{BUILD_HASH, config};

/// Signal to synchronize the flash operation status, usually used outside of the flash task.
/// True if the flash operation is finished correctly, false if the flash operation is finished with error.
pub(crate) static FLASH_OPERATION_FINISHED: Signal<crate::RawMutex, bool> = Signal::new();

// Message send from other tasks, which will do saving or clearing operation
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) enum FlashOperationMessage {
    #[cfg(feature = "_ble")]
    // BLE profile info to be saved
    ProfileInfo(ProfileInfo),
    #[cfg(feature = "_ble")]
    // Current active BLE profile number
    ActiveBleProfile(u8),
    #[cfg(all(feature = "_ble", feature = "split"))]
    // Peer address
    PeerAddress(PeerAddress),
    // Clear the storage
    Reset,
    // Clear the layout info
    ResetLayout,
    // Clear info of given slot number
    ClearSlot(u8),
    // Layout option
    LayoutOptions(u32),
    // Default layer number
    DefaultLayer(u8),
    #[cfg(feature = "host")]
    MacroData([u8; MACRO_SPACE_SIZE]),
    #[cfg(feature = "host")]
    KeymapKey {
        layer: u8,
        row: u8,
        col: u8,
        action: KeyAction,
    },
    #[cfg(feature = "host")]
    Encoder {
        layer: u8,
        idx: u8,
        action: EncoderAction,
    },
    #[cfg(feature = "host")]
    Combo {
        idx: u8,
        config: ComboConfig,
    },
    #[cfg(feature = "host")]
    Fork {
        idx: u8,
        fork: Fork,
    },
    #[cfg(feature = "host")]
    Morse {
        idx: u8,
        morse: Morse,
    },
    // Current saved connection type
    ConnectionType(u8),
    // Timeout time for combos
    ComboTimeout(u16),
    // Timeout time for one-shot keys
    OneShotTimeout(u16),
    // Interval for tap actions
    TapInterval(u16),
    // Interval for tapping capslock
    TapCapslockInterval(u16),
    // The prior-idle-time in ms used for in flow tap
    PriorIdleTime(u16),
    // Default morse profile containing all morse/tap-hold settings (mode, timeouts, unilateral_tap)
    MorseDefaultProfile(MorseProfile),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) enum StorageKey {
    StorageConfig,
    LayoutConfig,
    BehaviorConfig,
    ConnectionType,
    #[cfg(feature = "host")]
    MacroData,
    #[cfg(feature = "host")]
    Keymap {
        layer: u8,
        row: u8,
        col: u8,
    },
    #[cfg(feature = "host")]
    Encoder {
        layer: u8,
        idx: u8,
    },
    #[cfg(feature = "host")]
    Combo(u8),
    #[cfg(feature = "host")]
    Fork(u8),
    #[cfg(feature = "host")]
    Morse(u8),
    #[cfg(all(feature = "_ble", feature = "split"))]
    PeerAddress(u8),
    #[cfg(feature = "_ble")]
    ActiveBleProfile,
    #[cfg(feature = "_ble")]
    BondInfo(u8),
}

impl StorageKey {
    #[cfg(feature = "host")]
    pub(crate) const fn keymap(layer: u8, row: u8, col: u8) -> Self {
        Self::Keymap { layer, row, col }
    }

    #[cfg(feature = "_ble")]
    pub(crate) const fn bond_info(slot_num: u8) -> Self {
        Self::BondInfo(slot_num)
    }

    #[cfg(feature = "host")]
    pub(crate) const fn combo(idx: u8) -> Self {
        Self::Combo(idx)
    }

    #[cfg(feature = "host")]
    pub(crate) const fn encoder(idx: u8, layer: u8) -> Self {
        Self::Encoder { layer, idx }
    }

    #[cfg(feature = "host")]
    pub(crate) const fn fork(idx: u8) -> Self {
        Self::Fork(idx)
    }

    #[cfg(all(feature = "_ble", feature = "split"))]
    pub(crate) const fn peer_address(peer_id: u8) -> Self {
        Self::PeerAddress(peer_id)
    }

    #[cfg(feature = "host")]
    pub(crate) const fn morse(idx: u8) -> Self {
        Self::Morse(idx)
    }
}

impl Key for StorageKey {
    fn serialize_into(&self, buffer: &mut [u8]) -> Result<usize, SerializationError> {
        postcard::to_slice(self, buffer)
            .map(|used| used.len())
            .map_err(Into::into)
    }

    fn deserialize_from(buffer: &[u8]) -> Result<(Self, usize), SerializationError> {
        let (key, rest): (Self, &[u8]) = postcard::take_from_bytes(buffer).map_err(SerializationError::from)?;
        Ok((key, buffer.len() - rest.len()))
    }

    fn get_len(buffer: &[u8]) -> Result<usize, SerializationError> {
        Self::deserialize_from(buffer).map(|(_, len)| len)
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) enum StorageData {
    StorageConfig(LocalStorageConfig),
    LayoutConfig(LayoutConfig),
    BehaviorConfig(BehaviorConfig),
    ConnectionType(u8),
    #[cfg(feature = "host")]
    MacroData(#[serde(with = "crate::host::storage::macro_bytes_serde")] [u8; MACRO_SPACE_SIZE]),
    #[cfg(feature = "host")]
    KeyAction(KeyAction),
    #[cfg(feature = "host")]
    EncoderAction(EncoderAction),
    #[cfg(feature = "host")]
    Combo(ComboConfig),
    #[cfg(feature = "host")]
    Fork(Fork),
    #[cfg(feature = "host")]
    Morse(Morse),
    #[cfg(all(feature = "_ble", feature = "split"))]
    PeerAddress(PeerAddress),
    #[cfg(feature = "_ble")]
    BondInfo(ProfileInfo),
    #[cfg(feature = "_ble")]
    ActiveBleProfile(u8),
}

impl<'a> PostcardValue<'a> for StorageData {}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct LocalStorageConfig {
    enable: bool,
    build_hash: u32,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct LayoutConfig {
    default_layer: u8,
    layout_option: u32,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct BehaviorConfig {
    // The prior-idle-time in ms used for in flow tap
    pub(crate) prior_idle_time: u16,
    // Default morse profile containing mode, timeouts, and unilateral_tap settings
    pub(crate) morse_default_profile: MorseProfile,

    // Timeout time for combos
    pub(crate) combo_timeout: u16,
    // Timeout time for one-shot keys
    pub(crate) one_shot_timeout: u16,
    // Interval for tap actions
    pub(crate) tap_interval: u16,
    // Interval for tapping capslock.
    // macOS has special processing of capslock, when tapping capslock, the tap interval should be another value
    pub(crate) tap_capslock_interval: u16,
}

impl From<LocalStorageConfig> for StorageData {
    fn from(config: LocalStorageConfig) -> Self {
        Self::StorageConfig(config)
    }
}

impl From<LayoutConfig> for StorageData {
    fn from(config: LayoutConfig) -> Self {
        Self::LayoutConfig(config)
    }
}

impl From<&config::BehaviorConfig> for StorageData {
    fn from(behavior: &config::BehaviorConfig) -> Self {
        Self::BehaviorConfig(BehaviorConfig {
            prior_idle_time: behavior.morse.prior_idle_time.as_millis() as u16,
            morse_default_profile: behavior.morse.default_profile,
            combo_timeout: behavior.combo.timeout.as_millis() as u16,
            one_shot_timeout: behavior.one_shot.timeout.as_millis() as u16,
            tap_interval: behavior.tap.tap_interval,
            tap_capslock_interval: behavior.tap.tap_capslock_interval,
        })
    }
}

pub fn async_flash_wrapper<F: NorFlash>(flash: F) -> BlockingAsync<F> {
    embassy_embedded_hal::adapter::BlockingAsync::new(flash)
}

#[cfg(feature = "split")]
pub async fn new_storage_for_split_peripheral<F: AsyncNorFlash>(
    flash: F,
    storage_config: StorageConfig,
) -> Storage<F, 0, 0, 0, 0> {
    Storage::<F, 0, 0, 0, 0>::new(
        flash,
        #[cfg(feature = "host")]
        &[],
        #[cfg(feature = "host")]
        &None,
        &storage_config,
        &config::BehaviorConfig::default(),
    )
    .await
}

pub struct Storage<
    F: AsyncNorFlash,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize = 0,
> {
    pub(crate) flash: MapStorage<StorageKey, F, NoCache>,
    pub(crate) buffer: [u8; get_buffer_size()],
}

/// Read out storage config, update and then save back.
/// This macro applies to only some of the configs.
macro_rules! update_storage_field {
    ($f: expr, $buf: expr, $key:ident, $field:ident) => {{
        let key = StorageKey::$key;
        if let Ok(Some(StorageData::$key(mut saved))) = $f.fetch_item($buf, &key).await {
            saved.$field = $field;
            $f.store_item($buf, &key, &StorageData::$key(saved)).await
        } else {
            Ok(())
        }
    }};
}

impl<F: AsyncNorFlash, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    async fn fetch_data(&mut self, key: StorageKey) -> Result<Option<StorageData>, SSError<F::Error>> {
        self.flash.fetch_item(&mut self.buffer, &key).await
    }

    async fn store_data(&mut self, key: StorageKey, data: &StorageData) -> Result<(), SSError<F::Error>> {
        self.flash.store_item(&mut self.buffer, &key, data).await
    }

    pub async fn new(
        flash: F,
        #[cfg(feature = "host")] keymap: &[[[KeyAction; COL]; ROW]; NUM_LAYER],
        #[cfg(feature = "host")] encoder_map: &Option<&mut [[EncoderAction; NUM_ENCODER]; NUM_LAYER]>,
        storage_config: &StorageConfig,
        behavior_config: &config::BehaviorConfig,
    ) -> Self {
        // Check storage setting
        assert!(
            storage_config.num_sectors >= 2,
            "Number of used sector for storage must larger than 1"
        );

        // If config.start_addr == 0:
        // - For nRF chips: use sectors starting at 0x0006_0000
        // - For other chips: use the last `num_sectors` sectors
        // Otherwise, use storage config setting
        #[cfg(feature = "_nrf_ble")]
        let start_addr = if storage_config.start_addr == 0 {
            0x0006_0000
        } else {
            storage_config.start_addr
        };

        #[cfg(not(feature = "_nrf_ble"))]
        let start_addr = storage_config.start_addr;

        // Check storage setting
        info!(
            "Flash capacity {} KB, RMK use {} KB({} sectors) starting from 0x{:X} as storage",
            flash.capacity() / 1024,
            (F::ERASE_SIZE * storage_config.num_sectors as usize) / 1024,
            storage_config.num_sectors,
            storage_config.start_addr,
        );

        let storage_range = if start_addr == 0 {
            (flash.capacity() - storage_config.num_sectors as usize * F::ERASE_SIZE) as u32..flash.capacity() as u32
        } else {
            assert!(
                start_addr.is_multiple_of(F::ERASE_SIZE),
                "Storage's start addr MUST BE a multiplier of sector size"
            );
            start_addr as u32..(start_addr + storage_config.num_sectors as usize * F::ERASE_SIZE) as u32
        };

        let mut storage = Self {
            flash: MapStorage::new(flash, MapConfig::new(storage_range), NoCache::new()),
            buffer: [0; get_buffer_size()],
        };

        // Check whether keymap and configs have been storaged in flash
        if !storage.check_enable().await || storage_config.clear_storage {
            // Clear storage first
            debug!("Clearing storage!");
            let _ = storage.flash.erase_all().await;

            // Initialize storage from keymap and config
            if storage
                .initialize_storage_with_config(
                    #[cfg(feature = "host")]
                    keymap,
                    #[cfg(feature = "host")]
                    encoder_map,
                    behavior_config,
                )
                .await
                .is_err()
            {
                // When there's an error, `enable: false` should be saved back to storage, preventing partial initialization of storage
                storage
                    .store_data(
                        StorageKey::StorageConfig,
                        &StorageData::from(LocalStorageConfig {
                            enable: false,
                            build_hash: BUILD_HASH,
                        }),
                    )
                    .await
                    .ok();
            }
        } else if storage_config.clear_layout {
            #[cfg(feature = "host")]
            {
                debug!("clear_layout=true; overwriting layout items without erase.");
                let encoder_map = encoder_map.as_ref().map(|m| &**m);
                let _ = storage.reset_layout_only(keymap, &encoder_map, behavior_config).await;
            }
        }

        storage
    }

    pub(crate) async fn run(&mut self) {
        loop {
            let info: FlashOperationMessage = FLASH_CHANNEL.receive().await;
            debug!("Flash operation: {:?}", info);
            match match info {
                FlashOperationMessage::LayoutOptions(layout_option) => {
                    // Read out layout options, update layer option and save back
                    update_storage_field!(&mut self.flash, &mut self.buffer, LayoutConfig, layout_option)
                }
                FlashOperationMessage::Reset => self.flash.erase_all().await,
                FlashOperationMessage::ResetLayout => {
                    info!("Ignoring ResetLayout at runtime (handled at startup via clear_layout).");
                    Ok(())
                }
                FlashOperationMessage::DefaultLayer(default_layer) => {
                    // Read out layout options, update layer option and save back
                    update_storage_field!(&mut self.flash, &mut self.buffer, LayoutConfig, default_layer)
                }
                #[cfg(feature = "host")]
                FlashOperationMessage::MacroData(data) => {
                    self.store_data(StorageKey::MacroData, &StorageData::MacroData(data))
                        .await
                }
                #[cfg(feature = "host")]
                FlashOperationMessage::KeymapKey {
                    layer,
                    row,
                    col,
                    action,
                } => {
                    self.store_data(StorageKey::keymap(layer, row, col), &StorageData::KeyAction(action))
                        .await
                }
                #[cfg(feature = "host")]
                FlashOperationMessage::Encoder { layer, idx, action } => {
                    self.store_data(StorageKey::encoder(idx, layer), &StorageData::EncoderAction(action))
                        .await
                }
                #[cfg(feature = "host")]
                FlashOperationMessage::Combo { idx, config } => {
                    self.store_data(StorageKey::combo(idx), &StorageData::Combo(config))
                        .await
                }
                #[cfg(feature = "host")]
                FlashOperationMessage::Fork { idx, fork } => {
                    self.store_data(StorageKey::fork(idx), &StorageData::Fork(fork)).await
                }
                #[cfg(feature = "host")]
                FlashOperationMessage::Morse { idx, morse } => {
                    self.store_data(StorageKey::morse(idx), &StorageData::Morse(morse))
                        .await
                }
                FlashOperationMessage::ConnectionType(ty) => {
                    self.store_data(StorageKey::ConnectionType, &StorageData::ConnectionType(ty))
                        .await
                }
                #[cfg(all(feature = "_ble", feature = "split"))]
                FlashOperationMessage::PeerAddress(peer) => {
                    self.store_data(StorageKey::peer_address(peer.peer_id), &StorageData::PeerAddress(peer))
                        .await
                }
                #[cfg(feature = "_ble")]
                FlashOperationMessage::ActiveBleProfile(profile) => {
                    self.store_data(StorageKey::ActiveBleProfile, &StorageData::ActiveBleProfile(profile))
                        .await
                }
                #[cfg(feature = "_ble")]
                FlashOperationMessage::ClearSlot(slot_num) => {
                    use bt_hci::param::BdAddr;
                    use trouble_host::prelude::{CCCD, SecurityLevel};
                    use trouble_host::{BondInformation, Identity, LongTermKey};

                    info!("Clearing bond info slot_num: {}", slot_num);
                    // Remove item in `sequential-storage` is quite expensive, so just override the item with `removed = true`
                    let empty = ProfileInfo {
                        removed: true,
                        slot_num,
                        info: BondInformation::new(
                            Identity {
                                bd_addr: BdAddr::new([0; 6]),
                                irk: None,
                            },
                            LongTermKey::from_le_bytes([0; 16]),
                            SecurityLevel::NoEncryption,
                            false,
                        ),
                        cccd_table: CccdTable::new([(0u16, CCCD::default()); CCCD_TABLE_SIZE]),
                    };
                    self.store_data(StorageKey::bond_info(slot_num), &StorageData::BondInfo(empty))
                        .await
                }
                #[cfg(feature = "_ble")]
                FlashOperationMessage::ProfileInfo(b) => {
                    debug!("Saving profile info: {:?}", b);
                    self.store_data(StorageKey::bond_info(b.slot_num), &StorageData::BondInfo(b))
                        .await
                }
                FlashOperationMessage::ComboTimeout(combo_timeout) => {
                    update_storage_field!(&mut self.flash, &mut self.buffer, BehaviorConfig, combo_timeout)
                }
                FlashOperationMessage::OneShotTimeout(one_shot_timeout) => {
                    update_storage_field!(&mut self.flash, &mut self.buffer, BehaviorConfig, one_shot_timeout)
                }
                FlashOperationMessage::TapInterval(tap_interval) => {
                    update_storage_field!(&mut self.flash, &mut self.buffer, BehaviorConfig, tap_interval)
                }
                FlashOperationMessage::TapCapslockInterval(tap_capslock_interval) => {
                    update_storage_field!(&mut self.flash, &mut self.buffer, BehaviorConfig, tap_capslock_interval)
                }
                FlashOperationMessage::PriorIdleTime(prior_idle_time) => {
                    update_storage_field!(&mut self.flash, &mut self.buffer, BehaviorConfig, prior_idle_time)
                }
                FlashOperationMessage::MorseDefaultProfile(morse_default_profile) => {
                    update_storage_field!(&mut self.flash, &mut self.buffer, BehaviorConfig, morse_default_profile)
                }
                #[cfg(not(feature = "_ble"))]
                _ => Ok(()),
            } {
                Err(e) => {
                    print_storage_error::<F>(e);
                    FLASH_OPERATION_FINISHED.signal(false);
                }
                _ => {
                    FLASH_OPERATION_FINISHED.signal(true);
                }
            }
        }
    }

    pub(crate) async fn read_behavior_config(
        &mut self,
        behavior_config: &mut config::BehaviorConfig,
    ) -> Result<(), ()> {
        if let Some(StorageData::BehaviorConfig(c)) = self
            .fetch_data(StorageKey::BehaviorConfig)
            .await
            .map_err(|e| print_storage_error::<F>(e))?
        {
            behavior_config.morse.prior_idle_time = Duration::from_millis(c.prior_idle_time as u64);
            behavior_config.morse.default_profile = c.morse_default_profile;

            behavior_config.combo.timeout = Duration::from_millis(c.combo_timeout as u64);
            behavior_config.one_shot.timeout = Duration::from_millis(c.one_shot_timeout as u64);
            behavior_config.tap.tap_interval = c.tap_interval;
            behavior_config.tap.tap_capslock_interval = c.tap_capslock_interval;
        }

        Ok(())
    }

    async fn initialize_storage_with_config(
        &mut self,
        #[cfg(feature = "host")] keymap: &[[[KeyAction; COL]; ROW]; NUM_LAYER],
        #[cfg(feature = "host")] encoder_map: &Option<&mut [[EncoderAction; NUM_ENCODER]; NUM_LAYER]>,
        behavior: &config::BehaviorConfig,
    ) -> Result<(), ()> {
        // Save storage config
        self.store_data(
            StorageKey::StorageConfig,
            &StorageData::from(LocalStorageConfig {
                enable: true,
                build_hash: BUILD_HASH,
            }),
        )
        .await
        .map_err(|e| print_storage_error::<F>(e))?;

        // Save layout config
        self.store_data(
            StorageKey::LayoutConfig,
            &StorageData::from(LayoutConfig {
                default_layer: 0,
                layout_option: 0,
            }),
        )
        .await
        .map_err(|e| print_storage_error::<F>(e))?;

        // Save behavior config
        self.store_data(StorageKey::BehaviorConfig, &StorageData::from(behavior))
            .await
            .map_err(|e| print_storage_error::<F>(e))?;

        #[cfg(feature = "host")]
        for (layer, layer_data) in keymap.iter().enumerate() {
            for (row, row_data) in layer_data.iter().enumerate() {
                for (col, action) in row_data.iter().enumerate() {
                    self.store_data(
                        StorageKey::keymap(layer as u8, row as u8, col as u8),
                        &StorageData::KeyAction(*action),
                    )
                    .await
                    .map_err(|e| print_storage_error::<F>(e))?;
                }
            }
        }

        // Save encoder configurations
        #[cfg(feature = "host")]
        if let Some(encoder_map) = encoder_map {
            for (layer, layer_data) in encoder_map.iter().enumerate() {
                for (idx, action) in layer_data.iter().enumerate() {
                    self.store_data(
                        StorageKey::encoder(idx as u8, layer as u8),
                        &StorageData::EncoderAction(*action),
                    )
                    .await
                    .map_err(|e| print_storage_error::<F>(e))?;
                }
            }
        }

        Ok(())
    }

    #[cfg(feature = "host")]
    async fn reset_layout_only(
        &mut self,
        keymap: &[[[KeyAction; COL]; ROW]; NUM_LAYER],
        encoder_map: &Option<&[[EncoderAction; NUM_ENCODER]; NUM_LAYER]>,
        behavior: &config::BehaviorConfig,
    ) -> Result<(), SSError<F::Error>> {
        self.store_data(
            StorageKey::LayoutConfig,
            &StorageData::from(LayoutConfig {
                default_layer: 0,
                layout_option: 0,
            }),
        )
        .await?;
        self.store_data(StorageKey::BehaviorConfig, &StorageData::from(behavior))
            .await?;

        // TODO: Generic reset for vial and other hosts
        for (layer, layer_data) in keymap.iter().enumerate() {
            for (row, row_data) in layer_data.iter().enumerate() {
                for (col, action) in row_data.iter().enumerate() {
                    self.store_data(
                        StorageKey::keymap(layer as u8, row as u8, col as u8),
                        &StorageData::KeyAction(*action),
                    )
                    .await?;
                }
            }
        }

        // TODO: Generic reset for vial and other hosts
        if let Some(encoder_map) = encoder_map {
            for (layer, layer_data) in encoder_map.iter().enumerate() {
                for (idx, action) in layer_data.iter().enumerate() {
                    self.store_data(
                        StorageKey::encoder(idx as u8, layer as u8),
                        &StorageData::EncoderAction(*action),
                    )
                    .await?;
                }
            }
        }

        Ok(())
    }

    async fn check_enable(&mut self) -> bool {
        if let Ok(Some(StorageData::StorageConfig(config))) = self.fetch_data(StorageKey::StorageConfig).await {
            if config.enable && config.build_hash == BUILD_HASH {
                return true;
            }
        }
        false
    }

    #[cfg(feature = "_ble")]
    pub(crate) async fn read_trouble_bond_info(&mut self, slot_num: u8) -> Result<Option<ProfileInfo>, ()> {
        let read_data = self
            .fetch_data(StorageKey::bond_info(slot_num))
            .await
            .map_err(|e| print_storage_error::<F>(e))?;

        Ok(match read_data {
            Some(StorageData::BondInfo(info)) => Some(info),
            _ => None,
        })
    }

    #[cfg(all(feature = "_ble", feature = "split"))]
    pub async fn read_peer_address(&mut self, peer_id: u8) -> Result<Option<PeerAddress>, ()> {
        let read_data = self
            .fetch_data(StorageKey::peer_address(peer_id))
            .await
            .map_err(|e| print_storage_error::<F>(e))?;

        Ok(match read_data {
            Some(StorageData::PeerAddress(data)) => Some(data),
            _ => None,
        })
    }

    #[cfg(all(feature = "_ble", feature = "split"))]
    pub async fn write_peer_address(&mut self, peer_address: PeerAddress) -> Result<(), ()> {
        let key = StorageKey::peer_address(peer_address.peer_id);
        let item = StorageData::PeerAddress(peer_address);

        self.store_data(key, &item)
            .await
            .map_err(|e| print_storage_error::<F>(e))
    }
}

pub(crate) fn print_storage_error<F: AsyncNorFlash>(e: SSError<F::Error>) {
    match e {
        #[cfg(feature = "defmt")]
        SSError::Storage { value: e } => error!("Flash error: {:?}", defmt::Debug2Format(&e)),
        #[cfg(not(feature = "defmt"))]
        SSError::Storage { value: _e } => error!("Flash error"),
        SSError::FullStorage => error!("Storage is full"),
        SSError::Corrupted {} => error!("Storage is corrupted"),
        SSError::BufferTooBig => error!("Buffer too big"),
        SSError::BufferTooSmall(x) => error!("Buffer too small, needs {} bytes", x),
        SSError::SerializationError(e) => error!("Map value error: {}", e),
        SSError::ItemTooBig => error!("Item too big"),
        _ => error!("Unknown storage error"),
    }
}

const fn get_buffer_size() -> usize {
    #[cfg(feature = "host")]
    {
        // The buffer size needed = size_of(StorageData) = MACRO_SPACE_SIZE + 8(generally)
        // According to doc of `sequential-storage`, for some flashes it should be aligned in 32 bytes
        // To make sure the buffer works, do this alignment always
        let buffer_size = if crate::MACRO_SPACE_SIZE < 248 {
            256
        } else {
            crate::MACRO_SPACE_SIZE + 8
        };

        // Efficiently round up to the nearest multiple of 32 using bit manipulation.
        (buffer_size + 31) & !31
    }

    #[cfg(not(feature = "host"))]
    256
}

#[macro_export]
/// Helper macro for reading storage config
macro_rules! read_storage {
    ($storage: ident, $key: expr, $buf: expr) => {
        $storage.flash.fetch_item(&mut $buf, $key).await
    };
}

#[cfg(test)]
mod tests {
    use embassy_futures::block_on;
    use sequential_storage::cache::NoCache;
    use sequential_storage::map::{MapConfig, MapStorage};

    use super::*;
    use crate::config::{BehaviorConfig as RuntimeBehaviorConfig, StorageConfig as RuntimeStorageConfig};

    #[derive(Debug, Clone, Copy)]
    struct TestFlashError;

    impl embedded_storage_async::nor_flash::NorFlashError for TestFlashError {
        fn kind(&self) -> embedded_storage_async::nor_flash::NorFlashErrorKind {
            embedded_storage_async::nor_flash::NorFlashErrorKind::Other
        }
    }

    struct TestFlash<const SIZE: usize, const ERASE_SIZE: usize, const WRITE_SIZE: usize> {
        bytes: [u8; SIZE],
    }

    impl<const SIZE: usize, const ERASE_SIZE: usize, const WRITE_SIZE: usize> TestFlash<SIZE, ERASE_SIZE, WRITE_SIZE> {
        fn new() -> Self {
            Self { bytes: [0xFF; SIZE] }
        }
    }

    impl<const SIZE: usize, const ERASE_SIZE: usize, const WRITE_SIZE: usize> embedded_storage::nor_flash::ErrorType
        for TestFlash<SIZE, ERASE_SIZE, WRITE_SIZE>
    {
        type Error = TestFlashError;
    }

    impl<const SIZE: usize, const ERASE_SIZE: usize, const WRITE_SIZE: usize> embedded_storage::nor_flash::ReadNorFlash
        for TestFlash<SIZE, ERASE_SIZE, WRITE_SIZE>
    {
        const READ_SIZE: usize = 1;

        fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
            let start = offset as usize;
            let end = start + bytes.len();
            bytes.copy_from_slice(&self.bytes[start..end]);
            Ok(())
        }

        fn capacity(&self) -> usize {
            SIZE
        }
    }

    impl<const SIZE: usize, const ERASE_SIZE: usize, const WRITE_SIZE: usize> embedded_storage::nor_flash::NorFlash
        for TestFlash<SIZE, ERASE_SIZE, WRITE_SIZE>
    {
        const WRITE_SIZE: usize = WRITE_SIZE;
        const ERASE_SIZE: usize = ERASE_SIZE;

        fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
            self.bytes[from as usize..to as usize].fill(0xFF);
            Ok(())
        }

        fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
            let start = offset as usize;
            let end = start + bytes.len();
            for (dst, src) in self.bytes[start..end].iter_mut().zip(bytes.iter()) {
                *dst &= *src;
            }
            Ok(())
        }
    }

    impl<const SIZE: usize, const ERASE_SIZE: usize, const WRITE_SIZE: usize>
        embedded_storage_async::nor_flash::ReadNorFlash for TestFlash<SIZE, ERASE_SIZE, WRITE_SIZE>
    {
        const READ_SIZE: usize = 1;

        async fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
            embedded_storage::nor_flash::ReadNorFlash::read(self, offset, bytes)
        }

        fn capacity(&self) -> usize {
            SIZE
        }
    }

    impl<const SIZE: usize, const ERASE_SIZE: usize, const WRITE_SIZE: usize>
        embedded_storage_async::nor_flash::NorFlash for TestFlash<SIZE, ERASE_SIZE, WRITE_SIZE>
    {
        const WRITE_SIZE: usize = WRITE_SIZE;
        const ERASE_SIZE: usize = ERASE_SIZE;

        async fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
            embedded_storage::nor_flash::NorFlash::erase(self, from, to)
        }

        async fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
            embedded_storage::nor_flash::NorFlash::write(self, offset, bytes)
        }
    }

    #[cfg(feature = "host")]
    #[test]
    fn storage_key_round_trip() {
        let cases = [
            StorageKey::StorageConfig,
            StorageKey::LayoutConfig,
            StorageKey::BehaviorConfig,
            StorageKey::ConnectionType,
            StorageKey::MacroData,
            StorageKey::Keymap {
                layer: 2,
                row: 3,
                col: 4,
            },
            StorageKey::Encoder { layer: 1, idx: 5 },
            StorageKey::Combo(6),
            StorageKey::Fork(7),
            StorageKey::Morse(8),
        ];

        let mut buffer = [0u8; 64];
        for key in cases {
            let size = <StorageKey as Key>::serialize_into(&key, &mut buffer).unwrap();
            let (decoded, used) = <StorageKey as Key>::deserialize_from(&buffer[..size]).unwrap();
            assert_eq!(decoded, key);
            assert_eq!(used, size);
        }
    }

    #[cfg(feature = "host")]
    #[test]
    fn build_hash_mismatch_reinitializes_storage() {
        block_on(async {
            type Flash = TestFlash<16_384, 4_096, 1>;

            let storage_range = (16_384 - 2 * 4_096) as u32..16_384u32;
            let mut map =
                MapStorage::<StorageKey, _, _>::new(Flash::new(), MapConfig::new(storage_range), NoCache::new());
            let mut buffer = [0u8; 256];

            map.store_item(
                &mut buffer,
                &StorageKey::StorageConfig,
                &StorageData::StorageConfig(LocalStorageConfig {
                    enable: true,
                    build_hash: BUILD_HASH.wrapping_sub(1),
                }),
            )
            .await
            .unwrap();
            map.store_item(
                &mut buffer,
                &StorageKey::LayoutConfig,
                &StorageData::LayoutConfig(LayoutConfig {
                    default_layer: 7,
                    layout_option: 42,
                }),
            )
            .await
            .unwrap();

            let (flash, _) = map.destroy();
            let keymap = [[[KeyAction::No; 1]; 1]; 1];
            let encoder_map: Option<&mut [[EncoderAction; 0]; 1]> = None;

            let mut storage = Storage::<Flash, 1, 1, 1, 0>::new(
                flash,
                &keymap,
                &encoder_map,
                &RuntimeStorageConfig::default(),
                &RuntimeBehaviorConfig::default(),
            )
            .await;

            let stored_layout = storage.fetch_data(StorageKey::LayoutConfig).await.unwrap();
            let stored_config = storage.fetch_data(StorageKey::StorageConfig).await.unwrap();

            assert!(matches!(
                stored_layout,
                Some(StorageData::LayoutConfig(LayoutConfig {
                    default_layer: 0,
                    layout_option: 0,
                }))
            ));
            assert!(matches!(
                stored_config,
                Some(StorageData::StorageConfig(LocalStorageConfig {
                    enable: true,
                    build_hash: BUILD_HASH,
                }))
            ));
        });
    }
}
