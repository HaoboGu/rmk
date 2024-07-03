mod eeconfig;

use byteorder::{BigEndian, ByteOrder};
use defmt::{error, info, Format};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embedded_storage::nor_flash::NorFlash;
use embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash;
use rmk_config::StorageConfig;
use sequential_storage::{
    cache::NoCache,
    map::{fetch_item, store_item, SerializationError, Value},
    Error as SSError,
};

use core::ops::Range;
#[cfg(feature = "_nrf_ble")]
use {crate::ble::nrf::bonder::BondInfo, core::mem};

use crate::{
    action::KeyAction,
    via::keycode_convert::{from_via_keycode, to_via_keycode},
};

use self::eeconfig::EeKeymapConfig;

// Sync messages from server to flash
pub(crate) static FLASH_CHANNEL: Channel<CriticalSectionRawMutex, FlashOperationMessage, 8> =
    Channel::new();

// Message send from bonder to flash task, which will do saving or clearing operation
#[derive(Clone, Copy, Debug, Format)]
pub(crate) enum FlashOperationMessage {
    // Bond info to be saved
    #[cfg(feature = "_nrf_ble")]
    BondInfo(BondInfo),
    // Clear the storage
    Reset,
    // Clear info of given slot number
    ClearSlot(u8),
    // Layout option
    LayoutOptions(u32),
    // Default layer number
    DefaultLayer(u8),
    KeymapKey {
        layer: u8,
        col: u8,
        row: u8,
        action: KeyAction,
    },
}

#[repr(u32)]
pub(crate) enum StorageKeys {
    StorageConfig,
    LedLightConfig,
    RgbLightConfig,
    KeymapConfig,
    LayoutConfig,
    KeymapKeys,
    #[cfg(feature = "_nrf_ble")]
    BleBondInfo,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum StorageData<const ROW: usize, const COL: usize, const NUM_LAYER: usize> {
    StorageConfig(LocalStorageConfig),
    LayoutConfig(LayoutConfig),
    KeymapConfig(EeKeymapConfig),
    KeymapKey(KeymapKey<ROW, COL, NUM_LAYER>),
    #[cfg(feature = "_nrf_ble")]
    BondInfo(BondInfo),
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

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize> Value<'a>
    for StorageData<ROW, COL, NUM_LAYER>
{
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
                Ok(4)
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
        }
    }

    fn deserialize_from(buffer: &[u8]) -> Result<Self, SerializationError>
    where
        Self: Sized,
    {
        if buffer.len() < 1 {
            return Err(SerializationError::InvalidFormat);
        }
        match buffer[0] {
            0x0 => {
                // StorageConfig
                // 1 is the initial state of flash, so it means storage is NOT initialized
                if buffer[1] == 1 {
                    Ok(StorageData::StorageConfig(LocalStorageConfig {
                        enable: false,
                    }))
                } else {
                    Ok(StorageData::StorageConfig(LocalStorageConfig {
                        enable: true,
                    }))
                }
            }
            0x1 => {
                // LedLightConfig
                Err(SerializationError::Custom(0))
            }
            0x2 => {
                // RgbLightConfig
                Err(SerializationError::Custom(0))
            }
            0x3 => {
                // KeymapConfig
                Ok(StorageData::KeymapConfig(EeKeymapConfig::from_bits(
                    BigEndian::read_u16(&buffer[1..3]),
                )))
            }
            0x4 => {
                // LayoutConfig
                let default_layer = buffer[1];
                let layout_option = BigEndian::read_u32(&buffer[2..6]);
                Ok(StorageData::LayoutConfig(LayoutConfig {
                    default_layer,
                    layout_option,
                }))
            }
            0x5 => {
                // KeymapKey
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
            #[cfg(feature = "_nrf_ble")]
            0x6 => {
                // BleBondInfo
                // Make `transmute_copy` happy, because the compiler doesn't know the size of buffer
                let mut buf = [0_u8; 120];
                buf.copy_from_slice(&buffer[1..121]);
                let info: BondInfo = unsafe { mem::transmute_copy(&buf) };

                Ok(StorageData::BondInfo(info))
            }

            _ => {
                info!("Key error: {}", buffer[0]);
                Err(SerializationError::Custom(1))
            }
        }
    }
}

impl<const ROW: usize, const COL: usize, const NUM_LAYER: usize> StorageData<ROW, COL, NUM_LAYER> {
    fn key(&self) -> u32 {
        match self {
            StorageData::StorageConfig(_) => StorageKeys::StorageConfig as u32,
            StorageData::LayoutConfig(_) => StorageKeys::LayoutConfig as u32,
            StorageData::KeymapConfig(_) => StorageKeys::KeymapConfig as u32,
            #[cfg(feature = "_nrf_ble")]
            StorageData::BondInfo(b) => get_bond_info_key(b.slot_num),
            StorageData::KeymapKey(k) => {
                let kk = *k;
                get_keymap_key::<ROW, COL, NUM_LAYER>(kk.row, kk.col, kk.layer)
            }
        }
    }
}
#[derive(Clone, Copy, Debug, Format)]
pub(crate) struct LocalStorageConfig {
    enable: bool,
}

#[derive(Clone, Copy, Debug, Format)]
pub(crate) struct LayoutConfig {
    default_layer: u8,
    layout_option: u32,
}

#[derive(Clone, Copy, Debug, Format)]
pub(crate) struct KeymapKey<const ROW: usize, const COL: usize, const NUM_LAYER: usize> {
    row: usize,
    col: usize,
    layer: usize,
    action: KeyAction,
}

pub struct Storage<F: AsyncNorFlash> {
    pub(crate) flash: F,
    pub(crate) storage_range: Range<u32>,
}

/// Read out storage config, update and then save back.
/// This macro applies to only some of the configs.
macro_rules! write_storage {
    ($f: expr, $buf: expr, $cache:expr, $key:ident, $field:ident, $range:expr) => {
        if let Ok(Some(StorageData::$key(mut saved))) =
            fetch_item::<u32, StorageData<ROW, COL, NUM_LAYER>, _>(
                $f,
                $range,
                $cache,
                $buf,
                StorageKeys::$key as u32,
            )
            .await
        {
            saved.$field = $field;
            store_item::<u32, StorageData<ROW, COL, NUM_LAYER>, _>(
                $f,
                $range,
                $cache,
                $buf,
                StorageKeys::$key as u32,
                &StorageData::$key(saved),
            )
            .await
        } else {
            Ok(())
        }
    };
}

impl<F: AsyncNorFlash> Storage<F> {
    pub async fn new<const ROW: usize, const COL: usize, const NUM_LAYER: usize>(
        flash: F,
        keymap: &[[[KeyAction; COL]; ROW]; NUM_LAYER],
        config: StorageConfig,
    ) -> Self {
        // Check storage setting
        assert!(
            config.num_sectors >= 2,
            "Number of used sector for storage must larger than 1"
        );

        // If config.start_addr == 0, use last `num_sectors` sectors
        // Other wise, use storage config setting
        let storage_range = if config.start_addr == 0 {
            (flash.capacity() - config.num_sectors as usize * F::ERASE_SIZE) as u32
                ..flash.capacity() as u32
        } else {
            assert!(
                config.start_addr % F::ERASE_SIZE == 0,
                "Storage's start addr MUST BE a multiplier of sector size"
            );
            config.start_addr as u32
                ..(config.start_addr + config.num_sectors as usize * F::ERASE_SIZE) as u32
        };
        let mut storage = Self {
            flash,
            storage_range,
        };

        // Check whether keymap and configs have been storaged in flash
        if !storage.check_enable::<ROW, COL, NUM_LAYER>().await {
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
                    &mut [0; 128],
                    StorageKeys::StorageConfig as u32,
                    &StorageData::StorageConfig::<ROW, COL, NUM_LAYER>(LocalStorageConfig {
                        enable: false,
                    }),
                )
                .await
                .ok();
            }
        }

        storage
    }

    // TODO: Is there a way to convert `NorFlash` trait object to `F: AsyncNorFlash`?
    pub(crate) async fn new_from_blocking<BF: NorFlash>(_flash: BF) {
        // Self { flash }
    }

    pub(crate) async fn run<const ROW: usize, const COL: usize, const NUM_LAYER: usize>(&mut self) {
        let mut storage_data_buffer = [0_u8; 128];
        let mut storage_cache = NoCache::new();
        loop {
            let info: FlashOperationMessage = FLASH_CHANNEL.receive().await;
            if let Err(e) = match info {
                FlashOperationMessage::LayoutOptions(layout_option) => {
                    // Read out layout options, update layer option and save back
                    write_storage!(
                        &mut self.flash,
                        &mut storage_data_buffer,
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
                        &mut storage_data_buffer,
                        &mut storage_cache,
                        LayoutConfig,
                        default_layer,
                        self.storage_range.clone()
                    )
                }
                FlashOperationMessage::KeymapKey {
                    layer,
                    col,
                    row,
                    action,
                } => {
                    let data = StorageData::KeymapKey(KeymapKey::<ROW, COL, NUM_LAYER> {
                        row: row as usize,
                        col: col as usize,
                        layer: layer as usize,
                        action,
                    });
                    store_item(
                        &mut self.flash,
                        self.storage_range.clone(),
                        &mut storage_cache,
                        &mut storage_data_buffer,
                        data.key(),
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
                    store_item::<u32, StorageData<ROW, COL, NUM_LAYER>, _>(
                        &mut self.flash,
                        self.storage_range.clone(),
                        &mut storage_cache,
                        &mut storage_data_buffer,
                        data.key(),
                        &data,
                    )
                    .await
                }
                #[cfg(feature = "_nrf_ble")]
                FlashOperationMessage::BondInfo(b) => {
                    info!("Saving bond info: {}", info);
                    let data = StorageData::BondInfo(b);
                    store_item::<u32, StorageData<ROW, COL, NUM_LAYER>, _>(
                        &mut self.flash,
                        self.storage_range.clone(),
                        &mut storage_cache,
                        &mut storage_data_buffer,
                        data.key(),
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

    pub(crate) async fn read_keymap<const ROW: usize, const COL: usize, const NUM_LAYER: usize>(
        &mut self,
        keymap: &mut [[[KeyAction; COL]; ROW]; NUM_LAYER],
    ) -> Result<(), ()> {
        let mut buf = [0u8; 128];
        for (layer, layer_data) in keymap.iter_mut().enumerate() {
            for (row, row_data) in layer_data.iter_mut().enumerate() {
                for (col, value) in row_data.iter_mut().enumerate() {
                    let key = get_keymap_key::<ROW, COL, NUM_LAYER>(row, col, layer);
                    let item = match fetch_item::<u32, StorageData<ROW, COL, NUM_LAYER>, _>(
                        &mut self.flash,
                        self.storage_range.clone(),
                        &mut NoCache::new(),
                        &mut buf,
                        key,
                    )
                    .await
                    {
                        Ok(Some(StorageData::KeymapKey(k))) => k.action,
                        Ok(None) => {
                            error!("Got none when reading keymap from storage at (layer,col,row)=({},{},{})", layer, col, row);
                            return Err(());
                        }
                        Err(e) => {
                            print_storage_error::<F>(e);
                            error!(
                                "Load keymap key from storage error: (layer,col,row)=({},{},{})",
                                layer, col, row
                            );
                            return Err(());
                        }
                        _ => {
                            error!(
                                "Load keymap key from storage error: (layer,col,row)=({},{},{})",
                                layer, col, row
                            );
                            return Err(());
                        }
                    };
                    *value = item;
                }
            }
        }
        Ok(())
    }

    async fn initialize_storage_with_config<
        const ROW: usize,
        const COL: usize,
        const NUM_LAYER: usize,
    >(
        &mut self,
        keymap: &[[[KeyAction; COL]; ROW]; NUM_LAYER],
    ) -> Result<(), ()> {
        let mut cache = NoCache::new();
        let mut buf = [0u8; 128];
        // Save storage config
        let storage_config =
            StorageData::<ROW, COL, NUM_LAYER>::StorageConfig(LocalStorageConfig { enable: true });
        store_item(
            &mut self.flash,
            self.storage_range.clone(),
            &mut cache,
            &mut buf,
            storage_config.key(),
            &storage_config,
        )
        .await
        .map_err(|e| print_storage_error::<F>(e))?;

        // Save layout config
        let layout_config = StorageData::<ROW, COL, NUM_LAYER>::LayoutConfig(LayoutConfig {
            default_layer: 0,
            layout_option: 0,
        });
        store_item(
            &mut self.flash,
            self.storage_range.clone(),
            &mut cache,
            &mut buf,
            layout_config.key(),
            &layout_config,
        )
        .await
        .map_err(|e| print_storage_error::<F>(e))?;

        for (layer, layer_data) in keymap.iter().enumerate() {
            for (row, row_data) in layer_data.iter().enumerate() {
                for (col, action) in row_data.iter().enumerate() {
                    let item = StorageData::KeymapKey(KeymapKey::<ROW, COL, NUM_LAYER> {
                        row,
                        col,
                        layer,
                        action: *action,
                    });

                    store_item(
                        &mut self.flash,
                        self.storage_range.clone(),
                        &mut cache,
                        &mut buf,
                        item.key(),
                        &item,
                    )
                    .await
                    .map_err(|e| print_storage_error::<F>(e))?;
                }
            }
        }

        Ok(())
    }

    async fn check_enable<const ROW: usize, const COL: usize, const NUM_LAYER: usize>(
        &mut self,
    ) -> bool {
        let mut buf = [0u8; 128];
        if let Ok(Some(StorageData::StorageConfig(config))) =
            fetch_item::<u32, StorageData<ROW, COL, NUM_LAYER>, _>(
                &mut self.flash,
                self.storage_range.clone(),
                &mut NoCache::new(),
                &mut buf,
                StorageKeys::StorageConfig as u32,
            )
            .await
        {
            config.enable
        } else {
            false
        }
    }
}

fn print_storage_error<F: AsyncNorFlash>(e: SSError<F::Error>) {
    match e {
        SSError::Storage { value: _ } => error!("Flash error"),
        SSError::FullStorage => error!("Storage is full"),
        SSError::Corrupted {} => error!("Storage is corrupted"),
        SSError::BufferTooBig => error!("Buffer too big"),
        SSError::BufferTooSmall(_) => error!("Buffer too small"),
        SSError::SerializationError(e) => error!("Map value error: {}", e),
        _ => error!("Unknown storage error"),
    }
}
