pub mod dummy_flash;
mod eeconfig;

use crate::{
    channel::FLASH_CHANNEL,
    combo::{Combo, COMBO_MAX_LENGTH},
    config::StorageConfig,
    BUILD_HASH,
};
use byteorder::{BigEndian, ByteOrder};
use core::fmt::Debug;
use core::ops::Range;
use embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash;
use heapless::Vec;
use sequential_storage::{
    cache::NoCache,
    map::{fetch_all_items, fetch_item, store_item, SerializationError, Value},
    Error as SSError,
};
#[cfg(feature = "_nrf_ble")]
use {crate::ble::nrf::bonder::BondInfo, core::mem};

use crate::keyboard_macro::MACRO_SPACE_SIZE;
use crate::{
    action::KeyAction,
    via::keycode_convert::{from_via_keycode, to_via_keycode},
};

use self::eeconfig::EeKeymapConfig;

// Message send from bonder to flash task, which will do saving or clearing operation
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) enum FlashOperationMessage {
    // Bond info to be saved
    #[cfg(feature = "_nrf_ble")]
    BondInfo(BondInfo),
    // Current active BLE profile number
    #[cfg(feature = "_nrf_ble")]
    ActiveBleProfile(u8),
    // Clear the storage
    Reset,
    // Clear info of given slot number
    ClearSlot(u8),
    // Layout option
    LayoutOptions(u32),
    // Default layer number
    DefaultLayer(u8),
    // Write macro
    WriteMacro([u8; MACRO_SPACE_SIZE]),
    KeymapKey {
        layer: u8,
        col: u8,
        row: u8,
        action: KeyAction,
    },
    // Current saved connection type
    ConnectionType(u8),
    // Write combo
    WriteCombo(ComboData),
}

#[repr(u32)]
pub(crate) enum StorageKeys {
    StorageConfig,
    LedLightConfig,
    RgbLightConfig,
    KeymapConfig,
    LayoutConfig,
    KeymapKeys,
    MacroData,
    ComboData,
    ConnectionType,
    #[cfg(feature = "_nrf_ble")]
    ActiveBleProfile = 0xEE,
    #[cfg(feature = "_nrf_ble")]
    BleBondInfo = 0xEF,
}

impl StorageKeys {
    pub(crate) fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(StorageKeys::StorageConfig),
            1 => Some(StorageKeys::LedLightConfig),
            2 => Some(StorageKeys::RgbLightConfig),
            3 => Some(StorageKeys::KeymapConfig),
            4 => Some(StorageKeys::LayoutConfig),
            5 => Some(StorageKeys::KeymapKeys),
            6 => Some(StorageKeys::MacroData),
            7 => Some(StorageKeys::ComboData),
            #[cfg(feature = "_nrf_ble")]
            0xEF => Some(StorageKeys::BleBondInfo),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum StorageData {
    StorageConfig(LocalStorageConfig),
    LayoutConfig(LayoutConfig),
    KeymapConfig(EeKeymapConfig),
    KeymapKey(KeymapKey),
    MacroData([u8; MACRO_SPACE_SIZE]),
    ComboData(ComboData),
    ConnectionType(u8),
    #[cfg(feature = "_nrf_ble")]
    BondInfo(BondInfo),
    #[cfg(feature = "_nrf_ble")]
    ActiveBleProfile(u8),
}

pub(crate) fn get_bond_info_key(slot_num: u8) -> u32 {
    0x2000 + slot_num as u32
}

pub(crate) fn get_keymap_key<const ROW: usize, const COL: usize, const NUM_LAYER: usize>(
    row: usize,
    col: usize,
    layer: usize,
) -> u32 {
    (0x1000 + layer * COL * ROW + row * COL + col) as u32
}

pub(crate) fn get_combo_key(idx: usize) -> u32 {
    (0x3000 + idx) as u32
}

impl Value<'_> for StorageData {
    fn serialize_into(&self, buffer: &mut [u8]) -> Result<usize, SerializationError> {
        if buffer.len() < 6 {
            return Err(SerializationError::BufferTooSmall);
        }
        match self {
            StorageData::StorageConfig(c) => {
                buffer[0] = StorageKeys::StorageConfig as u8;
                // If enabled, write 0 to flash.
                if c.enable {
                    buffer[1] = 0;
                } else {
                    buffer[1] = 1;
                }
                // Save build_hash
                BigEndian::write_u32(&mut buffer[2..6], c.build_hash);
                Ok(6)
            }
            StorageData::LayoutConfig(c) => {
                buffer[0] = StorageKeys::LayoutConfig as u8;
                buffer[1] = c.default_layer;
                BigEndian::write_u32(&mut buffer[2..6], c.layout_option);
                Ok(6)
            }
            StorageData::KeymapConfig(c) => {
                buffer[0] = StorageKeys::KeymapConfig as u8;
                let bits = c.into_bits();
                BigEndian::write_u16(&mut buffer[1..3], bits);
                Ok(3)
            }
            StorageData::KeymapKey(k) => {
                buffer[0] = StorageKeys::KeymapKeys as u8;
                BigEndian::write_u16(&mut buffer[1..3], to_via_keycode(k.action));
                buffer[3] = k.layer as u8;
                buffer[4] = k.col as u8;
                buffer[5] = k.row as u8;
                Ok(6)
            }
            StorageData::MacroData(d) => {
                if buffer.len() < MACRO_SPACE_SIZE + 1 {
                    return Err(SerializationError::BufferTooSmall);
                }
                buffer[0] = StorageKeys::MacroData as u8;
                buffer[1..MACRO_SPACE_SIZE + 1].copy_from_slice(d);
                Ok(MACRO_SPACE_SIZE + 1)
            }
            StorageData::ComboData(combo) => {
                if buffer.len() < 11 {
                    return Err(SerializationError::BufferTooSmall);
                }
                buffer[0] = StorageKeys::ComboData as u8;
                for i in 0..4 {
                    BigEndian::write_u16(
                        &mut buffer[1 + i * 2..3 + i * 2],
                        to_via_keycode(combo.actions[i]),
                    );
                }
                BigEndian::write_u16(&mut buffer[9..11], to_via_keycode(combo.output));
                Ok(11)
            }
            StorageData::ConnectionType(ty) => {
                buffer[0] = StorageKeys::ConnectionType as u8;
                buffer[1] = *ty;
                Ok(2)
            }
            #[cfg(feature = "_nrf_ble")]
            StorageData::BondInfo(b) => {
                if buffer.len() < 121 {
                    return Err(SerializationError::BufferTooSmall);
                }

                // Must be 120
                // info!("size of BondInfo: {}", size_of_val(self));
                buffer[0] = StorageKeys::BleBondInfo as u8;
                let buf: [u8; 120] = unsafe { mem::transmute_copy(b) };
                buffer[1..121].copy_from_slice(&buf);
                Ok(121)
            }
            #[cfg(feature = "_nrf_ble")]
            StorageData::ActiveBleProfile(slot_num) => {
                buffer[0] = StorageKeys::ActiveBleProfile as u8;
                buffer[1] = *slot_num;
                Ok(2)
            }
        }
    }

    fn deserialize_from(buffer: &[u8]) -> Result<Self, SerializationError>
    where
        Self: Sized,
    {
        if buffer.is_empty() {
            return Err(SerializationError::InvalidFormat);
        }
        if let Some(key_type) = StorageKeys::from_u8(buffer[0]) {
            match key_type {
                StorageKeys::StorageConfig => {
                    if buffer.len() < 6 {
                        return Err(SerializationError::BufferTooSmall);
                    }
                    // 1 is the initial state of flash, so it means storage is NOT initialized
                    if buffer[1] == 1 {
                        Ok(StorageData::StorageConfig(LocalStorageConfig {
                            enable: false,
                            build_hash: BUILD_HASH,
                        }))
                    } else {
                        // Enabled, read build hash
                        let build_hash = BigEndian::read_u32(&buffer[2..6]);
                        Ok(StorageData::StorageConfig(LocalStorageConfig {
                            enable: true,
                            build_hash,
                        }))
                    }
                }
                StorageKeys::LedLightConfig => Err(SerializationError::Custom(0)),
                StorageKeys::RgbLightConfig => Err(SerializationError::Custom(0)),
                StorageKeys::KeymapConfig => Ok(StorageData::KeymapConfig(
                    EeKeymapConfig::from_bits(BigEndian::read_u16(&buffer[1..3])),
                )),
                StorageKeys::LayoutConfig => {
                    let default_layer = buffer[1];
                    let layout_option = BigEndian::read_u32(&buffer[2..6]);
                    Ok(StorageData::LayoutConfig(LayoutConfig {
                        default_layer,
                        layout_option,
                    }))
                }
                StorageKeys::KeymapKeys => {
                    let action = from_via_keycode(BigEndian::read_u16(&buffer[1..3]));
                    let layer = buffer[3] as usize;
                    let col = buffer[4] as usize;
                    let row = buffer[5] as usize;

                    // row, col, layer are used to calculate key only, not used here
                    Ok(StorageData::KeymapKey(KeymapKey {
                        row,
                        col,
                        layer,
                        action,
                    }))
                }
                StorageKeys::MacroData => {
                    if buffer.len() < MACRO_SPACE_SIZE + 1 {
                        return Err(SerializationError::InvalidData);
                    }
                    let mut buf = [0_u8; MACRO_SPACE_SIZE];
                    buf.copy_from_slice(&buffer[1..MACRO_SPACE_SIZE + 1]);
                    Ok(StorageData::MacroData(buf))
                }
                StorageKeys::ComboData => {
                    if buffer.len() < 11 {
                        return Err(SerializationError::InvalidData);
                    }
                    let mut actions = [KeyAction::No; 4];
                    for i in 0..4 {
                        actions[i] =
                            from_via_keycode(BigEndian::read_u16(&buffer[1 + i * 2..3 + i * 2]));
                    }
                    let output = from_via_keycode(BigEndian::read_u16(&buffer[9..11]));
                    Ok(StorageData::ComboData(ComboData {
                        idx: 0,
                        actions,
                        output,
                    }))
                }
                StorageKeys::ConnectionType => Ok(StorageData::ConnectionType(buffer[1])),
                #[cfg(feature = "_nrf_ble")]
                StorageKeys::BleBondInfo => {
                    // Make `transmute_copy` happy, because the compiler doesn't know the size of buffer
                    let mut buf = [0_u8; 120];
                    buf.copy_from_slice(&buffer[1..121]);
                    let info: BondInfo = unsafe { mem::transmute_copy(&buf) };

                    Ok(StorageData::BondInfo(info))
                }
                #[cfg(feature = "_nrf_ble")]
                StorageKeys::ActiveBleProfile => Ok(StorageData::ActiveBleProfile(buffer[1])),
            }
        } else {
            Err(SerializationError::Custom(1))
        }
    }
}

impl StorageData {
    fn key(&self) -> u32 {
        match self {
            StorageData::StorageConfig(_) => StorageKeys::StorageConfig as u32,
            StorageData::LayoutConfig(_) => StorageKeys::LayoutConfig as u32,
            StorageData::KeymapConfig(_) => StorageKeys::KeymapConfig as u32,
            StorageData::KeymapKey(_) => {
                panic!("To get storage key for KeymapKey, use `get_keymap_key` instead");
            }
            StorageData::MacroData(_) => StorageKeys::MacroData as u32,
            StorageData::ComboData(_) => {
                panic!("To get combo key for ComboData, use `get_combo_key` instead");
            }
            StorageData::ConnectionType(_) => StorageKeys::ConnectionType as u32,
            #[cfg(feature = "_nrf_ble")]
            StorageData::BondInfo(b) => get_bond_info_key(b.slot_num),
            #[cfg(feature = "_nrf_ble")]
            StorageData::ActiveBleProfile(_) => StorageKeys::ActiveBleProfile as u32,
        }
    }
}
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct LocalStorageConfig {
    enable: bool,
    build_hash: u32,
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct LayoutConfig {
    default_layer: u8,
    layout_option: u32,
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct KeymapKey {
    row: usize,
    col: usize,
    layer: usize,
    action: KeyAction,
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct ComboData {
    pub(crate) idx: usize,
    pub(crate) actions: [KeyAction; COMBO_MAX_LENGTH],
    pub(crate) output: KeyAction,
}

pub(crate) struct Storage<
    F: AsyncNorFlash,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
> {
    pub(crate) flash: F,
    pub(crate) storage_range: Range<u32>,
    buffer: [u8; get_buffer_size()],
}

/// Read out storage config, update and then save back.
/// This macro applies to only some of the configs.
macro_rules! write_storage {
    ($f: expr, $buf: expr, $cache:expr, $key:ident, $field:ident, $range:expr) => {
        if let Ok(Some(StorageData::$key(mut saved))) =
            fetch_item::<u32, StorageData, _>($f, $range, $cache, $buf, &(StorageKeys::$key as u32))
                .await
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

impl<F: AsyncNorFlash, const ROW: usize, const COL: usize, const NUM_LAYER: usize>
    Storage<F, ROW, COL, NUM_LAYER>
{
    pub(crate) async fn new(
        flash: F,
        keymap: &[[[KeyAction; COL]; ROW]; NUM_LAYER],
        config: StorageConfig,
    ) -> Self {
        // Check storage setting
        assert!(
            config.num_sectors >= 2,
            "Number of used sector for storage must larger than 1"
        );

        // If config.start_addr == 0, use last `num_sectors` sectors or sectors begin at 0x0006_0000 for nRF52
        // Other wise, use storage config setting
        #[cfg(feature = "_nrf_ble")]
        let start_addr = if config.start_addr == 0 {
            0x0006_0000
        } else {
            config.start_addr
        };

        #[cfg(not(feature = "_nrf_ble"))]
        let start_addr = config.start_addr;

        // Check storage setting
        info!(
            "Flash capacity {} KB, RMK use {} KB({} sectors) starting from 0x{:X} as storage",
            flash.capacity() / 1024,
            (F::ERASE_SIZE * config.num_sectors as usize) / 1024,
            config.num_sectors,
            config.start_addr,
        );
        let storage_range = if start_addr == 0 {
            (flash.capacity() - config.num_sectors as usize * F::ERASE_SIZE) as u32
                ..flash.capacity() as u32
        } else {
            assert!(
                start_addr % F::ERASE_SIZE == 0,
                "Storage's start addr MUST BE a multiplier of sector size"
            );
            start_addr as u32..(start_addr + config.num_sectors as usize * F::ERASE_SIZE) as u32
        };

        let mut storage = Self {
            flash,
            storage_range,
            buffer: [0; get_buffer_size()],
        };

        // Check whether keymap and configs have been storaged in flash
        if !storage.check_enable().await {
            // Clear storage first
            debug!("Clearing storage!");
            let _ =
                sequential_storage::erase_all(&mut storage.flash, storage.storage_range.clone())
                    .await;

            // Initialize storage from keymap and config
            if storage
                .initialize_storage_with_config(keymap)
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
        }

        storage
    }

    pub(crate) async fn run(&mut self) {
        let mut storage_cache = NoCache::new();
        loop {
            let info: FlashOperationMessage = FLASH_CHANNEL.receive().await;
            debug!("Flash operation: {:?}", info);
            if let Err(e) = match info {
                FlashOperationMessage::LayoutOptions(layout_option) => {
                    // Read out layout options, update layer option and save back
                    write_storage!(
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
                FlashOperationMessage::DefaultLayer(default_layer) => {
                    // Read out layout options, update layer option and save back
                    write_storage!(
                        &mut self.flash,
                        &mut self.buffer,
                        &mut storage_cache,
                        LayoutConfig,
                        default_layer,
                        self.storage_range.clone()
                    )
                }
                FlashOperationMessage::WriteMacro(macro_data) => {
                    info!("Saving keyboard macro data");
                    store_item(
                        &mut self.flash,
                        self.storage_range.clone(),
                        &mut storage_cache,
                        &mut self.buffer,
                        &(StorageKeys::MacroData as u32),
                        &StorageData::MacroData(macro_data),
                    )
                    .await
                }
                FlashOperationMessage::KeymapKey {
                    layer,
                    col,
                    row,
                    action,
                } => {
                    let data = StorageData::KeymapKey(KeymapKey {
                        row: row as usize,
                        col: col as usize,
                        layer: layer as usize,
                        action,
                    });
                    let key = get_keymap_key::<ROW, COL, NUM_LAYER>(
                        row as usize,
                        col as usize,
                        layer as usize,
                    );
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
                FlashOperationMessage::WriteCombo(combo) => {
                    let key = get_combo_key(combo.idx);
                    store_item(
                        &mut self.flash,
                        self.storage_range.clone(),
                        &mut storage_cache,
                        &mut self.buffer,
                        &key,
                        &StorageData::ComboData(combo),
                    )
                    .await
                }
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
                #[cfg(feature = "_nrf_ble")]
                FlashOperationMessage::ActiveBleProfile(profile) => {
                    let data = StorageData::ActiveBleProfile(profile);
                    store_item::<u32, StorageData, _>(
                        &mut self.flash,
                        self.storage_range.clone(),
                        &mut storage_cache,
                        &mut self.buffer,
                        &data.key(),
                        &data,
                    )
                    .await
                }
                #[cfg(feature = "_nrf_ble")]
                FlashOperationMessage::ClearSlot(key) => {
                    info!("Clearing bond info slot_num: {}", key);
                    // Remove item in `sequential-storage` is quite expensive, so just override the item with `removed = true`
                    let mut empty = BondInfo::default();
                    empty.removed = true;
                    let data = StorageData::BondInfo(empty);
                    store_item::<u32, StorageData, _>(
                        &mut self.flash,
                        self.storage_range.clone(),
                        &mut storage_cache,
                        &mut self.buffer,
                        &data.key(),
                        &data,
                    )
                    .await
                }
                #[cfg(feature = "_nrf_ble")]
                FlashOperationMessage::BondInfo(b) => {
                    info!("Saving bond info: {:?}", b);
                    let data = StorageData::BondInfo(b);
                    store_item::<u32, StorageData, _>(
                        &mut self.flash,
                        self.storage_range.clone(),
                        &mut storage_cache,
                        &mut self.buffer,
                        &data.key(),
                        &data,
                    )
                    .await
                }
                #[cfg(not(feature = "_nrf_ble"))]
                _ => Ok(()),
            } {
                print_storage_error::<F>(e);
            }
        }
    }

    pub(crate) async fn read_keymap(
        &mut self,
        keymap: &mut [[[KeyAction; COL]; ROW]; NUM_LAYER],
    ) -> Result<(), ()> {
        let mut storage_cache = NoCache::new();
        if let Ok(mut key_iterator) = fetch_all_items::<u32, _, _>(
            &mut self.flash,
            self.storage_range.clone(),
            &mut storage_cache,
            &mut self.buffer,
        )
        .await
        {
            // Iterator the storage, read all keymap keys
            while let Ok(Some((_key, item))) = key_iterator
                .next::<u32, StorageData>(&mut self.buffer)
                .await
            {
                match item {
                    StorageData::KeymapKey(key) => {
                        if key.layer < NUM_LAYER && key.row < ROW && key.col < COL {
                            keymap[key.layer][key.row][key.col] = key.action;
                        }
                    }
                    _ => continue,
                }
            }
        };

        Ok(())
    }

    pub(crate) async fn read_macro_cache(&mut self, macro_cache: &mut [u8]) -> Result<(), ()> {
        // Read storage and send back from send_channel
        let read_data = fetch_item::<u32, StorageData, _>(
            &mut self.flash,
            self.storage_range.clone(),
            &mut NoCache::new(),
            &mut self.buffer,
            &(StorageKeys::MacroData as u32),
        )
        .await
        .map_err(|e| print_storage_error::<F>(e))?;

        if let Some(StorageData::MacroData(data)) = read_data {
            // Send data back
            macro_cache.copy_from_slice(&data);
        }

        Ok(())
    }

    pub(crate) async fn read_combos(&mut self, combos: &mut [Combo]) -> Result<(), ()> {
        for i in 0..combos.len() {
            let key = get_combo_key(i);
            let read_data = fetch_item::<u32, StorageData, _>(
                &mut self.flash,
                self.storage_range.clone(),
                &mut NoCache::new(),
                &mut self.buffer,
                &key,
            )
            .await
            .map_err(|e| print_storage_error::<F>(e))?;

            if let Some(StorageData::ComboData(combo)) = read_data {
                let mut actions = Vec::<_, 4>::new();
                for &action in combo.actions.iter().filter(|&&a| a != KeyAction::No) {
                    let _ = actions.push(action);
                }
                combos[i] = Combo::new(actions, combo.output, combos[i].layer);
            }
        }

        Ok(())
    }

    async fn initialize_storage_with_config(
        &mut self,
        keymap: &[[[KeyAction; COL]; ROW]; NUM_LAYER],
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

        for (layer, layer_data) in keymap.iter().enumerate() {
            for (row, row_data) in layer_data.iter().enumerate() {
                for (col, action) in row_data.iter().enumerate() {
                    let item = StorageData::KeymapKey(KeymapKey {
                        row,
                        col,
                        layer,
                        action: *action,
                    });

                    let key = get_keymap_key::<ROW, COL, NUM_LAYER>(row, col, layer);

                    store_item(
                        &mut self.flash,
                        self.storage_range.clone(),
                        &mut cache,
                        &mut self.buffer,
                        &key,
                        &item,
                    )
                    .await
                    .map_err(|e| print_storage_error::<F>(e))?;
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
            if config.enable && config.build_hash == BUILD_HASH {
                return true;
            }
        }
        false
    }
}

fn print_storage_error<F: AsyncNorFlash>(e: SSError<F::Error>) {
    match e {
        SSError::Storage { value: _ } => error!("Flash error"),
        SSError::FullStorage => error!("Storage is full"),
        SSError::Corrupted {} => error!("Storage is corrupted"),
        SSError::BufferTooBig => error!("Buffer too big"),
        SSError::BufferTooSmall(x) => error!("Buffer too small, needs {} bytes", x),
        SSError::SerializationError(e) => error!("Map value error: {}", e),
        _ => error!("Unknown storage error"),
    }
}

const fn get_buffer_size() -> usize {
    // The buffer size needed = size_of(StorageData) = MACRO_SPACE_SIZE + 8(generally)
    // According to doc of `sequential-storage`, for some flashes it should be aligned in 32 bytes
    // To make sure the buffer works, do this alignment always
    let buffer_size = if MACRO_SPACE_SIZE < 248 {
        256
    } else {
        MACRO_SPACE_SIZE + 8
    };

    let remainder = buffer_size % 32;

    if remainder == 0 {
        buffer_size
    } else {
        buffer_size + 32 - remainder
    }
}
