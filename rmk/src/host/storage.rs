use byteorder::{BigEndian, ByteOrder as _};
use embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash;
use heapless::Vec;
use rmk_types::action::{EncoderAction, KeyAction, MorseProfile};
use rmk_types::led_indicator::LedIndicator;
use rmk_types::modifier::ModifierCombination;
use rmk_types::mouse_button::MouseButtons;
use sequential_storage::cache::NoCache;
use sequential_storage::map::{SerializationError, Value, fetch_item};

use crate::combo::Combo;
use crate::fork::{Fork, StateBits};
use crate::host::via::keycode_convert::{from_via_keycode, to_via_keycode};
use crate::morse::{Morse, MorsePattern};
use crate::storage::{
    Storage, StorageData, StorageKeys, get_combo_key, get_fork_key, get_morse_key, print_storage_error,
};
use crate::{COMBO_MAX_LENGTH, COMBO_MAX_NUM, FORK_MAX_NUM, MACRO_SPACE_SIZE, MORSE_MAX_NUM};

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct KeymapKey {
    pub(crate) row: u8,
    pub(crate) col: u8,
    pub(crate) layer: u8,
    pub(crate) action: KeyAction,
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct EncoderConfig {
    /// Encoder index
    pub(crate) idx: u8,
    /// Layer
    pub(crate) layer: u8,
    /// Encoder action
    pub(crate) action: EncoderAction,
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct ComboData {
    /// Combo index
    pub(crate) idx: usize,
    /// Combo components
    pub(crate) actions: [KeyAction; COMBO_MAX_LENGTH],
    /// Combo output
    pub(crate) output: KeyAction,
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub(crate) struct ForkData {
    /// Fork index
    pub(crate) idx: usize,
    /// Fork instance
    pub(crate) fork: Fork,
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
    Encoder(EncoderConfig),
    // Write combo
    Combo(ComboData),
    // Write fork
    Fork(ForkData),
    // Write tap dance
    Morse(u8, Morse),
}

impl Value<'_> for KeymapData {
    fn serialize_into(&self, buffer: &mut [u8]) -> Result<usize, sequential_storage::map::SerializationError> {
        if buffer.len() < 6 {
            return Err(SerializationError::BufferTooSmall);
        }
        match self {
            KeymapData::KeymapKey(k) => {
                buffer[0] = StorageKeys::KeymapConfig as u8;
                BigEndian::write_u16(&mut buffer[1..3], to_via_keycode(k.action));
                buffer[3] = k.layer;
                buffer[4] = k.col;
                buffer[5] = k.row;
                Ok(6)
            }

            KeymapData::Encoder(e) => {
                buffer[0] = StorageKeys::EncoderKeys as u8;
                BigEndian::write_u16(&mut buffer[1..3], to_via_keycode(e.action.clockwise()));
                BigEndian::write_u16(&mut buffer[3..5], to_via_keycode(e.action.counter_clockwise()));
                buffer[5] = e.idx;
                buffer[6] = e.layer;
                Ok(7)
            }

            KeymapData::Macro(d) => {
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

            KeymapData::Combo(combo) => {
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

            KeymapData::Fork(fork) => {
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

            KeymapData::Morse(_, morse) => {
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
        }
    }

    fn deserialize_from(buffer: &'_ [u8]) -> Result<Self, sequential_storage::map::SerializationError>
    where
        Self: Sized,
    {
        if buffer.is_empty() {
            return Err(SerializationError::InvalidFormat);
        }
        if let Some(key_type) = StorageKeys::from_u8(buffer[0]) {
            match key_type {
                StorageKeys::KeymapConfig => {
                    let action = from_via_keycode(BigEndian::read_u16(&buffer[1..3]));
                    let layer = buffer[3];
                    let col = buffer[4];
                    let row = buffer[5];
                    Ok(KeymapData::KeymapKey(KeymapKey {
                        row,
                        col,
                        layer,
                        action,
                    }))
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
                    Ok(KeymapData::Macro(buf))
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
                    Ok(KeymapData::Combo(ComboData {
                        idx: 0,
                        actions,
                        output,
                    }))
                }

                StorageKeys::EncoderKeys => {
                    if buffer.len() < 7 {
                        return Err(SerializationError::BufferTooSmall);
                    }
                    let clockwise = from_via_keycode(BigEndian::read_u16(&buffer[1..3]));
                    let counter_clockwise = from_via_keycode(BigEndian::read_u16(&buffer[3..5]));
                    let idx = buffer[5];
                    let layer = buffer[6];

                    Ok(KeymapData::Encoder(EncoderConfig {
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
                        modifiers: ModifierCombination::from_bits((modifier_masks & 0xFF) as u8),
                        leds: LedIndicator::from_bits((led_masks & 0xFF) as u8),
                        mouse: MouseButtons::from_bits((mouse_masks & 0xFF) as u8),
                    };
                    let match_none = StateBits {
                        modifiers: ModifierCombination::from_bits(((modifier_masks >> 8) & 0xFF) as u8),
                        leds: LedIndicator::from_bits(((led_masks >> 8) & 0xFF) as u8),
                        mouse: MouseButtons::from_bits(((mouse_masks >> 8) & 0xFF) as u8),
                    };
                    let kept_modifiers = ModifierCombination::from_bits(((modifier_masks >> 16) & 0xFF) as u8);
                    let bindable = (modifier_masks & (1 << 24)) != 0;

                    Ok(KeymapData::Fork(ForkData {
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
                    let profile = MorseProfile::from(BigEndian::read_u32(&buffer[1..5]));
                    let count = BigEndian::read_u16(&buffer[5..7]) as usize;

                    if buffer.len() < 7 + 4 * count {
                        return Err(SerializationError::InvalidData);
                    }

                    let mut morse = Morse {
                        profile,
                        ..Default::default()
                    };

                    let mut i = 7;
                    for _ in 0..count {
                        let pattern = MorsePattern::from_u16(BigEndian::read_u16(&buffer[i..i + 2]));
                        let key_action = from_via_keycode(BigEndian::read_u16(&buffer[i + 2..i + 4]));
                        _ = morse.actions.insert(pattern, key_action.to_action());
                        i += 4;
                    }

                    // The morse id isn't important
                    Ok(KeymapData::Morse(0, morse))
                }
                _ => Err(SerializationError::Custom(2)),
            }
        } else {
            Err(SerializationError::Custom(1))
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
        use sequential_storage::cache::NoCache;
        use sequential_storage::map::fetch_all_items;

        use crate::storage::print_storage_error;

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

        if let Some(StorageData::VialData(KeymapData::Macro(data))) = read_data {
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

            if let Some(StorageData::VialData(KeymapData::Combo(combo))) = read_data {
                let mut actions: Vec<KeyAction, COMBO_MAX_LENGTH> = Vec::new();
                for &action in combo.actions.iter().filter(|&&a| !a.is_empty()) {
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

            if let Some(StorageData::VialData(KeymapData::Fork(fork))) = read_data {
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
    use crate::morse::{HOLD, TAP};

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
