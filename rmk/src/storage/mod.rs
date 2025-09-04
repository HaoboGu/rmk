pub mod dummy_flash;

use core::fmt::Debug;
use core::ops::Range;

use byteorder::{BigEndian, ByteOrder};
use embassy_embedded_hal::adapter::BlockingAsync;
use embassy_sync::signal::Signal;
use embassy_time::Duration;
use embedded_storage::nor_flash::NorFlash;
use embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash;
use heapless::Vec;
use sequential_storage::Error as SSError;
use sequential_storage::cache::NoCache;
use sequential_storage::map::{SerializationError, Value, fetch_all_items, fetch_item, store_item};
#[cfg(feature = "_ble")]
use {
    crate::ble::trouble::ble_server::CCCD_TABLE_SIZE,
    crate::ble::trouble::profile::ProfileInfo,
    trouble_host::{BondInformation, IdentityResolvingKey, LongTermKey, prelude::*},
};

use crate::action::{EncoderAction, KeyAction};
use crate::channel::FLASH_CHANNEL;
use crate::combo::Combo;
use crate::config::{self, StorageConfig};
use crate::fork::{Fork, StateBits};
use crate::hid_state::{HidModifiers, HidMouseButtons};
use crate::light::LedIndicator;
use crate::morse::{Morse, MorsePattern};
#[cfg(all(feature = "_ble", feature = "split"))]
use crate::split::ble::PeerAddress;
use crate::via::keycode_convert::{from_via_keycode, to_via_keycode};
use crate::{BUILD_HASH, COMBO_MAX_LENGTH, COMBO_MAX_NUM, FORK_MAX_NUM, MACRO_SPACE_SIZE, MORSE_MAX_NUM};

/// Signal to synchronize the flash operation status, usually used outside of the flash task.
/// True if the flash operation is finished correctly, false if the flash operation is finished with error.
pub(crate) static FLASH_OPERATION_FINISHED: Signal<crate::RawMutex, bool> = Signal::new();

// Message send from bonder to flash task, which will do saving or clearing operation
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
    // Write macro
    WriteMacro([u8; MACRO_SPACE_SIZE]),
    // Write a key in keymap
    KeymapKey {
        layer: u8,
        col: u8,
        row: u8,
        action: KeyAction,
    },
    // Write encoder configuration
    EncoderKey {
        idx: u8,
        layer: u8,
        action: EncoderAction,
    },
    // Current saved connection type
    ConnectionType(u8),
    // Write combo
    WriteCombo(ComboData),
    // Write fork
    WriteFork(ForkData),
    // Write morse config
    WriteMorse(u8, Morse),
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
    // Timeout time for morse keys in default tap hold profile
    MorseHoldTimeout(u16),
    // Whether the unilateral tap is enabled in default tap hold profile
    UnilateralTap(bool),
}

/// StorageKeys is the prefix digit stored in the flash, it's used to identify the type of the stored data.
///
/// This is because the whole storage item is an Rust enum due to the limitation of `sequential_storage`.
/// When deserializing, we need to know the type of the stored data to know how to parse it, the first byte of the stored data is always the type, aka StorageKeys.
#[repr(u32)]
pub(crate) enum StorageKeys {
    StorageConfig = 0,
    KeymapConfig = 1,
    LayoutConfig = 2,
    BehaviorConfig = 3,
    MacroData = 4,
    ComboData = 5,
    ConnectionType = 6,
    EncoderKeys = 7,
    ForkData = 8,
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
            1 => Some(StorageKeys::KeymapConfig),
            2 => Some(StorageKeys::LayoutConfig),
            3 => Some(StorageKeys::BehaviorConfig),
            4 => Some(StorageKeys::MacroData),
            5 => Some(StorageKeys::ComboData),
            6 => Some(StorageKeys::ConnectionType),
            7 => Some(StorageKeys::EncoderKeys),
            8 => Some(StorageKeys::ForkData),
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

#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) enum StorageData {
    StorageConfig(LocalStorageConfig),
    LayoutConfig(LayoutConfig),
    BehaviorConfig(BehaviorConfig),
    KeymapKey(KeymapKey),
    EncoderConfig(EncoderConfig),
    MacroData([u8; MACRO_SPACE_SIZE]),
    ComboData(ComboData),
    ConnectionType(u8),
    ForkData(ForkData),
    MorseData(Morse),
    #[cfg(all(feature = "_ble", feature = "split"))]
    PeerAddress(PeerAddress),
    #[cfg(feature = "_ble")]
    BondInfo(ProfileInfo),
    #[cfg(feature = "_ble")]
    ActiveBleProfile(u8),
}

/// Get the key to retrieve the keymap key from the storage.
pub(crate) fn get_keymap_key<const ROW: usize, const COL: usize, const NUM_LAYER: usize>(
    row: usize,
    col: usize,
    layer: usize,
) -> u32 {
    0x1000 + (layer * COL * ROW + row * COL + col) as u32
}

/// Get the key to retrieve the bond info from the storage.
pub(crate) fn get_bond_info_key(slot_num: u8) -> u32 {
    0x2000 + slot_num as u32
}

/// Get the key to retrieve the combo from the storage.
pub(crate) fn get_combo_key(idx: usize) -> u32 {
    0x3000 + idx as u32
}

/// Get the key to retrieve the encoder config from the storage.
pub(crate) fn get_encoder_config_key<const NUM_ENCODER: usize>(idx: usize, layer: usize) -> u32 {
    0x4000 + (idx + NUM_ENCODER * layer) as u32
}

pub(crate) fn get_fork_key(idx: usize) -> u32 {
    0x5000 + idx as u32
}

/// Get the key to retrieve the peer address from the storage.
pub(crate) fn get_peer_address_key(peer_id: u8) -> u32 {
    0x6000 + peer_id as u32
}

/// Get the key to retrieve the morse key from the storage.
pub(crate) fn get_morse_key(idx: u8) -> u32 {
    0x7000 + idx as u32
}

// TODO: Move ser/de code to corresponding structs
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
            StorageData::BehaviorConfig(c) => {
                buffer[0] = StorageKeys::BehaviorConfig as u8;
                BigEndian::write_u16(&mut buffer[1..3], c.prior_idle_time);
                BigEndian::write_u16(&mut buffer[3..5], c.morse_hold_timeout_ms);
                buffer[5] = if c.unilateral_tap { 1 } else { 0 };

                BigEndian::write_u16(&mut buffer[6..8], c.combo_timeout);
                BigEndian::write_u16(&mut buffer[8..10], c.one_shot_timeout);
                BigEndian::write_u16(&mut buffer[10..12], c.tap_interval);
                BigEndian::write_u16(&mut buffer[12..14], c.tap_capslock_interval);
                Ok(14)
            }
            StorageData::KeymapKey(k) => {
                buffer[0] = StorageKeys::KeymapConfig as u8;
                BigEndian::write_u16(&mut buffer[1..3], to_via_keycode(k.action));
                buffer[3] = k.layer as u8;
                buffer[4] = k.col as u8;
                buffer[5] = k.row as u8;
                Ok(6)
            }
            StorageData::EncoderConfig(e) => {
                buffer[0] = StorageKeys::EncoderKeys as u8;
                BigEndian::write_u16(&mut buffer[1..3], to_via_keycode(e.action.clockwise()));
                BigEndian::write_u16(&mut buffer[3..5], to_via_keycode(e.action.counter_clockwise()));
                buffer[5] = e.idx as u8;
                buffer[6] = e.layer as u8;
                Ok(7)
            }
            StorageData::MacroData(d) => {
                if buffer.len() < MACRO_SPACE_SIZE + 1 {
                    return Err(SerializationError::BufferTooSmall);
                }
                buffer[0] = StorageKeys::MacroData as u8;
                let mut idx = MACRO_SPACE_SIZE - 1;
                // Check from the end of the macro buffer, find the first non-zero byte
                while let Some(b) = d.get(idx) {
                    if *b != 0 || idx == 0 {
                        break;
                    }
                    idx -= 1;
                }
                let data_len = idx + 1;
                // Macro data length
                buffer[1..3].copy_from_slice(&(data_len as u16).to_le_bytes());
                // Macro data
                buffer[3..3 + data_len].copy_from_slice(&d[..data_len]);
                Ok(data_len + 3)
            }
            StorageData::ComboData(combo) => {
                if buffer.len() < 3 + COMBO_MAX_LENGTH * 2 {
                    return Err(SerializationError::BufferTooSmall);
                }
                buffer[0] = StorageKeys::ComboData as u8;
                for i in 0..COMBO_MAX_LENGTH {
                    BigEndian::write_u16(&mut buffer[1 + i * 2..3 + i * 2], to_via_keycode(combo.actions[i]));
                }
                BigEndian::write_u16(
                    &mut buffer[1 + COMBO_MAX_LENGTH * 2..3 + COMBO_MAX_LENGTH * 2],
                    to_via_keycode(combo.output),
                );
                Ok(3 + COMBO_MAX_LENGTH * 2)
            }
            StorageData::ForkData(fork) => {
                if buffer.len() < 13 {
                    return Err(SerializationError::BufferTooSmall);
                }
                buffer[0] = StorageKeys::ForkData as u8;
                BigEndian::write_u16(&mut buffer[1..3], to_via_keycode(fork.fork.trigger));
                BigEndian::write_u16(&mut buffer[3..5], to_via_keycode(fork.fork.negative_output));
                BigEndian::write_u16(&mut buffer[5..7], to_via_keycode(fork.fork.positive_output));

                BigEndian::write_u16(
                    &mut buffer[7..9],
                    fork.fork.match_any.leds.into_bits() as u16 | (fork.fork.match_none.leds.into_bits() as u16) << 8,
                );
                BigEndian::write_u16(
                    &mut buffer[9..11],
                    fork.fork.match_any.mouse.into_bits() as u16 | (fork.fork.match_none.mouse.into_bits() as u16) << 8,
                );
                BigEndian::write_u32(
                    &mut buffer[11..15],
                    fork.fork.match_any.modifiers.into_bits() as u32
                        | (fork.fork.match_none.modifiers.into_bits() as u32) << 8
                        | (fork.fork.kept_modifiers.into_bits() as u32) << 16
                        | if fork.fork.bindable { 1 << 24 } else { 0 },
                );
                Ok(15)
            }
            StorageData::MorseData(morse) => {
                let total_size = 7 + 4 * morse.actions.len();
                if buffer.len() < total_size {
                    return Err(SerializationError::BufferTooSmall);
                }
                buffer[0] = StorageKeys::MorseData as u8;
                BigEndian::write_u32(&mut buffer[1..5], morse.profile.into());
                BigEndian::write_u16(&mut buffer[5..7], morse.actions.len() as u16);
                let mut i = 7;
                for (pattern, action) in &morse.actions {
                    BigEndian::write_u16(
                        &mut buffer[i..i + 2],
                        pattern.to_u16(), //pattern
                    );
                    BigEndian::write_u16(&mut buffer[i + 2..i + 4], to_via_keycode(KeyAction::Single(*action)));
                    i += 4;
                }

                Ok(total_size)
            }
            StorageData::ConnectionType(ty) => {
                buffer[0] = StorageKeys::ConnectionType as u8;
                buffer[1] = *ty;
                Ok(2)
            }
            #[cfg(all(feature = "_ble", feature = "split"))]
            StorageData::PeerAddress(p) => {
                if buffer.len() < 9 {
                    return Err(SerializationError::BufferTooSmall);
                }
                buffer[0] = StorageKeys::PeerAddress as u8;
                buffer[1] = p.peer_id;
                buffer[2] = if p.is_valid { 1 } else { 0 };
                buffer[3..9].copy_from_slice(&p.address);
                Ok(9)
            }
            #[cfg(feature = "_ble")]
            StorageData::ActiveBleProfile(slot_num) => {
                buffer[0] = StorageKeys::ActiveBleProfile as u8;
                buffer[1] = *slot_num;
                Ok(2)
            }
            #[cfg(feature = "_ble")]
            StorageData::BondInfo(b) => {
                if buffer.len() < 40 + CCCD_TABLE_SIZE * 4 {
                    return Err(SerializationError::BufferTooSmall);
                }
                buffer[0] = StorageKeys::BleBondInfo as u8;
                let ltk = b.info.ltk.to_le_bytes();
                let address = b.info.identity.bd_addr;
                let irk = match b.info.identity.irk {
                    Some(irk) => irk.to_le_bytes(),
                    None => [0; 16],
                };
                buffer[1] = b.slot_num;
                buffer[2..18].copy_from_slice(&ltk);
                buffer[18..24].copy_from_slice(address.raw());
                buffer[24..40].copy_from_slice(&irk);
                let cccd_table = b.cccd_table.inner();
                for i in 0..CCCD_TABLE_SIZE {
                    match cccd_table.get(i) {
                        Some(cccd) => {
                            let handle: u16 = cccd.0;
                            let cccd: u16 = cccd.1.raw();
                            buffer[40 + i * 4..42 + i * 4].copy_from_slice(&handle.to_le_bytes());
                            buffer[42 + i * 4..44 + i * 4].copy_from_slice(&cccd.to_le_bytes());
                        }
                        None => {
                            buffer[40 + i * 4..44 + i * 4].copy_from_slice(&[0, 0, 0, 0]);
                        }
                    };
                }
                Ok(40 + CCCD_TABLE_SIZE * 4)
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
                StorageKeys::KeymapConfig => {
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
                StorageKeys::LayoutConfig => {
                    let default_layer = buffer[1];
                    let layout_option = BigEndian::read_u32(&buffer[2..6]);
                    Ok(StorageData::LayoutConfig(LayoutConfig {
                        default_layer,
                        layout_option,
                    }))
                }
                StorageKeys::BehaviorConfig => {
                    if buffer.len() < 14 {
                        return Err(SerializationError::BufferTooSmall);
                    }
                    let keymap_config = BehaviorConfig {
                        prior_idle_time: BigEndian::read_u16(&buffer[1..3]),
                        morse_hold_timeout_ms: BigEndian::read_u16(&buffer[3..5]),
                        unilateral_tap: buffer[5] != 0,

                        combo_timeout: BigEndian::read_u16(&buffer[6..8]),
                        one_shot_timeout: BigEndian::read_u16(&buffer[8..10]),
                        tap_interval: BigEndian::read_u16(&buffer[10..12]),
                        tap_capslock_interval: BigEndian::read_u16(&buffer[12..14]),
                    };
                    Ok(StorageData::BehaviorConfig(keymap_config))
                }
                StorageKeys::MacroData => {
                    if buffer.len() < 3 {
                        return Err(SerializationError::InvalidData);
                    }
                    let mut buf = [0_u8; MACRO_SPACE_SIZE];
                    let macro_length = u16::from_le_bytes(buffer[1..3].try_into().unwrap()) as usize;
                    if macro_length > MACRO_SPACE_SIZE + 1 || buffer.len() < 3 + macro_length {
                        // Check length
                        return Err(SerializationError::InvalidData);
                    }
                    buf[0..macro_length].copy_from_slice(&buffer[3..3 + macro_length]);
                    Ok(StorageData::MacroData(buf))
                }
                StorageKeys::ComboData => {
                    if buffer.len() < 3 + COMBO_MAX_LENGTH * 2 {
                        return Err(SerializationError::InvalidData);
                    }
                    let mut actions = [KeyAction::No; COMBO_MAX_LENGTH];
                    for i in 0..COMBO_MAX_LENGTH {
                        actions[i] = from_via_keycode(BigEndian::read_u16(&buffer[1 + i * 2..3 + i * 2]));
                    }
                    let output = from_via_keycode(BigEndian::read_u16(
                        &buffer[1 + COMBO_MAX_LENGTH * 2..3 + COMBO_MAX_LENGTH * 2],
                    ));
                    Ok(StorageData::ComboData(ComboData {
                        idx: 0,
                        actions,
                        output,
                    }))
                }
                StorageKeys::ConnectionType => Ok(StorageData::ConnectionType(buffer[1])),
                StorageKeys::EncoderKeys => {
                    if buffer.len() < 7 {
                        return Err(SerializationError::BufferTooSmall);
                    }
                    let clockwise = from_via_keycode(BigEndian::read_u16(&buffer[1..3]));
                    let counter_clockwise = from_via_keycode(BigEndian::read_u16(&buffer[3..5]));
                    let idx = buffer[5] as usize;
                    let layer = buffer[6] as usize;

                    Ok(StorageData::EncoderConfig(EncoderConfig {
                        idx,
                        layer,
                        action: EncoderAction::new(clockwise, counter_clockwise),
                    }))
                }
                StorageKeys::ForkData => {
                    if buffer.len() < 15 {
                        return Err(SerializationError::InvalidData);
                    }
                    let trigger = from_via_keycode(BigEndian::read_u16(&buffer[1..3]));
                    let negative_output = from_via_keycode(BigEndian::read_u16(&buffer[3..5]));
                    let positive_output = from_via_keycode(BigEndian::read_u16(&buffer[5..7]));

                    let led_masks = BigEndian::read_u16(&buffer[7..9]);
                    let mouse_masks = BigEndian::read_u16(&buffer[9..11]);
                    let modifier_masks = BigEndian::read_u32(&buffer[11..15]);

                    let match_any = StateBits {
                        modifiers: HidModifiers::from_bits((modifier_masks & 0xFF) as u8),
                        leds: LedIndicator::from_bits((led_masks & 0xFF) as u8),
                        mouse: HidMouseButtons::from_bits((mouse_masks & 0xFF) as u8),
                    };
                    let match_none = StateBits {
                        modifiers: HidModifiers::from_bits(((modifier_masks >> 8) & 0xFF) as u8),
                        leds: LedIndicator::from_bits(((led_masks >> 8) & 0xFF) as u8),
                        mouse: HidMouseButtons::from_bits(((mouse_masks >> 8) & 0xFF) as u8),
                    };
                    let kept_modifiers = HidModifiers::from_bits(((modifier_masks >> 16) & 0xFF) as u8);
                    let bindable = (modifier_masks & (1 << 24)) != 0;

                    Ok(StorageData::ForkData(ForkData {
                        idx: 0,
                        fork: Fork::new(
                            trigger,
                            negative_output,
                            positive_output,
                            match_any,
                            match_none,
                            kept_modifiers,
                            bindable,
                        ),
                    }))
                }
                StorageKeys::MorseData => {
                    if buffer.len() < 7 {
                        return Err(SerializationError::InvalidData);
                    }
                    let profile = BigEndian::read_u32(&buffer[1..5]);
                    let count = BigEndian::read_u16(&buffer[5..7]) as usize;

                    if buffer.len() < 7 + 4 * count {
                        return Err(SerializationError::InvalidData);
                    }

                    let mut morse = Morse::default();
                    morse.profile = profile.into();

                    let mut i = 7;
                    for _ in 0..count {
                        let pattern = MorsePattern::from_u16(BigEndian::read_u16(&buffer[i..i + 2]));
                        let key_action = from_via_keycode(BigEndian::read_u16(&buffer[i + 2..i + 4]));
                        _ = morse.actions.push((pattern, key_action.to_action()));
                        i += 4;
                    }

                    Ok(StorageData::MorseData(morse))
                }
                #[cfg(all(feature = "_ble", feature = "split"))]
                StorageKeys::PeerAddress => {
                    if buffer.len() < 9 {
                        return Err(SerializationError::InvalidData);
                    }
                    let peer_id = buffer[1];
                    let is_valid = buffer[2] != 0;
                    let mut address = [0u8; 6];
                    address.copy_from_slice(&buffer[3..9]);
                    Ok(StorageData::PeerAddress(PeerAddress {
                        peer_id,
                        is_valid,
                        address,
                    }))
                }
                #[cfg(feature = "_ble")]
                StorageKeys::ActiveBleProfile => {
                    if buffer.len() < 2 {
                        return Err(SerializationError::BufferTooSmall);
                    }
                    Ok(StorageData::ActiveBleProfile(buffer[1]))
                }
                #[cfg(feature = "_ble")]
                StorageKeys::BleBondInfo => {
                    if buffer.len() < 40 + CCCD_TABLE_SIZE * 4 {
                        return Err(SerializationError::BufferTooSmall);
                    }
                    let slot_num = buffer[1];
                    let ltk = LongTermKey::from_le_bytes(buffer[2..18].try_into().unwrap());
                    let address = BdAddr::new(buffer[18..24].try_into().unwrap());
                    let irk = IdentityResolvingKey::from_le_bytes(buffer[24..40].try_into().unwrap());
                    // Use all 0s as the empty irk
                    let info = if irk.0 == 0 {
                        BondInformation::new(
                            Identity {
                                bd_addr: address,
                                irk: None,
                            },
                            ltk,
                        )
                    } else {
                        BondInformation::new(
                            Identity {
                                bd_addr: address,
                                irk: Some(irk),
                            },
                            ltk,
                        )
                    };
                    // Read info:
                    let mut cccd_table_values = [(0u16, CCCD::default()); CCCD_TABLE_SIZE];
                    for i in 0..CCCD_TABLE_SIZE {
                        let handle = u16::from_le_bytes(buffer[40 + i * 4..42 + i * 4].try_into().unwrap());
                        let cccd = u16::from_le_bytes(buffer[42 + i * 4..44 + i * 4].try_into().unwrap());
                        cccd_table_values[i] = (handle, cccd.into());
                    }
                    Ok(StorageData::BondInfo(ProfileInfo {
                        slot_num,
                        removed: false,
                        info,
                        cccd_table: CccdTable::new(cccd_table_values),
                    }))
                }
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
            StorageData::BehaviorConfig(_) => StorageKeys::BehaviorConfig as u32,
            StorageData::KeymapKey(_) => {
                panic!("To get storage key for KeymapKey, use `get_keymap_key` instead");
            }
            StorageData::EncoderConfig(_) => {
                panic!("To get encoder config key, use `get_encoder_config_key` instead");
            }
            StorageData::MacroData(_) => StorageKeys::MacroData as u32,
            StorageData::ComboData(_) => {
                panic!("To get combo key for ComboData, use `get_combo_key` instead");
            }
            StorageData::ConnectionType(_) => StorageKeys::ConnectionType as u32,
            StorageData::ForkData(_) => {
                panic!("To get fork key for ForkData, use `get_fork_key` instead");
            }
            StorageData::MorseData(_) => {
                panic!("To get morse key for MorseData, use `get_morse_key` instead");
            }
            #[cfg(all(feature = "_ble", feature = "split"))]
            StorageData::PeerAddress(p) => get_peer_address_key(p.peer_id),
            #[cfg(feature = "_ble")]
            StorageData::ActiveBleProfile(_) => StorageKeys::ActiveBleProfile as u32,
            #[cfg(feature = "_ble")]
            StorageData::BondInfo(b) => get_bond_info_key(b.slot_num),
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
pub(crate) struct EncoderConfig {
    /// Encoder index
    idx: usize,
    /// Layer
    layer: usize,
    /// Encoder action
    action: EncoderAction,
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct ComboData {
    pub(crate) idx: usize,
    pub(crate) actions: [KeyAction; COMBO_MAX_LENGTH],
    pub(crate) output: KeyAction,
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct ForkData {
    pub(crate) idx: usize,
    pub(crate) fork: Fork,
}

#[derive(Clone, Copy, Debug)]
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
        &[],
        &None,
        &storage_config,
        &config::BehaviorConfig::<0, 0>::default(),
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
        keymap: &[[[KeyAction; COL]; ROW]; NUM_LAYER],
        encoder_map: &Option<&mut [[EncoderAction; NUM_ENCODER]; NUM_LAYER]>,
        storage_config: &StorageConfig,
        behavior_config: &config::BehaviorConfig<ROW, COL>,
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
                start_addr % F::ERASE_SIZE == 0,
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
                .initialize_storage_with_config(keymap, encoder_map, behavior_config)
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
            debug!("clear_layout=true; overwriting layout items without erase.");

            let encoder_map = encoder_map.as_ref().map(|m| &**m);

            let _ = storage.reset_layout_only(keymap, &encoder_map, behavior_config).await;
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
                    let key = get_keymap_key::<ROW, COL, NUM_LAYER>(row as usize, col as usize, layer as usize);
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
                FlashOperationMessage::EncoderKey { idx, layer, action } => {
                    let data = StorageData::EncoderConfig(EncoderConfig {
                        idx: idx as usize,
                        layer: layer as usize,
                        action,
                    });
                    let key = get_encoder_config_key::<NUM_ENCODER>(idx as usize, layer as usize);
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
                FlashOperationMessage::WriteFork(fork) => {
                    let key = get_fork_key(fork.idx);
                    store_item(
                        &mut self.flash,
                        self.storage_range.clone(),
                        &mut storage_cache,
                        &mut self.buffer,
                        &key,
                        &StorageData::ForkData(fork),
                    )
                    .await
                }
                FlashOperationMessage::WriteMorse(id, morse) => {
                    let key = get_morse_key(id);
                    store_item(
                        &mut self.flash,
                        self.storage_range.clone(),
                        &mut storage_cache,
                        &mut self.buffer,
                        &key,
                        &StorageData::MorseData(morse),
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
                        &data.key(),
                        &data,
                    )
                    .await
                }
                #[cfg(feature = "_ble")]
                FlashOperationMessage::ClearSlot(key) => {
                    info!("Clearing bond info slot_num: {}", key);
                    // Remove item in `sequential-storage` is quite expensive, so just override the item with `removed = true`
                    let mut empty = ProfileInfo::default();
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
                #[cfg(feature = "_ble")]
                FlashOperationMessage::ProfileInfo(b) => {
                    debug!("Saving profile info: {:?}", b);
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

    pub(crate) async fn read_keymap(
        &mut self,
        keymap: &mut [[[KeyAction; COL]; ROW]; NUM_LAYER],
        encoder_map: &mut Option<&mut [[EncoderAction; NUM_ENCODER]; NUM_LAYER]>,
    ) -> Result<(), ()> {
        let mut storage_cache = NoCache::new();
        // Use fetch_all_items to speed up the keymap reading
        let mut key_iterator = fetch_all_items::<u32, _, _>(
            &mut self.flash,
            self.storage_range.clone(),
            &mut storage_cache,
            &mut self.buffer,
        )
        .await
        .map_err(|e| print_storage_error::<F>(e))?;

        // Read all keymap keys and encoder configs
        while let Some((_key, item)) = key_iterator
            .next::<StorageData>(&mut self.buffer)
            .await
            .map_err(|e| print_storage_error::<F>(e))?
        {
            match item {
                StorageData::KeymapKey(key) => {
                    if key.layer < NUM_LAYER && key.row < ROW && key.col < COL {
                        keymap[key.layer][key.row][key.col] = key.action;
                    }
                }
                StorageData::EncoderConfig(encoder) => {
                    if let Some(map) = encoder_map {
                        if encoder.layer < NUM_LAYER && encoder.idx < NUM_ENCODER {
                            map[encoder.layer][encoder.idx] = encoder.action;
                        }
                    }
                }
                _ => continue,
            }
        }

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

    pub(crate) async fn read_combos(&mut self, combos: &mut Vec<Combo, COMBO_MAX_NUM>) -> Result<(), ()> {
        for (i, item) in combos.iter_mut().enumerate() {
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
                let mut actions: Vec<KeyAction, COMBO_MAX_LENGTH> = Vec::new();
                for &action in combo.actions.iter().filter(|&&a| a != KeyAction::No) {
                    let _ = actions.push(action);
                }
                *item = Combo::new(actions, combo.output, item.layer);
            }
        }

        Ok(())
    }

    pub(crate) async fn read_forks(&mut self, forks: &mut Vec<Fork, FORK_MAX_NUM>) -> Result<(), ()> {
        for (i, item) in forks.iter_mut().enumerate() {
            let key = get_fork_key(i);
            let read_data = fetch_item::<u32, StorageData, _>(
                &mut self.flash,
                self.storage_range.clone(),
                &mut NoCache::new(),
                &mut self.buffer,
                &key,
            )
            .await
            .map_err(|e| print_storage_error::<F>(e))?;

            if let Some(StorageData::ForkData(fork)) = read_data {
                *item = fork.fork;
            }
        }

        Ok(())
    }

    pub(crate) async fn read_morses(&mut self, morses: &mut Vec<Morse, MORSE_MAX_NUM>) -> Result<(), ()> {
        for (i, item) in morses.iter_mut().enumerate() {
            let key = get_morse_key(i as u8);
            let read_data = fetch_item::<u32, StorageData, _>(
                &mut self.flash,
                self.storage_range.clone(),
                &mut NoCache::new(),
                &mut self.buffer,
                &key,
            )
            .await
            .map_err(|e| print_storage_error::<F>(e))?;

            if let Some(StorageData::MorseData(morse)) = read_data {
                *item = morse;
            }
        }

        Ok(())
    }

    pub(crate) async fn read_behavior_config(
        &mut self,
        behavior_config: &mut config::BehaviorConfig<ROW, COL>,
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
            behavior_config.tap_hold.prior_idle_time = Duration::from_millis(c.prior_idle_time as u64);
            behavior_config
                .tap_hold
                .default_profile
                .set_hold_timeout_ms(c.morse_hold_timeout_ms);
            behavior_config
                .tap_hold
                .default_profile
                .set_unilateral_tap(c.unilateral_tap);

            behavior_config.combo.timeout = Duration::from_millis(c.combo_timeout as u64);
            behavior_config.one_shot.timeout = Duration::from_millis(c.one_shot_timeout as u64);
            behavior_config.tap.tap_interval = c.tap_interval;
            behavior_config.tap.tap_capslock_interval = c.tap_capslock_interval;
        }

        Ok(())
    }

    async fn initialize_storage_with_config(
        &mut self,
        keymap: &[[[KeyAction; COL]; ROW]; NUM_LAYER],
        encoder_map: &Option<&mut [[EncoderAction; NUM_ENCODER]; NUM_LAYER]>,
        behavior: &config::BehaviorConfig<ROW, COL>,
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
            prior_idle_time: behavior.tap_hold.prior_idle_time.as_millis() as u16,
            morse_hold_timeout_ms: behavior.tap_hold.default_profile.hold_timeout_ms(),
            unilateral_tap: behavior.tap_hold.default_profile.unilateral_tap(),

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

        // Save encoder configurations
        if let Some(encoder_map) = encoder_map {
            for (layer, layer_data) in encoder_map.iter().enumerate() {
                for (idx, action) in layer_data.iter().enumerate() {
                    let item = StorageData::EncoderConfig(EncoderConfig {
                        idx,
                        layer,
                        action: *action,
                    });

                    let key = get_encoder_config_key::<NUM_ENCODER>(idx, layer);

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

    async fn reset_layout_only(
        &mut self,
        keymap: &[[[KeyAction; COL]; ROW]; NUM_LAYER],
        encoder_map: &Option<&[[EncoderAction; NUM_ENCODER]; NUM_LAYER]>,
        behavior: &config::BehaviorConfig<ROW, COL>,
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
            //enable_flow_tap: behavior.tap_hold.enable_flow_tap,
            prior_idle_time: behavior.tap_hold.prior_idle_time.as_millis() as u16,
            morse_hold_timeout_ms: behavior.tap_hold.default_profile.hold_timeout_ms(),
            unilateral_tap: behavior.tap_hold.default_profile.unilateral_tap(),

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
                    .await?;
                }
            }
        }

        if let Some(encoder_map) = encoder_map {
            for (layer, layer_data) in encoder_map.iter().enumerate() {
                for (idx, action) in layer_data.iter().enumerate() {
                    let item = StorageData::EncoderConfig(EncoderConfig {
                        idx,
                        layer,
                        action: *action,
                    });
                    let key = get_encoder_config_key::<NUM_ENCODER>(idx, layer);
                    store_item(
                        &mut self.flash,
                        self.storage_range.clone(),
                        &mut cache,
                        &mut self.buffer,
                        &key,
                        &item,
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

fn print_storage_error<F: AsyncNorFlash>(e: SSError<F::Error>) {
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
    // The buffer size needed = size_of(StorageData) = MACRO_SPACE_SIZE + 8(generally)
    // According to doc of `sequential-storage`, for some flashes it should be aligned in 32 bytes
    // To make sure the buffer works, do this alignment always
    let buffer_size = if MACRO_SPACE_SIZE < 248 {
        256
    } else {
        MACRO_SPACE_SIZE + 8
    };

    // Efficiently round up to the nearest multiple of 32 using bit manipulation.
    (buffer_size + 31) & !31
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

#[cfg(test)]
mod tests {
    use sequential_storage::map::Value;

    use super::*;
    use crate::action::Action;
    use crate::config::TapHoldProfile;
    use crate::keycode::KeyCode;
    use crate::morse::{HOLD, TAP};

    #[test]
    fn test_morse_serialization_deserialization() {
        let morse = Morse::new_from_vial(
            Action::Key(KeyCode::A),
            Action::Key(KeyCode::B),
            Action::Key(KeyCode::C),
            Action::Key(KeyCode::D),
            TapHoldProfile::new()
                .with_is_filled(true)
                .with_permissive_hold(true)
                .with_unilateral_tap(true)
                .with_hold_timeout_ms(190u16)
                .with_gap_timeout_ms(180u16),
        );

        // Serialization
        let mut buffer = [0u8; 7 + 4 * 4];
        let storage_data = StorageData::MorseData(morse.clone());
        let serialized_size = Value::serialize_into(&storage_data, &mut buffer).unwrap();

        // Deserialization
        let deserialized_data = StorageData::deserialize_from(&buffer[..serialized_size]).unwrap();

        // Validation
        match deserialized_data {
            StorageData::MorseData(deserialized_morse) => {
                // actions
                assert_eq!(deserialized_morse.actions.len(), morse.actions.len());
                for (original, deserialized) in morse.actions.iter().zip(deserialized_morse.actions.iter()) {
                    assert_eq!(original, deserialized);
                }
                // profile
                assert_eq!(deserialized_morse.profile, morse.profile);
            }
            _ => panic!("Expected MorseData"),
        }
    }

    #[test]
    fn test_morse_with_partial_actions() {
        // Create a Morse with partial actions
        let mut morse: Morse = Morse::default();
        _ = morse.put(TAP, Action::Key(KeyCode::A));
        _ = morse.put(HOLD, Action::Key(KeyCode::B));

        // Serialization
        let mut buffer = [0u8; 7 + 4 * 4];
        let storage_data = StorageData::MorseData(morse.clone());
        let serialized_size = Value::serialize_into(&storage_data, &mut buffer).unwrap();

        // Deserialization
        let deserialized_data = StorageData::deserialize_from(&buffer[..serialized_size]).unwrap();

        // Validation
        match deserialized_data {
            StorageData::MorseData(deserialized_morse) => {
                // actions
                assert_eq!(deserialized_morse.actions.len(), morse.actions.len());
                for (original, deserialized) in morse.actions.iter().zip(deserialized_morse.actions.iter()) {
                    assert_eq!(original, deserialized);
                }
                // profile
                assert_eq!(deserialized_morse.profile, morse.profile);
            }
            _ => panic!("Expected MorseData"),
        }
    }

    #[test]
    fn test_morse_with_morse_serialization_deserialization() {
        let mut morse = Morse {
            profile: TapHoldProfile::new()
                .with_is_filled(true)
                .with_hold_on_other_press(true)
                .with_unilateral_tap(false)
                .with_hold_timeout_ms(210u16)
                .with_gap_timeout_ms(220u16),
            actions: Vec::default(),
        };
        morse
            .actions
            .push((MorsePattern::from_u16(0b1_01), Action::Key(KeyCode::A)))
            .ok();
        morse
            .actions
            .push((MorsePattern::from_u16(0b1_1000), Action::Key(KeyCode::B)))
            .ok();
        morse
            .actions
            .push((MorsePattern::from_u16(0b1_1010), Action::Key(KeyCode::C)))
            .ok();

        // Serialization
        let mut buffer = [0u8; 7 + 3 * 4];
        let storage_data = StorageData::MorseData(morse.clone());
        let serialized_size = Value::serialize_into(&storage_data, &mut buffer).unwrap();

        // Deserialization
        let deserialized_data = StorageData::deserialize_from(&buffer[..serialized_size]).unwrap();

        // Validation
        match deserialized_data {
            StorageData::MorseData(deserialized_morse) => {
                // actions
                assert_eq!(deserialized_morse.actions.len(), morse.actions.len());
                for (original, deserialized) in morse.actions.iter().zip(deserialized_morse.actions.iter()) {
                    assert_eq!(original, deserialized);
                }
                // profile
                assert_eq!(deserialized_morse.profile, morse.profile);
            }
            _ => panic!("Expected MorseData"),
        }
    }
}
