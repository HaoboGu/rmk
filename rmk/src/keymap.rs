use crate::{
    action::{EncoderAction, KeyAction},
    combo::{Combo, COMBO_MAX_NUM},
    config::BehaviorConfig,
    event::{KeyEvent, RotaryEncoderEvent},
    fork::{Fork, FORK_MAX_NUM},
    keyboard_macro::{MacroOperation, MACRO_SPACE_SIZE},
    keycode::KeyCode,
};
#[cfg(feature = "storage")]
use crate::{boot::reboot_keyboard, storage::Storage};
#[cfg(feature = "storage")]
use embedded_storage_async::nor_flash::NorFlash;
use num_enum::FromPrimitive;

/// Keymap represents the stack of layers.
///
/// The conception of Keymap in rmk is borrowed from qmk: <https://docs.qmk.fm/#/keymap>.
///
/// Keymap should be binded to the actual pcb matrix definition.
/// RMK detects hardware key strokes, uses tuple `(row, col, layer)` to retrieve the action from Keymap.
pub struct KeyMap<
    'a,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize = 0,
> {
    /// Layers
    pub(crate) layers: &'a mut [[[KeyAction; COL]; ROW]; NUM_LAYER],
    /// Rotary encoders, each rotary encoder is represented as (Clockwise, CounterClockwise)
    pub(crate) encoders: Option<&'a mut [[EncoderAction; NUM_ENCODER]; NUM_LAYER]>,
    /// Current state of each layer
    layer_state: [bool; NUM_LAYER],
    /// Default layer number, max: 32
    default_layer: u8,
    /// Layer cache
    layer_cache: [[u8; COL]; ROW],
    /// Macro cache
    pub(crate) macro_cache: [u8; MACRO_SPACE_SIZE],
    /// Combos
    pub(crate) combos: [Combo; COMBO_MAX_NUM],
    /// Forks
    pub(crate) forks: [Fork; FORK_MAX_NUM],
    /// Options for configurable action behavior
    pub(crate) behavior: BehaviorConfig,
}

fn _reorder_combos(combos: &mut [Combo; COMBO_MAX_NUM]) {
    // Sort the combos by their length
    combos.sort_unstable_by(|c1, c2| c2.actions.len().cmp(&c1.actions.len()))
}

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    pub async fn new(
        action_map: &'a mut [[[KeyAction; COL]; ROW]; NUM_LAYER],
        encoder_map: Option<&'a mut [[EncoderAction; NUM_ENCODER]; NUM_LAYER]>,
        behavior: BehaviorConfig,
    ) -> Self {
        // If the storage is initialized, read keymap from storage
        let mut combos: [Combo; COMBO_MAX_NUM] = Default::default();
        for (i, combo) in behavior.combo.combos.iter().enumerate() {
            combos[i] = combo.clone();
        }

        //reorder the combos
        _reorder_combos(&mut combos);

        let mut forks: [Fork; FORK_MAX_NUM] = Default::default();
        for (i, fork) in behavior.fork.forks.iter().enumerate() {
            forks[i] = fork.clone();
        }

        KeyMap {
            layers: action_map,
            encoders: encoder_map,
            layer_state: [false; NUM_LAYER],
            default_layer: 0,
            layer_cache: [[0; COL]; ROW],
            macro_cache: [0; MACRO_SPACE_SIZE],
            combos,
            forks,
            behavior,
        }
    }
    #[cfg(feature = "storage")]
    pub async fn new_from_storage<F: NorFlash>(
        action_map: &'a mut [[[KeyAction; COL]; ROW]; NUM_LAYER],
        mut encoder_map: Option<&'a mut [[EncoderAction; NUM_ENCODER]; NUM_LAYER]>,
        storage: Option<&mut Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
        behavior: BehaviorConfig,
    ) -> Self {
        // If the storage is initialized, read keymap from storage
        let mut macro_cache = [0; MACRO_SPACE_SIZE];
        let mut combos: [Combo; COMBO_MAX_NUM] = Default::default();
        for (i, combo) in behavior.combo.combos.iter().enumerate() {
            combos[i] = combo.clone();
        }
        let mut forks: [Fork; FORK_MAX_NUM] = Default::default();
        for (i, fork) in behavior.fork.forks.iter().enumerate() {
            forks[i] = fork.clone();
        }
        if let Some(storage) = storage {
            if {
                Ok(())
                    // Read keymap to `action_map`
                    .and(storage.read_keymap(action_map, &mut encoder_map).await)
                    // Read macro cache
                    .and(storage.read_macro_cache(&mut macro_cache).await)
                    // Read combo cache
                    .and(storage.read_combos(&mut combos).await)
                    // Read fork cache
                    .and(storage.read_forks(&mut forks).await)
            }
            .is_err()
            {
                error!("Failed to read from storage, clearing...");
                sequential_storage::erase_all(&mut storage.flash, storage.storage_range.clone())
                    .await
                    .ok();

                reboot_keyboard();
            }
        }

        KeyMap {
            layers: action_map,
            encoders: encoder_map,
            layer_state: [false; NUM_LAYER],
            default_layer: 0,
            layer_cache: [[0; COL]; ROW],
            macro_cache,
            combos,
            forks,
            behavior,
        }
    }

    pub(crate) fn get_keymap_config(&self) -> (usize, usize, usize) {
        (ROW, COL, NUM_LAYER)
    }

    /// Get the default layer number
    pub(crate) fn get_default_layer(&self) -> u8 {
        self.default_layer
    }

    /// Set the default layer number
    pub(crate) fn set_default_layer(&mut self, layer_num: u8) {
        self.default_layer = layer_num;
    }

    /// Get the next macro operation starting from given index and offset
    /// Return current macro operation and the next operations's offset
    pub(crate) fn get_next_macro_operation(
        &self,
        macro_start_idx: usize,
        offset: usize,
    ) -> (MacroOperation, usize) {
        let idx = macro_start_idx + offset;
        if idx >= self.macro_cache.len() - 1 {
            return (MacroOperation::End, offset);
        }
        match (self.macro_cache[idx], self.macro_cache[idx + 1]) {
            (0, _) => (MacroOperation::End, offset),
            (1, 1) => {
                // SS_QMK_PREFIX + SS_TAP_CODE
                if idx + 2 < self.macro_cache.len() {
                    let keycode = KeyCode::from_primitive(self.macro_cache[idx + 2] as u16);
                    (MacroOperation::Tap(keycode), offset + 3)
                } else {
                    (MacroOperation::End, offset + 3)
                }
            }
            (1, 2) => {
                // SS_QMK_PREFIX + SS_DOWN_CODE
                if idx + 2 < self.macro_cache.len() {
                    let keycode = KeyCode::from_primitive(self.macro_cache[idx + 2] as u16);
                    (MacroOperation::Press(keycode), offset + 3)
                } else {
                    (MacroOperation::End, offset + 3)
                }
            }
            (1, 3) => {
                // SS_QMK_PREFIX + SS_UP_CODE
                if idx + 2 < self.macro_cache.len() {
                    let keycode = KeyCode::from_primitive(self.macro_cache[idx + 2] as u16);
                    (MacroOperation::Release(keycode), offset + 3)
                } else {
                    (MacroOperation::End, offset + 3)
                }
            }
            (1, 4) => {
                // SS_QMK_PREFIX + SS_DELAY_CODE
                if idx + 3 < self.macro_cache.len() {
                    let delay_ms = (self.macro_cache[idx + 2] as u16 - 1)
                        + (self.macro_cache[idx + 3] as u16 - 1) * 255;
                    (MacroOperation::Delay(delay_ms), offset + 4)
                } else {
                    (MacroOperation::End, offset + 4)
                }
            }
            (1, 5) | (1, 6) | (1, 7) => {
                warn!("VIAL_MACRO_EXT is not supported");
                (MacroOperation::Delay(0), offset + 4)
            }
            _ => {
                // Current byte is the ascii code, convert it to keyboard keycode(with caps state)
                let (keycode, is_caps) = KeyCode::from_ascii(self.macro_cache[idx]);
                (MacroOperation::Text(keycode, is_caps), offset + 1)
            }
        }
    }

    pub(crate) fn get_macro_start(&self, mut macro_idx: u8) -> Option<usize> {
        let mut idx = 0;
        // Find idx until the macro start of given index
        loop {
            if macro_idx == 0 || idx >= self.macro_cache.len() {
                break;
            }
            if self.macro_cache[idx] == 0 {
                macro_idx -= 1;
            }
            idx += 1;
        }

        if idx == self.macro_cache.len() {
            None
        } else {
            Some(idx)
        }
    }

    pub(crate) fn set_action_at(
        &mut self,
        row: usize,
        col: usize,
        layer_num: usize,
        action: KeyAction,
    ) {
        self.layers[layer_num][row][col] = action;
    }

    /// Fetch the action in keymap, with layer cache
    pub(crate) fn get_action_at(&mut self, row: usize, col: usize, layer_num: usize) -> KeyAction {
        self.layers[layer_num][row][col]
    }

    /// Fetch the action in keymap, with layer cache
    pub(crate) fn get_action_with_layer_cache(&mut self, key_event: KeyEvent) -> KeyAction {
        let row = key_event.row as usize;
        let col = key_event.col as usize;
        if !key_event.pressed {
            // Releasing a pressed key, use cached layer and restore the cache
            let layer = self.pop_layer_from_cache(row, col);
            return self.layers[layer as usize][row][col];
        }

        // Iterate from higher layer to lower layer, the lowest checked layer is the default layer
        for (layer_idx, layer) in self.layers.iter().enumerate().rev() {
            if self.layer_state[layer_idx] || layer_idx as u8 == self.default_layer {
                // This layer is activated
                let action = layer[row][col];
                if action == KeyAction::Transparent {
                    continue;
                }

                // Found a valid action in the layer, cache it
                self.save_layer_cache(row, col, layer_idx as u8);

                return action;
            }

            if layer_idx as u8 == self.default_layer {
                // No action
                break;
            }
        }

        KeyAction::No
    }

    pub(crate) fn get_encoder_with_layer_cache(
        &self,
        encoder_event: RotaryEncoderEvent,
    ) -> Option<&EncoderAction> {
        let layer = self.get_activated_layer();
        if let Some(encoders) = &self.encoders {
            encoders[layer as usize].get(encoder_event.id as usize)
        } else {
            None
        }
    }

    pub(crate) fn get_activated_layer(&self) -> u8 {
        for (layer_idx, _) in self.layers.iter().enumerate().rev() {
            if self.layer_state[layer_idx] || layer_idx as u8 == self.default_layer {
                return layer_idx as u8;
            }
        }

        self.default_layer
    }

    fn get_layer_from_cache(&self, row: usize, col: usize) -> u8 {
        self.layer_cache[row][col]
    }

    fn pop_layer_from_cache(&mut self, row: usize, col: usize) -> u8 {
        let layer = self.layer_cache[row][col];
        self.layer_cache[row][col] = self.default_layer;

        layer
    }

    fn save_layer_cache(&mut self, row: usize, col: usize, layer_num: u8) {
        self.layer_cache[row][col] = layer_num;
    }

    /// Update Tri Layer state
    fn update_tri_layer(&mut self) {
        if let Some(ref tri_layer) = self.behavior.tri_layer {
            self.layer_state[tri_layer[2] as usize] =
                self.layer_state[tri_layer[0] as usize] && self.layer_state[tri_layer[1] as usize];
        }
    }

    /// Activate given layer
    pub(crate) fn activate_layer(&mut self, layer_num: u8) {
        if layer_num as usize >= NUM_LAYER {
            warn!(
                "Not a valid layer {}, keyboard supports only {} layers",
                layer_num, NUM_LAYER
            );
            return;
        }
        self.layer_state[layer_num as usize] = true;
        self.update_tri_layer();
    }

    /// Deactivate given layer
    pub(crate) fn deactivate_layer(&mut self, layer_num: u8) {
        if layer_num as usize >= NUM_LAYER {
            warn!(
                "Not a valid layer {}, keyboard supports only {} layers",
                layer_num, NUM_LAYER
            );
            return;
        }
        self.layer_state[layer_num as usize] = false;
        self.update_tri_layer();
    }

    /// Toggle given layer
    pub(crate) fn toggle_layer(&mut self, layer_num: u8) {
        if layer_num as usize >= NUM_LAYER {
            warn!(
                "Not a valid layer {}, keyboard supports only {} layers",
                layer_num, NUM_LAYER
            );
            return;
        }

        self.layer_state[layer_num as usize] = !self.layer_state[layer_num as usize];
    }

    //order combos by their actions length
    pub(crate) fn reorder_combos(&mut self) {
        _reorder_combos(&mut self.combos);
    }
}

#[cfg(test)]
mod test {
    use crate::combo::COMBO_MAX_NUM;
    use crate::k;
    use crate::{action::KeyAction, keycode::KeyCode};

    use super::{Combo, _reorder_combos};

    #[test]
    fn test_combo_reordering() {
        let combos_raw = vec![
            Combo::new([k!(A), k!(B), k!(C), k!(D)], k!(Z), None),
            Combo::new([k!(A), k!(B)], k!(X), None),
            Combo::new([k!(A), k!(B), k!(C)], k!(Y), None),
        ];
        // trans combos from vec to array
        let mut combos: [Combo; COMBO_MAX_NUM] = Default::default();
        for (i, combo) in combos_raw.clone().into_iter().enumerate() {
            combos[i] = combo;
        }

        _reorder_combos(&mut combos);

        let result: Vec<u16> = combos
            .iter()
            .enumerate()
            .map(|(_, c)| match c.output {
                KeyAction::Single(k) => k.to_action_code(),
                _ => KeyCode::No as u16,
            })
            .collect();
        assert_eq!(
            result,
            vec![
                KeyCode::Z as u16,
                KeyCode::Y as u16,
                KeyCode::X as u16,
                0,
                0,
                0,
                0,
                0
            ]
        );
    }
}
