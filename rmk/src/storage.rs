// Storage wraps both `` and `embedded_storage_async`

use byteorder::{BigEndian, ByteOrder};
use defmt::{error, info, Format};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::channel::Channel;
use embedded_storage::nor_flash::NorFlash;
use embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash;
use packed_struct::PackedStructSlice;
use sequential_storage::{
    cache::NoCache,
    map::{fetch_item, store_item, StorageItem},
};

#[cfg(feature = "ble")]
use crate::ble::bonder::BondInfo;
#[cfg(feature = "ble")]
use core::mem;

use crate::{
    action::KeyAction,
    config::CONFIG_FLASH_RANGE,
    eeprom::eeconfig::EeKeymapConfig,
    via::keycode_convert::{from_via_keycode, to_via_keycode},
};

// Sync messages from server to flash
pub(crate) static FLASH_CHANNEL: Channel<ThreadModeRawMutex, FlashOperationMessage, 4> =
    Channel::new();

// Message send from bonder to flash task, which will do saving or clearing operation
#[derive(Clone, Copy, Debug, Format)]
pub(crate) enum FlashOperationMessage {
    // // Bond info to be saved
    #[cfg(feature = "ble")]
    BondInfo(BondInfo),
    // Clear info of given slot number
    Clear(u8),
}

#[repr(usize)]
pub(crate) enum StorageKeys {
    StorageConfig,
    LedLightConfig,
    RgbLightConfig,
    KeymapConfig,
    LayoutConfig,
    KeymapKeys,
    #[cfg(feature = "ble")]
    BleBondInfo,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum StorageData<const ROW: usize, const COL: usize, const NUM_LAYER: usize> {
    StorageConfig(StorageConfig),
    LayoutConfig(LayoutConfig),
    KeymapConfig(EeKeymapConfig),
    KeymapKey(KeymapKey<ROW, COL, NUM_LAYER>),
    #[cfg(feature = "ble")]
    BondInfo(BondInfo),
}

pub(crate) fn get_bond_info_key(slot_num: u8) -> usize {
    0x2000 + slot_num as usize
}

pub(crate) fn get_keymap_key<const ROW: usize, const COL: usize, const NUM_LAYER: usize>(
    row: usize,
    col: usize,
    layer: usize,
) -> usize {
    0x1000 + layer * COL * ROW + row * COL + col
}

#[derive(Format, Debug)]
pub(crate) enum StorageError {
    BufferTooSmall,
    ItemWrongSize,
    PackedStructError,
    SaveItemError,
    KeyError,
    NotSupported,
}

impl<const ROW: usize, const COL: usize, const NUM_LAYER: usize> StorageItem
    for StorageData<ROW, COL, NUM_LAYER>
{
    type Key = usize;

    type Error = StorageError;

    fn serialize_into(&self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        // TODO:
        if buffer.len() < 128 {
            return Err(StorageError::BufferTooSmall);
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
                match c.pack_to_slice(&mut buffer[1..3]) {
                    Ok(_) => Ok(3),
                    Err(_) => {
                        error!("Packing EeKeymapConfig error");
                        return Err(StorageError::PackedStructError);
                    }
                }
            }
            StorageData::KeymapKey(k) => {
                buffer[0] = StorageKeys::KeymapKeys as u8;
                BigEndian::write_u16(&mut buffer[1..3], to_via_keycode(k.action));
                buffer[3] = k.layer as u8;
                buffer[4] = k.col as u8;
                buffer[5] = k.row as u8;
                Ok(6)
            }
            #[cfg(feature = "ble")]
            StorageData::BondInfo(b) => {
                if buffer.len() < 121 {
                    return Err(StorageError::BufferTooSmall);
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

    fn deserialize_from(buffer: &[u8]) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        if buffer.len() < 1 {
            return Err(StorageError::ItemWrongSize);
        }

        match buffer[0] {
            0x0 => {
                // StorageConfig
                // 1 is the initial state of flash, so it means storage is NOT initialized
                if buffer[1] == 1 {
                    Ok(StorageData::StorageConfig(StorageConfig { enable: false }))
                } else {
                    Ok(StorageData::StorageConfig(StorageConfig { enable: true }))
                }
            }
            0x1 => {
                // LedLightConfig
                Err(StorageError::NotSupported)
            }
            0x2 => {
                // RgbLightConfig
                Err(StorageError::NotSupported)
            }
            0x3 => {
                // KeymapConfig
                if let Ok(config) = EeKeymapConfig::unpack_from_slice(&buffer[1..]) {
                    Ok(StorageData::KeymapConfig(config))
                } else {
                    error!("Unpacking EeKeymapConfig error");
                    Err(StorageError::PackedStructError)
                }
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
            #[cfg(feature = "ble")]
            0x6 => {
                // BleBondInfo
                // Make `transmute_copy` happy, because the compiler doesn't know the size of buffer
                let mut buf = [0_u8; 120];
                buf.copy_from_slice(&buffer[1..121]);
                let info: BondInfo = unsafe { mem::transmute_copy(&buf) };
                info!("Reading bond info key: {}", info);

                Ok(StorageData::BondInfo(info))
            }

            _ => {
                info!("Key error: {}", buffer[0]);
                Err(StorageError::KeyError)
            }
        }
    }

    fn key(&self) -> Self::Key {
        match self {
            StorageData::StorageConfig(_) => StorageKeys::StorageConfig as usize,
            StorageData::LayoutConfig(_) => StorageKeys::LayoutConfig as usize,
            StorageData::KeymapConfig(_) => StorageKeys::KeymapConfig as usize,
            #[cfg(feature = "ble")]
            StorageData::BondInfo(b) => get_bond_info_key(b.slot_num),
            StorageData::KeymapKey(k) => {
                let kk = *k;
                let key = get_keymap_key::<ROW, COL, NUM_LAYER>(kk.row, kk.col, kk.layer);
                key
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Format)]
pub(crate) struct StorageConfig {
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
}

impl<F: AsyncNorFlash> Storage<F> {
    pub(crate) async fn run<const ROW: usize, const COL: usize, const NUM_LAYER: usize>(
        &mut self,
    ) -> ! {
        // TODO: Save keymap key
        let mut storage_data_buffer = [0_u8; 128];
        loop {
            let info: FlashOperationMessage = FLASH_CHANNEL.receive().await;
            match info {
                #[cfg(feature = "ble")]
                FlashOperationMessage::Clear(key) => {
                    info!("Clearing bond info slot_num: {}", key);
                    // Remove item in `sequential-storage` is quite expensive, so just override the item with `removed = true`
                    let mut empty = BondInfo::default();
                    empty.removed = true;
                    let data = StorageData::BondInfo(empty);
                    store_item::<StorageData<ROW, COL, NUM_LAYER>, _>(
                        &mut self.flash,
                        CONFIG_FLASH_RANGE,
                        NoCache::new(),
                        &mut storage_data_buffer,
                        &data,
                    )
                    .await
                    .ok();
                }
                #[cfg(feature = "ble")]
                FlashOperationMessage::BondInfo(b) => {
                    info!("Saving item: {}", info);
                    let data = StorageData::BondInfo(b);
                    store_item::<StorageData<ROW, COL, NUM_LAYER>, _>(
                        &mut self.flash,
                        CONFIG_FLASH_RANGE,
                        NoCache::new(),
                        &mut storage_data_buffer,
                        &data,
                    )
                    .await
                    .ok();
                }
                _ => (),
            };
        }
    }

    pub async fn new<const ROW: usize, const COL: usize, const NUM_LAYER: usize>(
        flash: F,
        keymap: &[[[KeyAction; COL]; ROW]; NUM_LAYER],
    ) -> Self {
        // TODO: initialize all keymap & config stuffs
        let mut storage = Self { flash };
        // Check whether keymap and configs have been storaged in flash

        if !storage.check_enable::<ROW, COL, NUM_LAYER>().await {
            // Initialize storage from keymap and config
            if let Err(e) = storage.initialize_storage_with_config(keymap).await {
                error!("Initialize storage error: {}", e)
            }
            // TODO: ignore read from flash if it's just initialized
        }

        storage
    }

    pub(crate) async fn read_keymap<const ROW: usize, const COL: usize, const NUM_LAYER: usize>(
        &mut self,
        keymap: &mut [[[KeyAction; COL]; ROW]; NUM_LAYER],
    ) {
        let mut buf = [0u8; 128];
        for (layer, layer_data) in keymap.iter_mut().enumerate() {
            for (row, row_data) in layer_data.iter_mut().enumerate() {
                for (col, value) in row_data.iter_mut().enumerate() {
                    let key = get_keymap_key::<ROW, COL, NUM_LAYER>(row, col, layer);
                    info!("Reading key: {},{},{}: {}", layer, col, row, key);
                    let item = match fetch_item::<StorageData<ROW, COL, NUM_LAYER>, _>(
                        &mut self.flash,
                        CONFIG_FLASH_RANGE,
                        NoCache::new(),
                        &mut buf,
                        key,
                    )
                    .await
                    {
                        Ok(Some(StorageData::KeymapKey(k))) => {
                            info!(
                                "Read keymap key: (layer,col,row)=({},{},{}): {}",
                                layer, col, row, k.action
                            );
                            k.action
                        }
                        Ok(None) => {
                            error!("None (layer,col,row)=({},{},{})", layer, col, row);
                            KeyAction::No
                        }
                        _ => {
                            error!(
                                "Load keymap key from storage error: (layer,col,row)=({},{},{})",
                                layer, col, row
                            );
                            KeyAction::No
                        }
                    };
                    *value = item;
                }
            }
        }
    }

    async fn initialize_storage_with_config<
        const ROW: usize,
        const COL: usize,
        const NUM_LAYER: usize,
    >(
        &mut self,
        keymap: &[[[KeyAction; COL]; ROW]; NUM_LAYER],
    ) -> Result<(), StorageError> {
        let mut buf = [0u8; 128];
        // Save storage config
        store_item(
            &mut self.flash,
            CONFIG_FLASH_RANGE,
            NoCache::new(),
            &mut buf,
            &StorageData::<ROW, COL, NUM_LAYER>::StorageConfig(StorageConfig { enable: true }),
        )
        .await
        .unwrap();
        // .map_err(|_e| StorageError::SaveItemError)?;

        // Save layout config
        store_item(
            &mut self.flash,
            CONFIG_FLASH_RANGE,
            NoCache::new(),
            &mut buf,
            &StorageData::<ROW, COL, NUM_LAYER>::LayoutConfig(LayoutConfig {
                default_layer: 0,
                layout_option: 0,
            }),
        )
        .await
        .unwrap();
        // .map_err(|_e| StorageError::SaveItemError)?;

        for (layer, layer_data) in keymap.iter().enumerate() {
            for (row, row_data) in layer_data.iter().enumerate() {
                for (col, action) in row_data.iter().enumerate() {
                    let item = StorageData::KeymapKey(KeymapKey::<ROW, COL, NUM_LAYER> {
                        row,
                        col,
                        layer,
                        action: *action,
                    });
                    if let StorageData::KeymapKey(k) = item {
                        info!(
                            "stoing item: {},{},{}, {}, {}",
                            layer,
                            col,
                            row,
                            get_keymap_key::<ROW, COL, NUM_LAYER>(row, col, layer),
                            k
                        );
                    }

                    store_item(
                        &mut self.flash,
                        CONFIG_FLASH_RANGE,
                        NoCache::new(),
                        &mut buf,
                        &item,
                    )
                    .await
                    .unwrap();
                    // .map_err(|_e| StorageError::SaveItemError)?;
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
            fetch_item::<StorageData<ROW, COL, NUM_LAYER>, _>(
                &mut self.flash,
                CONFIG_FLASH_RANGE,
                NoCache::new(),
                &mut buf,
                StorageKeys::StorageConfig as usize,
            )
            .await
        {
            config.enable
        } else {
            false
        }
    }

    // TODO: Is there a way to convert `NorFlash` trait object to `F: AsyncNorFlash`?
    pub(crate) async fn new_from_blocking<BF: NorFlash>(_flash: BF) {
        // Self { flash }
    }
}
