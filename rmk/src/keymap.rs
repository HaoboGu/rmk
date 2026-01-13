use rmk_types::action::{EncoderAction, KeyAction};
#[cfg(all(feature = "storage", feature = "host"))]
use {
    crate::{boot::reboot_keyboard, storage::Storage},
    embedded_storage_async::nor_flash::NorFlash,
};

use crate::config::{BehaviorConfig, PositionalConfig};
use crate::event::{KeyboardEvent, KeyboardEventPos};
use crate::input_device::rotary_encoder::Direction;
use crate::keyboard_macros::MacroOperation;
#[cfg(feature = "vial_lock")]
use crate::matrix::MatrixState;

/// Keymap represents the stack of layers.
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
    /// Rotary encoder cache
    encoder_layer_cache: [[u8; 2]; NUM_ENCODER],
    /// Options for configurable action behavior
    pub(crate) behavior: &'a mut BehaviorConfig,
    pub positional_config: &'a mut PositionalConfig<ROW, COL>,
    /// Matrix state
    #[cfg(feature = "vial_lock")]
    pub(crate) matrix_state: MatrixState<ROW, COL>,
    /// Mouse button state (buttons 0-7 as bits)
    pub(crate) mouse_buttons: u8,
}

/// fills up the vector to its capacity
pub(crate) fn fill_vec<T: Default + Clone, const N: usize>(vector: &mut heapless::Vec<T, N>) {
    vector
        .resize(vector.capacity(), T::default())
        .expect("impossible error, as we resize to the capacity of the vector!");
}

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    pub async fn new(
        action_map: &'a mut [[[KeyAction; COL]; ROW]; NUM_LAYER],
        encoder_map: Option<&'a mut [[EncoderAction; NUM_ENCODER]; NUM_LAYER]>,
        behavior: &'a mut BehaviorConfig,
        positional_config: &'a mut PositionalConfig<ROW, COL>,
    ) -> Self {
        // If the storage is initialized, read keymap from storage

        fill_vec(&mut behavior.fork.forks); // Is this needed? (has no Vial support)
        fill_vec(&mut behavior.morse.morses);

        KeyMap {
            layers: action_map,
            encoders: encoder_map,
            layer_state: [false; NUM_LAYER],
            default_layer: 0,
            layer_cache: [[0; COL]; ROW],
            encoder_layer_cache: [[0; 2]; NUM_ENCODER],
            behavior,
            positional_config,
            #[cfg(feature = "vial_lock")]
            matrix_state: MatrixState::new(),
            mouse_buttons: 0,
        }
    }

    #[cfg(all(feature = "storage", feature = "host"))]
    pub async fn new_from_storage<F: NorFlash>(
        action_map: &'a mut [[[KeyAction; COL]; ROW]; NUM_LAYER],
        mut encoder_map: Option<&'a mut [[EncoderAction; NUM_ENCODER]; NUM_LAYER]>,
        storage: Option<&mut Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
        behavior: &'a mut BehaviorConfig,
        positional_config: &'a mut PositionalConfig<ROW, COL>,
    ) -> Self {
        // If the storage is initialized, read keymap from storage
        fill_vec(&mut behavior.fork.forks); // Is this needed? (has no Vial support)
        fill_vec(&mut behavior.morse.morses);

        if let Some(storage) = storage
            && {
                Ok(())
                    // Read keymap to `action_map`
                    .and(storage.read_keymap(action_map, &mut encoder_map).await)
                    // Read behavior config
                    .and(storage.read_behavior_config(behavior).await)
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
                    // Read morse cache
                    .and(storage.read_morses(&mut behavior.morse.morses).await)
            }
            .is_err()
        {
            error!("Failed to read from storage, clearing...");
            sequential_storage::erase_all(&mut storage.flash, storage.storage_range.clone())
                .await
                .ok();

            reboot_keyboard();
        }

        KeyMap {
            layers: action_map,
            encoders: encoder_map,
            layer_state: [false; NUM_LAYER],
            default_layer: 0,
            layer_cache: [[0; COL]; ROW],
            encoder_layer_cache: [[0; 2]; NUM_ENCODER],
            behavior,
            positional_config,
            #[cfg(feature = "vial_lock")]
            matrix_state: MatrixState::new(),
            mouse_buttons: 0,
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

    pub(crate) fn set_action_at(&mut self, pos: KeyboardEventPos, layer_num: usize, action: KeyAction) {
        match pos {
            KeyboardEventPos::Key(key_pos) => {
                let row = key_pos.row as usize;
                let col = key_pos.col as usize;
                self.layers[layer_num][row][col] = action;
            }
            KeyboardEventPos::RotaryEncoder(encoder_pos) => {
                if let Some(encoders) = &mut self.encoders
                    && let Some(encoder_action) = encoders[layer_num].get_mut(encoder_pos.id as usize)
                {
                    match encoder_pos.direction {
                        Direction::Clockwise => encoder_action.set_clockwise(action),
                        Direction::CounterClockwise => encoder_action.set_counter_clockwise(action),
                        Direction::None => {}
                    }
                }
            }
        }
    }

    /// Fetch the action in keymap, with layer cache
    pub(crate) fn get_action_at(&self, pos: KeyboardEventPos, layer_num: usize) -> KeyAction {
        match pos {
            KeyboardEventPos::Key(key_pos) => {
                let row = key_pos.row as usize;
                let col = key_pos.col as usize;
                self.layers[layer_num][row][col]
            }
            KeyboardEventPos::RotaryEncoder(encoder_pos) => {
                // Get the action from the keymap
                if let Some(encoders) = &self.encoders
                    && let Some(encoder_action) = encoders[layer_num].get(encoder_pos.id as usize)
                    && encoder_pos.direction != Direction::None
                {
                    return match encoder_pos.direction {
                        Direction::Clockwise => encoder_action.clockwise(),
                        Direction::CounterClockwise => encoder_action.counter_clockwise(),
                        Direction::None => KeyAction::No,
                    };
                }
                KeyAction::No
            }
        }
    }

    /// Fetch the action in keymap, with layer cache
    pub(crate) fn get_action_with_layer_cache(&mut self, event: KeyboardEvent) -> KeyAction {
        if !event.pressed {
            // Releasing a pressed key, use cached layer and restore the cache
            let layer = self.pop_layer_from_cache(event.pos);
            let action = self.get_action_at(event.pos, layer as usize);
            return action;
        }

        // Iterate from higher layer to lower layer, the lowest checked layer is the default layer
        match event.pos {
            KeyboardEventPos::Key(key_pos) => {
                let row = key_pos.row as usize;
                let col = key_pos.col as usize;
                for (layer_idx, layer) in self.layers.iter().enumerate().rev() {
                    if self.layer_state[layer_idx] || layer_idx as u8 == self.default_layer {
                        // This layer is activated
                        let action = layer[row][col];
                        if action == KeyAction::Transparent {
                            continue;
                        }

                        // Found a valid action in the layer, cache it
                        self.save_layer_cache(event.pos, layer_idx as u8);

                        return action;
                    }

                    if layer_idx as u8 == self.default_layer {
                        // No action
                        break;
                    }
                }
            }
            KeyboardEventPos::RotaryEncoder(encoder_pos) => {
                if let Some(encoders) = &self.encoders {
                    for (layer_idx, layer) in encoders.iter().enumerate().rev() {
                        // Get the KeyAction for rotary_encoder_event from self.encoders
                        if self.layer_state[layer_idx] || layer_idx as u8 == self.default_layer {
                            // This layer is activated
                            if let Some(encoder_action) = layer.get(encoder_pos.id as usize) {
                                let action = match encoder_pos.direction {
                                    Direction::Clockwise => encoder_action.clockwise(),
                                    Direction::CounterClockwise => encoder_action.counter_clockwise(),
                                    Direction::None => KeyAction::No,
                                };
                                if action == KeyAction::Transparent {
                                    continue;
                                }
                                self.save_layer_cache(event.pos, layer_idx as u8);
                                return action;
                            }
                        }
                        if layer_idx as u8 == self.default_layer {
                            // No action
                            break;
                        }
                    }
                }
            }
        }

        KeyAction::No
    }

    pub(crate) fn get_activated_layer(&self) -> u8 {
        for (layer_idx, _) in self.layers.iter().enumerate().rev() {
            if self.layer_state[layer_idx] || layer_idx as u8 == self.default_layer {
                return layer_idx as u8;
            }
        }

        self.default_layer
    }

    fn pop_layer_from_cache(&mut self, pos: KeyboardEventPos) -> u8 {
        match pos {
            KeyboardEventPos::Key(key_pos) => {
                let row = key_pos.row as usize;
                let col = key_pos.col as usize;
                let layer = self.layer_cache[row][col];
                self.layer_cache[row][col] = self.default_layer;

                layer
            }
            KeyboardEventPos::RotaryEncoder(encoder_pos) => {
                if let Some(cache) = self.encoder_layer_cache.get_mut(encoder_pos.id as usize)
                    && encoder_pos.direction != Direction::None
                {
                    let layer = cache[encoder_pos.direction as usize];
                    cache[encoder_pos.direction as usize] = self.default_layer;
                    return layer;
                }
                // Wrong argument, return the default layer
                self.default_layer
            }
        }
    }

    fn save_layer_cache(&mut self, pos: KeyboardEventPos, layer_num: u8) {
        match pos {
            KeyboardEventPos::Key(key_pos) => {
                let row = key_pos.row as usize;
                let col = key_pos.col as usize;
                self.layer_cache[row][col] = layer_num;
            }

            KeyboardEventPos::RotaryEncoder(encoder_pos) => {
                // Check if the rotary encoder id and direction are valid
                if let Some(cache) = self.encoder_layer_cache.get_mut(encoder_pos.id as usize)
                    && encoder_pos.direction != Direction::None
                {
                    cache[encoder_pos.direction as usize] = layer_num;
                }
            }
        }
    }

    /// Update fn layer state, this is only used for fn1(fn3) + fn2(fn3)
    pub(crate) fn update_fn_layer_state(&mut self) {
        if NUM_LAYER > 3 {
            self.layer_state[3] = self.layer_state[1] && self.layer_state[2];
            #[cfg(feature = "controller")]
            {
                let layer = self.get_activated_layer();
                crate::event::publish_controller_event(crate::event::LayerChangeEvent { layer });
            }
        }
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
            crate::event::publish_controller_event(crate::event::LayerChangeEvent { layer });
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
            crate::event::publish_controller_event(crate::event::LayerChangeEvent { layer });
        }
    }
}

#[cfg(test)]
mod test {
    use rmk_types::modifier::ModifierCombination;

    use crate::combo::{Combo, ComboConfig};
    use crate::fork::{Fork, StateBits};
    use crate::keymap::fill_vec;
    use crate::{COMBO_MAX_NUM, FORK_MAX_NUM, k};

    #[test]
    fn test_fill_vec() {
        let mut combos: heapless::Vec<_, COMBO_MAX_NUM> = heapless::Vec::from_slice(&[
            Combo::new(ComboConfig {
                actions: [k!(A), k!(B), k!(C), k!(D)],
                output: k!(Z),
                layer: None,
            }),
            Combo::new(ComboConfig {
                actions: [k!(A), k!(B), k!(No), k!(No)],
                output: k!(X),
                layer: None,
            }),
            Combo::new(ComboConfig {
                actions: [k!(A), k!(B), k!(C), k!(No)],
                output: k!(Y),
                layer: None,
            }),
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
                ModifierCombination::new(),
                false,
            ),
            Fork::new(
                k!(B),
                k!(B),
                k!(F),
                StateBits::default(),
                StateBits::default(),
                ModifierCombination::new(),
                false,
            ),
            Fork::new(
                k!(C),
                k!(Y),
                k!(Y),
                StateBits::default(),
                StateBits::default(),
                ModifierCombination::new(),
                false,
            ),
        ])
        .unwrap();

        fill_vec(&mut forks);

        assert_eq!(forks.len(), FORK_MAX_NUM);
    }
}
