#[cfg(feature = "storage")]
use embedded_storage_async::nor_flash::NorFlash;
#[cfg(feature = "controller")]
use {
    crate::channel::{send_controller_event, ControllerPub, CONTROLLER_CHANNEL},
    crate::event::ControllerEvent,
};

use crate::action::{EncoderAction, KeyAction};
use crate::combo::Combo;
use crate::config::BehaviorConfig;
use crate::event::{KeyEvent, RotaryEncoderEvent};
use crate::keyboard_macros::MacroOperation;
use crate::COMBO_MAX_NUM;
#[cfg(feature = "storage")]
use crate::{boot::reboot_keyboard, storage::Storage};

/// Keymap represents the stack of layers.
///
/// The conception of Keymap in rmk is borrowed from qmk: <https://docs.qmk.fm/#/keymap>.
///
/// Keymap should be binded to the actual pcb matrix definition.
/// RMK detects hardware key strokes, uses tuple `(row, col, layer)` to retrieve the action from Keymap.
pub struct KeyMap<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize = 0> {
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
    /// Options for configurable action behavior
    pub(crate) behavior: BehaviorConfig,
    /// Publisher for controller channel
    #[cfg(feature = "controller")]
    controller_pub: ControllerPub,
}

fn _reorder_combos(combos: &mut heapless::Vec<Combo, COMBO_MAX_NUM>) {
    // Sort the combos by their length
    combos.sort_unstable_by(|c1, c2| c2.actions.len().cmp(&c1.actions.len()))
}

/// fills up the vector to its capacity
pub(crate) fn fill_vec<T: Default + Clone, const N: usize>(vector: &mut heapless::Vec<T, N>) {
    vector
        .resize(vector.capacity(), T::default())
        .expect("impossible error, as we resie to the capcacity of the vector!");
}

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    pub async fn new(
        action_map: &'a mut [[[KeyAction; COL]; ROW]; NUM_LAYER],
        encoder_map: Option<&'a mut [[EncoderAction; NUM_ENCODER]; NUM_LAYER]>,
        mut behavior: BehaviorConfig,
    ) -> Self {
        // If the storage is initialized, read keymap from storage

        // fill up the empty places so new combos/forks can be configured via Vial
        fill_vec(&mut behavior.combo.combos);
        //reorder the combos
        _reorder_combos(&mut behavior.combo.combos);

        fill_vec(&mut behavior.fork.forks);

        KeyMap {
            layers: action_map,
            encoders: encoder_map,
            layer_state: [false; NUM_LAYER],
            default_layer: 0,
            layer_cache: [[0; COL]; ROW],
            behavior,
            #[cfg(feature = "controller")]
            controller_pub: unwrap!(CONTROLLER_CHANNEL.publisher()),
        }
    }
    #[cfg(feature = "storage")]
    pub async fn new_from_storage<F: NorFlash>(
        action_map: &'a mut [[[KeyAction; COL]; ROW]; NUM_LAYER],
        mut encoder_map: Option<&'a mut [[EncoderAction; NUM_ENCODER]; NUM_LAYER]>,
        storage: Option<&mut Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
        mut behavior: BehaviorConfig,
    ) -> Self {
        // If the storage is initialized, read keymap from storage
        fill_vec(&mut behavior.combo.combos);
        fill_vec(&mut behavior.fork.forks);
        fill_vec(&mut behavior.tap_dance.tap_dances);

        if let Some(storage) = storage {
            if {
                Ok(())
                    // Read keymap to `action_map`
                    .and(storage.read_keymap(action_map, &mut encoder_map).await)
                    // Read macro cache
                    .and(
                        storage
                            .read_macro_cache(&mut behavior.keyboard_macros.macro_sequences)
                            .await,
                    )
                    // Read combo cache
                    .and(storage.read_combos(&mut behavior.combo.combos).await)
                    // Read fork cache
                    .and(storage.read_forks(&mut behavior.fork.forks).await)
                    // Read tap dance cache
                    .and(storage.read_tap_dances(&mut behavior.tap_dance.tap_dances).await)
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
            behavior,
            #[cfg(feature = "controller")]
            controller_pub: unwrap!(CONTROLLER_CHANNEL.publisher()),
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

    pub(crate) fn get_next_macro_operation(&self, macro_start_idx: usize, offset: usize) -> (MacroOperation, usize) {
        MacroOperation::get_next_macro_operation(
            &self.behavior.keyboard_macros.macro_sequences,
            macro_start_idx,
            offset,
        )
    }

    pub(crate) fn get_macro_sequence_start(&self, guessed_macro_start_idx: u8) -> Option<usize> {
        MacroOperation::get_macro_sequence_start(
            &self.behavior.keyboard_macros.macro_sequences,
            guessed_macro_start_idx,
        )
    }

    pub(crate) fn set_action_at(&mut self, row: usize, col: usize, layer_num: usize, action: KeyAction) {
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

    pub(crate) fn get_encoder_with_layer_cache(&self, encoder_event: RotaryEncoderEvent) -> Option<&EncoderAction> {
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

        #[cfg(feature = "controller")]
        {
            let layer = self.get_activated_layer();
            send_controller_event(&mut self.controller_pub, ControllerEvent::Layer(layer));
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

        #[cfg(feature = "controller")]
        {
            let layer = self.get_activated_layer();
            send_controller_event(&mut self.controller_pub, ControllerEvent::Layer(layer));
        }
    }

    //order combos by their actions length
    pub(crate) fn reorder_combos(&mut self) {
        _reorder_combos(&mut self.behavior.combo.combos);
    }
}

#[cfg(test)]
mod test {
    use super::{Combo, _reorder_combos};
    use crate::action::KeyAction;
    use crate::fork::{Fork, StateBits};
    use crate::hid_state::HidModifiers;
    use crate::keycode::KeyCode;
    use crate::keymap::fill_vec;
    use crate::{k, COMBO_MAX_NUM, FORK_MAX_NUM};

    #[test]
    fn test_fill_vec() {
        let mut combos: heapless::Vec<_, COMBO_MAX_NUM> = heapless::Vec::from_slice(&[
            Combo::new([k!(A), k!(B), k!(C), k!(D)], k!(Z), None),
            Combo::new([k!(A), k!(B)], k!(X), None),
            Combo::new([k!(A), k!(B), k!(C)], k!(Y), None),
        ])
        .unwrap();

        fill_vec(&mut combos);

        assert_eq!(combos.len(), COMBO_MAX_NUM);

        let mut forks: heapless::Vec<_, FORK_MAX_NUM> = heapless::Vec::from_slice(&[
            Fork::new(
                k!(A),
                k!(Y),
                k!(F),
                StateBits::default(),
                StateBits::default(),
                HidModifiers::new(),
                false,
            ),
            Fork::new(
                k!(B),
                k!(B),
                k!(F),
                StateBits::default(),
                StateBits::default(),
                HidModifiers::new(),
                false,
            ),
            Fork::new(
                k!(C),
                k!(Y),
                k!(Y),
                StateBits::default(),
                StateBits::default(),
                HidModifiers::new(),
                false,
            ),
        ])
        .unwrap();

        fill_vec(&mut forks);

        assert_eq!(forks.len(), FORK_MAX_NUM);
    }

    #[test]
    fn test_combo_reordering() {
        let combos_raw = [
            Combo::new([k!(A), k!(B), k!(C), k!(D)], k!(Z), None),
            Combo::new([k!(A), k!(B)], k!(X), None),
            Combo::new([k!(A), k!(B), k!(C)], k!(Y), None),
        ];
        let mut combos = heapless::Vec::from_slice(&combos_raw).unwrap();

        _reorder_combos(&mut combos);
        fill_vec(&mut combos);

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
            vec![KeyCode::Z as u16, KeyCode::Y as u16, KeyCode::X as u16, 0, 0, 0, 0, 0]
        );
    }
}
