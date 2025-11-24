use embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash;
use postcard::experimental::max_size::MaxSize;
use rmk_types::action::{EncoderAction, KeyAction};
use sequential_storage::cache::NoCache;
use sequential_storage::map::{SerializationError, Value, fetch_item};
use serde::{Deserialize, Serialize};

use crate::combo::{Combo, ComboConfig};
use crate::fork::Fork;
use crate::morse::Morse;
use crate::ser_storage_variant;
use crate::storage::{
    Storage, StorageData, StorageKeys, get_combo_key, get_fork_key, get_morse_key,
    postcard_error_to_serialization_error, print_storage_error,
};
use crate::{COMBO_MAX_NUM, FORK_MAX_NUM, MACRO_SPACE_SIZE, MORSE_MAX_NUM};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct KeymapKey {
    pub(crate) row: u8,
    pub(crate) col: u8,
    pub(crate) layer: u8,
    pub(crate) action: KeyAction,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, MaxSize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct EncoderKeymap {
    /// Encoder index
    pub(crate) idx: u8,
    /// Layer
    pub(crate) layer: u8,
    /// Encoder action
    pub(crate) action: EncoderAction,
}

/// Keymap data that can be updated by the host tools like Vial.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) enum KeymapData {
    // Write macro
    Macro([u8; MACRO_SPACE_SIZE]),
    // Write a key in keymap
    KeymapKey(KeymapKey),
    // Write encoder configuration
    Encoder(EncoderKeymap),
    // Write combo - stores (idx, config)
    Combo(u8, ComboConfig),
    // Write fork - stores (idx, fork)
    Fork(u8, Fork),
    // Write tap dance
    Morse(u8, Morse),
}

impl Value<'_> for KeymapData {
    fn serialize_into(&self, buffer: &mut [u8]) -> Result<usize, SerializationError> {
        if buffer.is_empty() {
            return Err(SerializationError::BufferTooSmall);
        }

        match self {
            Self::Macro(m) => {
                // Macro: direct copy without postcard encoding
                if buffer.len() < 1 + m.len() {
                    return Err(SerializationError::BufferTooSmall);
                }
                buffer[0] = StorageKeys::MacroData as u8;
                buffer[1..1 + m.len()].copy_from_slice(m);
                Ok(1 + m.len())
            }
            Self::KeymapKey(k) => ser_storage_variant!(buffer, StorageKeys::KeymapConfig, k),
            Self::Encoder(e) => ser_storage_variant!(buffer, StorageKeys::EncoderKeys, e),
            Self::Combo(idx, combo) => {
                buffer[0] = StorageKeys::ComboData as u8;
                let len = postcard::to_slice(&(*idx, *combo), &mut buffer[1..])
                    .map_err(postcard_error_to_serialization_error)?
                    .len();
                Ok(len + 1)
            }
            Self::Fork(idx, fork) => {
                buffer[0] = StorageKeys::ForkData as u8;
                let len = postcard::to_slice(&(*idx, *fork), &mut buffer[1..])
                    .map_err(postcard_error_to_serialization_error)?
                    .len();
                Ok(len + 1)
            }
            Self::Morse(idx, morse) => {
                buffer[0] = StorageKeys::MorseData as u8;
                let len = postcard::to_slice(&(*idx, morse.clone()), &mut buffer[1..])
                    .map_err(postcard_error_to_serialization_error)?
                    .len();
                Ok(len + 1)
            }
        }
    }

    fn deserialize_from(buffer: &[u8]) -> Result<Self, SerializationError>
    where
        Self: Sized,
    {
        if buffer.len() < 2 {
            return Err(SerializationError::InvalidFormat);
        }

        let key = StorageKeys::from_u8(buffer[0]).ok_or(SerializationError::InvalidFormat)?;

        match key {
            StorageKeys::MacroData => {
                // Large array: copy bytes directly
                if buffer.len() < 1 + MACRO_SPACE_SIZE {
                    return Err(SerializationError::InvalidFormat);
                }
                let mut macro_data = [0u8; MACRO_SPACE_SIZE];
                macro_data.copy_from_slice(&buffer[1..1 + MACRO_SPACE_SIZE]);
                Ok(Self::Macro(macro_data))
            }
            StorageKeys::KeymapConfig => Ok(Self::KeymapKey(
                postcard::from_bytes(&buffer[1..]).map_err(postcard_error_to_serialization_error)?,
            )),
            StorageKeys::EncoderKeys => Ok(Self::Encoder(
                postcard::from_bytes(&buffer[1..]).map_err(postcard_error_to_serialization_error)?,
            )),
            StorageKeys::ComboData => {
                let (idx, combo): (u8, ComboConfig) =
                    postcard::from_bytes(&buffer[1..]).map_err(postcard_error_to_serialization_error)?;
                Ok(Self::Combo(idx, combo))
            }
            StorageKeys::ForkData => {
                let (idx, fork): (u8, Fork) =
                    postcard::from_bytes(&buffer[1..]).map_err(postcard_error_to_serialization_error)?;
                Ok(Self::Fork(idx, fork))
            }
            StorageKeys::MorseData => {
                let (idx, morse): (u8, Morse) =
                    postcard::from_bytes(&buffer[1..]).map_err(postcard_error_to_serialization_error)?;
                Ok(Self::Morse(idx, morse))
            }
            _ => Err(SerializationError::InvalidFormat),
        }
    }
}

impl<F: AsyncNorFlash, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    pub(crate) async fn read_keymap(
        &mut self,
        keymap: &mut [[[KeyAction; COL]; ROW]; NUM_LAYER],
        encoder_map: &mut Option<&mut [[EncoderAction; NUM_ENCODER]; NUM_LAYER]>,
    ) -> Result<(), ()> {
        use sequential_storage::map::fetch_all_items;

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
                StorageData::VialData(KeymapData::KeymapKey(key)) => {
                    let layer = key.layer as usize;
                    let row = key.row as usize;
                    let col = key.col as usize;
                    if layer < NUM_LAYER && row < ROW && col < COL {
                        keymap[layer][row][col] = key.action;
                    }
                }
                StorageData::VialData(KeymapData::Encoder(encoder)) => {
                    if let Some(map) = encoder_map {
                        let idx = encoder.idx as usize;
                        let layer = encoder.layer as usize;
                        if layer < NUM_LAYER && idx < NUM_ENCODER {
                            map[layer][idx] = encoder.action;
                        }
                    }
                }
                _ => continue,
            }
        }

        Ok(())
    }

    pub(crate) async fn read_macro_cache(&mut self, macro_cache: &mut [u8]) -> Result<(), ()> {
        let read_data = fetch_item::<u32, StorageData, _>(
            &mut self.flash,
            self.storage_range.clone(),
            &mut NoCache::new(),
            &mut self.buffer,
            &(StorageKeys::MacroData as u32),
        )
        .await
        .map_err(|e| print_storage_error::<F>(e))?;

        if let Some(StorageData::VialData(KeymapData::Macro(data))) = read_data {
            macro_cache.copy_from_slice(&data);
        }

        Ok(())
    }

    pub(crate) async fn read_combos(&mut self, combos: &mut [Option<Combo>; COMBO_MAX_NUM]) -> Result<(), ()> {
        use crate::combo::Combo;

        for (i, item) in combos.iter_mut().enumerate() {
            let key = get_combo_key(i as u8);
            let read_data = fetch_item::<u32, StorageData, _>(
                &mut self.flash,
                self.storage_range.clone(),
                &mut NoCache::new(),
                &mut self.buffer,
                &key,
            )
            .await
            .map_err(|e| print_storage_error::<F>(e))?;

            if let Some(StorageData::VialData(KeymapData::Combo(_idx, config))) = read_data {
                debug!("Read combo config: {:?}", config);
                *item = Some(Combo::new(config));
            }
        }

        Ok(())
    }

    pub(crate) async fn read_forks(&mut self, forks: &mut heapless::Vec<Fork, FORK_MAX_NUM>) -> Result<(), ()> {
        for (i, item) in forks.iter_mut().enumerate() {
            let key = get_fork_key(i as u8);
            let read_data = fetch_item::<u32, StorageData, _>(
                &mut self.flash,
                self.storage_range.clone(),
                &mut NoCache::new(),
                &mut self.buffer,
                &key,
            )
            .await
            .map_err(|e| print_storage_error::<F>(e))?;

            if let Some(StorageData::VialData(KeymapData::Fork(_idx, fork))) = read_data {
                *item = fork;
            }
        }

        Ok(())
    }

    pub(crate) async fn read_morses(&mut self, morses: &mut heapless::Vec<Morse, MORSE_MAX_NUM>) -> Result<(), ()> {
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

            if let Some(StorageData::VialData(KeymapData::Morse(_, morse))) = read_data {
                *item = morse;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rmk_types::action::{Action, MorseMode, MorseProfile};
    use rmk_types::keycode::KeyCode;
    use sequential_storage::map::Value;

    use super::*;
    use crate::morse::{HOLD, MorsePattern, TAP};

    #[test]
    fn test_morse_serialization_deserialization() {
        let morse = Morse::new_from_vial(
            Action::Key(KeyCode::A),
            Action::Key(KeyCode::B),
            Action::Key(KeyCode::C),
            Action::Key(KeyCode::D),
            MorseProfile::new(Some(true), Some(MorseMode::PermissiveHold), Some(190u16), Some(180u16)),
        );

        // Serialization
        let mut buffer = [0u8; 7 + 4 * 4];
        let storage_data = StorageData::VialData(KeymapData::Morse(0, morse.clone()));
        let serialized_size = Value::serialize_into(&storage_data, &mut buffer).unwrap();

        // Deserialization
        let deserialized_data = StorageData::deserialize_from(&buffer[..serialized_size]).unwrap();

        // Validation
        match deserialized_data {
            StorageData::VialData(KeymapData::Morse(_, deserialized_morse)) => {
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
        let storage_data = StorageData::VialData(KeymapData::Morse(0, morse.clone()));
        let serialized_size = Value::serialize_into(&storage_data, &mut buffer).unwrap();

        // Deserialization
        let deserialized_data = StorageData::deserialize_from(&buffer[..serialized_size]).unwrap();

        // Validation
        match deserialized_data {
            StorageData::VialData(KeymapData::Morse(_, deserialized_morse)) => {
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
            profile: MorseProfile::new(
                Some(false),
                Some(MorseMode::HoldOnOtherPress),
                Some(210u16),
                Some(220u16),
            ),
            actions: heapless::LinearMap::default(),
        };
        morse
            .actions
            .insert(MorsePattern::from_u16(0b1_01), Action::Key(KeyCode::A))
            .ok();
        morse
            .actions
            .insert(MorsePattern::from_u16(0b1_1000), Action::Key(KeyCode::B))
            .ok();
        morse
            .actions
            .insert(MorsePattern::from_u16(0b1_1010), Action::Key(KeyCode::C))
            .ok();

        // Serialization
        let mut buffer = [0u8; 7 + 3 * 4];
        let storage_data = StorageData::VialData(KeymapData::Morse(0, morse.clone()));
        let serialized_size = Value::serialize_into(&storage_data, &mut buffer).unwrap();

        // Deserialization
        let deserialized_data = StorageData::deserialize_from(&buffer[..serialized_size]).unwrap();

        // Validation
        match deserialized_data {
            StorageData::VialData(KeymapData::Morse(_, deserialized_morse)) => {
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
