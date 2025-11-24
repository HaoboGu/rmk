pub mod dummy_flash;

use core::fmt::Debug;
use core::ops::Range;

use embassy_embedded_hal::adapter::BlockingAsync;
use embassy_sync::signal::Signal;
use embassy_time::Duration;
use embedded_storage::nor_flash::NorFlash;
use embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash;
use sequential_storage::Error as SSError;
use sequential_storage::cache::NoCache;
use sequential_storage::map::{SerializationError, Value, fetch_item, store_item};
#[cfg(feature = "host")]
use {
    crate::host::storage::{KeymapData, KeymapKey},
    rmk_types::action::{EncoderAction, KeyAction},
};

#[cfg(feature = "_ble")]
use crate::ble::profile::ProfileInfo;
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
    // Vial Flash Message
    #[cfg(feature = "host")]
    VialMessage(KeymapData),
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
    // Timeout time for morse keys in default morse profile
    MorseHoldTimeout(u16),
    // Whether the unilateral tap is enabled in default morse profile
    UnilateralTap(bool),
}

/// StorageKeys is the prefix digit stored in the flash, it's used to identify the type of the stored data.
///
/// This is because the whole storage item is an Rust enum due to the limitation of `sequential_storage`.
/// When deserializing, we need to know the type of the stored data to know how to parse it, the first byte of the stored data is always the type, aka StorageKeys.
#[repr(u32)]
pub(crate) enum StorageKeys {
    StorageConfig = 0,
    #[cfg(feature = "host")]
    KeymapConfig = 1,
    LayoutConfig = 2,
    BehaviorConfig = 3,
    #[cfg(feature = "host")]
    MacroData = 4,
    #[cfg(feature = "host")]
    ComboData = 5,
    ConnectionType = 6,
    #[cfg(feature = "host")]
    EncoderKeys = 7,
    #[cfg(feature = "host")]
    ForkData = 8,
    #[cfg(feature = "host")]
    MorseData = 9,
    #[cfg(all(feature = "_ble", feature = "split"))]
    PeerAddress = 0xED,
    #[cfg(feature = "_ble")]
    ActiveBleProfile = 0xEE,
    #[cfg(feature = "_ble")]
    BleBondInfo = 0xEF,
}

impl StorageKeys {
    pub(crate) fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(StorageKeys::StorageConfig),
            #[cfg(feature = "host")]
            1 => Some(StorageKeys::KeymapConfig),
            2 => Some(StorageKeys::LayoutConfig),
            3 => Some(StorageKeys::BehaviorConfig),
            #[cfg(feature = "host")]
            4 => Some(StorageKeys::MacroData),
            #[cfg(feature = "host")]
            5 => Some(StorageKeys::ComboData),
            6 => Some(StorageKeys::ConnectionType),
            #[cfg(feature = "host")]
            7 => Some(StorageKeys::EncoderKeys),
            #[cfg(feature = "host")]
            8 => Some(StorageKeys::ForkData),
            #[cfg(feature = "host")]
            9 => Some(StorageKeys::MorseData),
            #[cfg(all(feature = "_ble", feature = "split"))]
            0xED => Some(StorageKeys::PeerAddress),
            #[cfg(feature = "_ble")]
            0xEE => Some(StorageKeys::ActiveBleProfile),
            #[cfg(feature = "_ble")]
            0xEF => Some(StorageKeys::BleBondInfo),
            _ => None,
        }
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) enum StorageData {
    StorageConfig(LocalStorageConfig),
    LayoutConfig(LayoutConfig),
    BehaviorConfig(BehaviorConfig),
    ConnectionType(u8),
    #[cfg(feature = "host")]
    VialData(KeymapData),
    #[cfg(all(feature = "_ble", feature = "split"))]
    PeerAddress(PeerAddress),
    #[cfg(feature = "_ble")]
    BondInfo(ProfileInfo),
    #[cfg(feature = "_ble")]
    ActiveBleProfile(u8),
}

/// Get the key to retrieve the keymap key from the storage.
#[cfg(feature = "host")]
pub(crate) fn get_keymap_key<const ROW: usize, const COL: usize, const NUM_LAYER: usize>(
    keymap_key: &KeymapKey,
) -> u32 {
    0x1000 + (keymap_key.layer as usize * COL * ROW + keymap_key.row as usize * COL + keymap_key.col as usize) as u32
}

/// Get the key to retrieve the bond info from the storage.
pub(crate) fn get_bond_info_key(slot_num: u8) -> u32 {
    0x2000 + slot_num as u32
}

/// Get the key to retrieve the combo from the storage.
#[cfg(feature = "host")]
pub(crate) fn get_combo_key(idx: u8) -> u32 {
    0x3000 + idx as u32
}

/// Get the key to retrieve the encoder config from the storage.
#[cfg(feature = "host")]
pub(crate) fn get_encoder_config_key<const NUM_ENCODER: usize>(idx: u8, layer: u8) -> u32 {
    0x4000 + (idx as usize + NUM_ENCODER * layer as usize) as u32
}

#[cfg(feature = "host")]
pub(crate) fn get_fork_key(idx: u8) -> u32 {
    0x5000 + idx as u32
}

/// Get the key to retrieve the peer address from the storage.
pub(crate) fn get_peer_address_key(peer_id: u8) -> u32 {
    0x6000 + peer_id as u32
}

/// Get the key to retrieve the tap dance from the storage.
#[cfg(feature = "host")]
pub(crate) fn get_morse_key(idx: u8) -> u32 {
    0x7000 + idx as u32
}

/// Convert postcard::Error to SerializationError
pub(crate) fn postcard_error_to_serialization_error(e: postcard::Error) -> SerializationError {
    match e {
        postcard::Error::SerializeBufferFull => SerializationError::BufferTooSmall,
        postcard::Error::DeserializeUnexpectedEnd
        | postcard::Error::DeserializeBadVarint
        | postcard::Error::DeserializeBadBool
        | postcard::Error::DeserializeBadChar
        | postcard::Error::DeserializeBadUtf8
        | postcard::Error::DeserializeBadOption
        | postcard::Error::DeserializeBadEnum
        | postcard::Error::DeserializeBadEncoding => SerializationError::InvalidFormat,
        // Other errors with debug info
        _ => {
            #[cfg(feature = "defmt")]
            {
                defmt::error!("Unexpected postcard error: {:?}", defmt::Debug2Format(&e));
            }
            SerializationError::Custom(1)
        }
    }
}

/// Macro to serialize standard variants: key + postcard-serialized data
/// Used by both StorageData and KeymapData
#[macro_export]
macro_rules! ser_storage_variant {
    ($buffer:expr, $key:expr, $data:expr) => {{
        $buffer[0] = $key as u8;
        let len = postcard::to_slice($data, &mut $buffer[1..])
            .map_err($crate::storage::postcard_error_to_serialization_error)?
            .len();
        Ok(len + 1)
    }};
}

// Helper methods for StorageData
impl StorageData {
    /// Get the StorageKey for this variant (used by store_item calls)
    const fn key(&self) -> u32 {
        match self {
            Self::StorageConfig(_) => StorageKeys::StorageConfig as u32,
            Self::LayoutConfig(_) => StorageKeys::LayoutConfig as u32,
            Self::BehaviorConfig(_) => StorageKeys::BehaviorConfig as u32,
            Self::ConnectionType(_) => StorageKeys::ConnectionType as u32,
            #[cfg(all(feature = "_ble", feature = "split"))]
            Self::PeerAddress(_) => StorageKeys::PeerAddress as u32,
            #[cfg(feature = "_ble")]
            Self::BondInfo(_) => StorageKeys::BleBondInfo as u32,
            #[cfg(feature = "_ble")]
            Self::ActiveBleProfile(_) => StorageKeys::ActiveBleProfile as u32,
            #[cfg(feature = "host")]
            Self::VialData(_) => {
                // VialData variants have their own keys managed elsewhere
                // This shouldn't be called for VialData, use specific key functions instead
                0
            }
        }
    }
}

impl Value<'_> for StorageData {
    fn serialize_into(&self, buffer: &mut [u8]) -> Result<usize, SerializationError> {
        if buffer.is_empty() {
            return Err(SerializationError::BufferTooSmall);
        }

        match self {
            Self::StorageConfig(d) => ser_storage_variant!(buffer, StorageKeys::StorageConfig, d),
            Self::LayoutConfig(d) => ser_storage_variant!(buffer, StorageKeys::LayoutConfig, d),
            Self::BehaviorConfig(d) => ser_storage_variant!(buffer, StorageKeys::BehaviorConfig, d),
            Self::ConnectionType(d) => ser_storage_variant!(buffer, StorageKeys::ConnectionType, d),
            #[cfg(all(feature = "_ble", feature = "split"))]
            Self::PeerAddress(d) => ser_storage_variant!(buffer, StorageKeys::PeerAddress, d),
            #[cfg(feature = "_ble")]
            Self::BondInfo(d) => ser_storage_variant!(buffer, StorageKeys::BleBondInfo, d),
            #[cfg(feature = "_ble")]
            Self::ActiveBleProfile(d) => ser_storage_variant!(buffer, StorageKeys::ActiveBleProfile, d),
            #[cfg(feature = "host")]
            Self::VialData(vial_data) => vial_data.serialize_into(buffer),
        }
    }

    fn deserialize_from(buffer: &[u8]) -> Result<Self, SerializationError>
    where
        Self: Sized,
    {
        if buffer.is_empty() {
            return Err(SerializationError::InvalidFormat);
        }

        let key = StorageKeys::from_u8(buffer[0]).ok_or(SerializationError::InvalidFormat)?;

        match key {
            StorageKeys::StorageConfig => Ok(Self::StorageConfig(
                postcard::from_bytes(&buffer[1..]).map_err(postcard_error_to_serialization_error)?,
            )),
            StorageKeys::LayoutConfig => Ok(Self::LayoutConfig(
                postcard::from_bytes(&buffer[1..]).map_err(postcard_error_to_serialization_error)?,
            )),
            StorageKeys::BehaviorConfig => Ok(Self::BehaviorConfig(
                postcard::from_bytes(&buffer[1..]).map_err(postcard_error_to_serialization_error)?,
            )),
            StorageKeys::ConnectionType => Ok(Self::ConnectionType(
                postcard::from_bytes(&buffer[1..]).map_err(postcard_error_to_serialization_error)?,
            )),
            #[cfg(all(feature = "_ble", feature = "split"))]
            StorageKeys::PeerAddress => Ok(Self::PeerAddress(
                postcard::from_bytes(&buffer[1..]).map_err(postcard_error_to_serialization_error)?,
            )),
            #[cfg(feature = "_ble")]
            StorageKeys::BleBondInfo => Ok(Self::BondInfo(
                postcard::from_bytes(&buffer[1..]).map_err(postcard_error_to_serialization_error)?,
            )),
            #[cfg(feature = "_ble")]
            StorageKeys::ActiveBleProfile => Ok(Self::ActiveBleProfile(
                postcard::from_bytes(&buffer[1..]).map_err(postcard_error_to_serialization_error)?,
            )),
            #[cfg(feature = "host")]
            StorageKeys::KeymapConfig
            | StorageKeys::MacroData
            | StorageKeys::ComboData
            | StorageKeys::EncoderKeys
            | StorageKeys::ForkData
            | StorageKeys::MorseData => {
                // VialData keys handled by KeymapData
                KeymapData::deserialize_from(buffer).map(Self::VialData)
            }
        }
    }
}
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize, postcard::experimental::max_size::MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct LocalStorageConfig {
    enable: bool,
    build_hash: u32,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize, postcard::experimental::max_size::MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct LayoutConfig {
    default_layer: u8,
    layout_option: u32,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize, postcard::experimental::max_size::MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct BehaviorConfig {
    // Enable flow tap for morse/tap-hold
    //pub(crate) enable_flow_tap: bool,
    // The prior-idle-time in ms used for in flow tap
    pub(crate) prior_idle_time: u16,
    // morse/tap-hold defaults
    pub(crate) morse_hold_timeout_ms: u16,
    pub(crate) unilateral_tap: bool,

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
    pub(crate) flash: F,
    pub(crate) storage_range: Range<u32>,
    pub(crate) buffer: [u8; get_buffer_size()],
}

/// Read out storage config, update and then save back.
/// This macro applies to only some of the configs.
macro_rules! update_storage_field {
    ($f: expr, $buf: expr, $cache:expr, $key:ident, $field:ident, $range:expr) => {
        if let Ok(Some(StorageData::$key(mut saved))) =
            fetch_item::<u32, StorageData, _>($f, $range, $cache, $buf, &(StorageKeys::$key as u32)).await
        {
            saved.$field = $field;
            store_item::<u32, StorageData, _>(
                $f,
                $range,
                $cache,
                $buf,
                &(StorageKeys::$key as u32),
                &StorageData::$key(saved),
            )
            .await
        } else {
            Ok(())
        }
    };
}

impl<F: AsyncNorFlash, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
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

        // If config.start_addr == 0, use last `num_sectors` sectors or sectors begin at 0x0006_0000 for nRF52
        // Other wise, use storage config setting
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
            flash,
            storage_range,
            buffer: [0; get_buffer_size()],
        };

        // Check whether keymap and configs have been storaged in flash
        if !storage.check_enable().await || storage_config.clear_storage {
            // Clear storage first
            debug!("Clearing storage!");
            let _ = sequential_storage::erase_all(&mut storage.flash, storage.storage_range.clone()).await;

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
                store_item(
                    &mut storage.flash,
                    storage.storage_range.clone(),
                    &mut NoCache::new(),
                    &mut storage.buffer,
                    &(StorageKeys::StorageConfig as u32),
                    &StorageData::StorageConfig(LocalStorageConfig {
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
        let mut storage_cache = NoCache::new();
        loop {
            let info: FlashOperationMessage = FLASH_CHANNEL.receive().await;
            debug!("Flash operation: {:?}", info);
            match match info {
                FlashOperationMessage::LayoutOptions(layout_option) => {
                    // Read out layout options, update layer option and save back
                    update_storage_field!(
                        &mut self.flash,
                        &mut self.buffer,
                        &mut storage_cache,
                        LayoutConfig,
                        layout_option,
                        self.storage_range.clone()
                    )
                }
                FlashOperationMessage::Reset => {
                    sequential_storage::erase_all(&mut self.flash, self.storage_range.clone()).await
                }
                FlashOperationMessage::ResetLayout => {
                    info!("Ignoring ResetLayout at runtime (handled at startup via clear_layout).");
                    Ok(())
                }
                FlashOperationMessage::DefaultLayer(default_layer) => {
                    // Read out layout options, update layer option and save back
                    update_storage_field!(
                        &mut self.flash,
                        &mut self.buffer,
                        &mut storage_cache,
                        LayoutConfig,
                        default_layer,
                        self.storage_range.clone()
                    )
                }
                #[cfg(feature = "host")]
                FlashOperationMessage::VialMessage(vial_data) => match vial_data {
                    KeymapData::Macro(macro_data) => {
                        info!("Saving keyboard macro data");
                        store_item(
                            &mut self.flash,
                            self.storage_range.clone(),
                            &mut storage_cache,
                            &mut self.buffer,
                            &(StorageKeys::MacroData as u32),
                            &StorageData::VialData(KeymapData::Macro(macro_data)),
                        )
                        .await
                    }
                    KeymapData::KeymapKey(keymap_key) => {
                        let key = get_keymap_key::<ROW, COL, NUM_LAYER>(&keymap_key);
                        let data = StorageData::VialData(KeymapData::KeymapKey(keymap_key));
                        store_item(
                            &mut self.flash,
                            self.storage_range.clone(),
                            &mut storage_cache,
                            &mut self.buffer,
                            &key,
                            &data,
                        )
                        .await
                    }
                    KeymapData::Encoder(encoder_config) => {
                        let data = KeymapData::Encoder(encoder_config);
                        let key = get_encoder_config_key::<NUM_ENCODER>(encoder_config.idx, encoder_config.layer);
                        store_item(
                            &mut self.flash,
                            self.storage_range.clone(),
                            &mut storage_cache,
                            &mut self.buffer,
                            &key,
                            &data,
                        )
                        .await
                    }
                    KeymapData::Combo(idx, config) => {
                        let key = get_combo_key(idx);
                        store_item(
                            &mut self.flash,
                            self.storage_range.clone(),
                            &mut storage_cache,
                            &mut self.buffer,
                            &key,
                            &StorageData::VialData(KeymapData::Combo(idx, config)),
                        )
                        .await
                    }
                    KeymapData::Fork(idx, fork) => {
                        store_item(
                            &mut self.flash,
                            self.storage_range.clone(),
                            &mut storage_cache,
                            &mut self.buffer,
                            &get_fork_key(idx),
                            &StorageData::VialData(KeymapData::Fork(idx, fork)),
                        )
                        .await
                    }
                    KeymapData::Morse(id, morse) => {
                        store_item(
                            &mut self.flash,
                            self.storage_range.clone(),
                            &mut storage_cache,
                            &mut self.buffer,
                            &get_morse_key(id),
                            &StorageData::VialData(KeymapData::Morse(id, morse)),
                        )
                        .await
                    }
                },
                FlashOperationMessage::ConnectionType(ty) => {
                    store_item(
                        &mut self.flash,
                        self.storage_range.clone(),
                        &mut storage_cache,
                        &mut self.buffer,
                        &(StorageKeys::ConnectionType as u32),
                        &StorageData::ConnectionType(ty),
                    )
                    .await
                }
                #[cfg(all(feature = "_ble", feature = "split"))]
                FlashOperationMessage::PeerAddress(peer) => {
                    let key = get_peer_address_key(peer.peer_id);
                    let data = StorageData::PeerAddress(peer);
                    store_item(
                        &mut self.flash,
                        self.storage_range.clone(),
                        &mut storage_cache,
                        &mut self.buffer,
                        &key,
                        &data,
                    )
                    .await
                }
                #[cfg(feature = "_ble")]
                FlashOperationMessage::ActiveBleProfile(profile) => {
                    let data = StorageData::ActiveBleProfile(profile);
                    store_item::<u32, StorageData, _>(
                        &mut self.flash,
                        self.storage_range.clone(),
                        &mut storage_cache,
                        &mut self.buffer,
                        &(StorageKeys::ActiveBleProfile as u32),
                        &data,
                    )
                    .await
                }
                #[cfg(feature = "_ble")]
                FlashOperationMessage::ClearSlot(slot_num) => {
                    use bt_hci::param::BdAddr;
                    use trouble_host::prelude::{CCCD, CccdTable, SecurityLevel};
                    use trouble_host::{BondInformation, Identity, LongTermKey};

                    use crate::ble::ble_server::CCCD_TABLE_SIZE;

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
                    let data = StorageData::BondInfo(empty);
                    store_item::<u32, StorageData, _>(
                        &mut self.flash,
                        self.storage_range.clone(),
                        &mut storage_cache,
                        &mut self.buffer,
                        &get_bond_info_key(slot_num),
                        &data,
                    )
                    .await
                }
                #[cfg(feature = "_ble")]
                FlashOperationMessage::ProfileInfo(b) => {
                    debug!("Saving profile info: {:?}", b);
                    let data = StorageData::BondInfo(b.clone());
                    store_item::<u32, StorageData, _>(
                        &mut self.flash,
                        self.storage_range.clone(),
                        &mut storage_cache,
                        &mut self.buffer,
                        &get_bond_info_key(b.slot_num),
                        &data,
                    )
                    .await
                }
                FlashOperationMessage::MorseHoldTimeout(morse_hold_timeout_ms) => update_storage_field!(
                    &mut self.flash,
                    &mut self.buffer,
                    &mut storage_cache,
                    BehaviorConfig,
                    morse_hold_timeout_ms,
                    self.storage_range.clone()
                ),
                FlashOperationMessage::ComboTimeout(combo_timeout) => update_storage_field!(
                    &mut self.flash,
                    &mut self.buffer,
                    &mut storage_cache,
                    BehaviorConfig,
                    combo_timeout,
                    self.storage_range.clone()
                ),
                FlashOperationMessage::OneShotTimeout(one_shot_timeout) => update_storage_field!(
                    &mut self.flash,
                    &mut self.buffer,
                    &mut storage_cache,
                    BehaviorConfig,
                    one_shot_timeout,
                    self.storage_range.clone()
                ),
                FlashOperationMessage::TapInterval(tap_interval) => update_storage_field!(
                    &mut self.flash,
                    &mut self.buffer,
                    &mut storage_cache,
                    BehaviorConfig,
                    tap_interval,
                    self.storage_range.clone()
                ),
                FlashOperationMessage::TapCapslockInterval(tap_capslock_interval) => update_storage_field!(
                    &mut self.flash,
                    &mut self.buffer,
                    &mut storage_cache,
                    BehaviorConfig,
                    tap_capslock_interval,
                    self.storage_range.clone()
                ),
                FlashOperationMessage::PriorIdleTime(prior_idle_time) => update_storage_field!(
                    &mut self.flash,
                    &mut self.buffer,
                    &mut storage_cache,
                    BehaviorConfig,
                    prior_idle_time,
                    self.storage_range.clone()
                ),
                FlashOperationMessage::UnilateralTap(unilateral_tap) => update_storage_field!(
                    &mut self.flash,
                    &mut self.buffer,
                    &mut storage_cache,
                    BehaviorConfig,
                    unilateral_tap,
                    self.storage_range.clone()
                ),
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
        if let Some(StorageData::BehaviorConfig(c)) = fetch_item::<u32, StorageData, _>(
            &mut self.flash,
            self.storage_range.clone(),
            &mut NoCache::new(),
            &mut self.buffer,
            &(StorageKeys::BehaviorConfig as u32),
        )
        .await
        .map_err(|e| print_storage_error::<F>(e))?
        {
            behavior_config.morse.prior_idle_time = Duration::from_millis(c.prior_idle_time as u64);
            behavior_config.morse.default_profile = behavior_config
                .morse
                .default_profile
                .with_hold_timeout_ms(Some(c.morse_hold_timeout_ms))
                .with_unilateral_tap(Some(c.unilateral_tap));

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
        let mut cache = NoCache::new();
        // Save storage config
        let storage_config = StorageData::StorageConfig(LocalStorageConfig {
            enable: true,
            build_hash: BUILD_HASH,
        });
        store_item(
            &mut self.flash,
            self.storage_range.clone(),
            &mut cache,
            &mut self.buffer,
            &storage_config.key(),
            &storage_config,
        )
        .await
        .map_err(|e| print_storage_error::<F>(e))?;

        // Save layout config
        let layout_config = StorageData::LayoutConfig(LayoutConfig {
            default_layer: 0,
            layout_option: 0,
        });
        store_item(
            &mut self.flash,
            self.storage_range.clone(),
            &mut cache,
            &mut self.buffer,
            &layout_config.key(),
            &layout_config,
        )
        .await
        .map_err(|e| print_storage_error::<F>(e))?;

        // Save behavior config
        let behavior_config = StorageData::BehaviorConfig(BehaviorConfig {
            prior_idle_time: behavior.morse.prior_idle_time.as_millis() as u16,
            morse_hold_timeout_ms: behavior.morse.default_profile.hold_timeout_ms().unwrap_or(0),
            unilateral_tap: behavior.morse.default_profile.unilateral_tap().unwrap_or(false),

            combo_timeout: behavior.combo.timeout.as_millis() as u16,
            one_shot_timeout: behavior.one_shot.timeout.as_millis() as u16,
            tap_interval: behavior.tap.tap_interval,
            tap_capslock_interval: behavior.tap.tap_capslock_interval,
        });

        store_item(
            &mut self.flash,
            self.storage_range.clone(),
            &mut cache,
            &mut self.buffer,
            &behavior_config.key(),
            &behavior_config,
        )
        .await
        .map_err(|e| print_storage_error::<F>(e))?;

        #[cfg(feature = "host")]
        for (layer, layer_data) in keymap.iter().enumerate() {
            for (row, row_data) in layer_data.iter().enumerate() {
                for (col, action) in row_data.iter().enumerate() {
                    let keymap_key = KeymapKey {
                        row: row as u8,
                        col: col as u8,
                        layer: layer as u8,
                        action: *action,
                    };
                    store_item(
                        &mut self.flash,
                        self.storage_range.clone(),
                        &mut cache,
                        &mut self.buffer,
                        &get_keymap_key::<ROW, COL, NUM_LAYER>(&keymap_key),
                        &StorageData::VialData(KeymapData::KeymapKey(keymap_key)),
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
                    use crate::host::storage::EncoderKeymap;

                    let encoder = EncoderKeymap {
                        idx: idx as u8,
                        layer: layer as u8,
                        action: *action,
                    };
                    store_item(
                        &mut self.flash,
                        self.storage_range.clone(),
                        &mut cache,
                        &mut self.buffer,
                        &get_encoder_config_key::<NUM_ENCODER>(encoder.idx, encoder.layer),
                        &StorageData::VialData(KeymapData::Encoder(encoder)),
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
        let mut cache = NoCache::new();

        let layout_config = StorageData::LayoutConfig(LayoutConfig {
            default_layer: 0,
            layout_option: 0,
        });
        store_item(
            &mut self.flash,
            self.storage_range.clone(),
            &mut cache,
            &mut self.buffer,
            &layout_config.key(),
            &layout_config,
        )
        .await?;

        let behavior_config = StorageData::BehaviorConfig(BehaviorConfig {
            prior_idle_time: behavior.morse.prior_idle_time.as_millis() as u16,
            morse_hold_timeout_ms: behavior.morse.default_profile.hold_timeout_ms().unwrap_or(0),
            unilateral_tap: behavior.morse.default_profile.unilateral_tap().unwrap_or(false),

            combo_timeout: behavior.combo.timeout.as_millis() as u16,
            one_shot_timeout: behavior.one_shot.timeout.as_millis() as u16,
            tap_interval: behavior.tap.tap_interval,
            tap_capslock_interval: behavior.tap.tap_capslock_interval,
        });
        store_item(
            &mut self.flash,
            self.storage_range.clone(),
            &mut cache,
            &mut self.buffer,
            &behavior_config.key(),
            &behavior_config,
        )
        .await?;

        // TODO: Generic reset for vial and other hosts
        for (layer, layer_data) in keymap.iter().enumerate() {
            for (row, row_data) in layer_data.iter().enumerate() {
                for (col, action) in row_data.iter().enumerate() {
                    let keymap_key = KeymapKey {
                        row: row as u8,
                        col: col as u8,
                        layer: layer as u8,
                        action: *action,
                    };
                    store_item(
                        &mut self.flash,
                        self.storage_range.clone(),
                        &mut cache,
                        &mut self.buffer,
                        &get_keymap_key::<ROW, COL, NUM_LAYER>(&keymap_key),
                        &StorageData::VialData(KeymapData::KeymapKey(keymap_key)),
                    )
                    .await?;
                }
            }
        }

        // TODO: Generic reset for vial and other hosts
        if let Some(encoder_map) = encoder_map {
            for (layer, layer_data) in encoder_map.iter().enumerate() {
                for (idx, action) in layer_data.iter().enumerate() {
                    use crate::host::storage::EncoderKeymap;
                    store_item(
                        &mut self.flash,
                        self.storage_range.clone(),
                        &mut cache,
                        &mut self.buffer,
                        &get_encoder_config_key::<NUM_ENCODER>(idx as u8, layer as u8),
                        &StorageData::VialData(KeymapData::Encoder(EncoderKeymap {
                            idx: idx as u8,
                            layer: layer as u8,
                            action: *action,
                        })),
                    )
                    .await?;
                }
            }
        }

        Ok(())
    }

    async fn check_enable(&mut self) -> bool {
        if let Ok(Some(StorageData::StorageConfig(config))) = fetch_item::<u32, StorageData, _>(
            &mut self.flash,
            self.storage_range.clone(),
            &mut NoCache::new(),
            &mut self.buffer,
            &(StorageKeys::StorageConfig as u32),
        )
        .await
        {
            // if config.enable && config.build_hash == BUILD_HASH {
            if config.enable {
                return true;
            }
        }
        false
    }

    #[cfg(feature = "_ble")]
    pub(crate) async fn read_trouble_bond_info(&mut self, slot_num: u8) -> Result<Option<ProfileInfo>, ()> {
        let read_data = fetch_item::<u32, StorageData, _>(
            &mut self.flash,
            self.storage_range.clone(),
            &mut NoCache::new(),
            &mut self.buffer,
            &get_bond_info_key(slot_num),
        )
        .await
        .map_err(|e| print_storage_error::<F>(e))?;

        if let Some(StorageData::BondInfo(info)) = read_data {
            Ok(Some(info))
        } else {
            Ok(None)
        }
    }

    #[cfg(all(feature = "_ble", feature = "split"))]
    pub async fn read_peer_address(&mut self, peer_id: u8) -> Result<Option<PeerAddress>, ()> {
        let read_data = fetch_item::<u32, StorageData, _>(
            &mut self.flash,
            self.storage_range.clone(),
            &mut NoCache::new(),
            &mut self.buffer,
            &get_peer_address_key(peer_id),
        )
        .await
        .map_err(|e| print_storage_error::<F>(e))?;

        if let Some(StorageData::PeerAddress(data)) = read_data {
            Ok(Some(data))
        } else {
            Ok(None)
        }
    }

    #[cfg(all(feature = "_ble", feature = "split"))]
    pub async fn write_peer_address(&mut self, peer_address: PeerAddress) -> Result<(), ()> {
        let peer_id = peer_address.peer_id;
        let item = StorageData::PeerAddress(peer_address);

        store_item(
            &mut self.flash,
            self.storage_range.clone(),
            &mut NoCache::new(),
            &mut self.buffer,
            &get_peer_address_key(peer_id),
            &item,
        )
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
        ::sequential_storage::map::fetch_item::<u32, $crate::storage::StorageData, _>(
            &mut $storage.flash,
            $storage.storage_range.clone(),
            &mut sequential_storage::cache::NoCache::new(),
            &mut $buf,
            $key,
        )
        .await
    };
}
