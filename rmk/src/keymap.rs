use crate::{action::KeyAction, matrix::KeyState};
use log::warn;

pub struct KeyMapConfig {
    /// Number of rows.
    pub row: usize,
    /// Number of columns.
    pub col: usize,
    /// Number of layer
    pub layer: usize,
}

/// KeyMap represents the stack of layers.
/// The conception of KeyMap in rmk is borrowed from qmk: <https://docs.qmk.fm/#/keymap>.
/// Keymap should be bind to the actual pcb matrix definition.
/// RMK detects hardware key strokes, uses (row,col) to retrieve the action from KeyMap.
pub struct KeyMap<const ROW: usize, const COL: usize, const NUM_LAYER: usize> {
    /// Layers
    pub(crate) layers: [[[KeyAction; COL]; ROW]; NUM_LAYER],
    /// Current state of each layer
    layer_state: [bool; NUM_LAYER],
    /// Default layer number, max: 32
    default_layer: u8,
    /// Layer cache
    layer_cache: [[u8; COL]; ROW],
}

impl<const ROW: usize, const COL: usize, const NUM_LAYER: usize> KeyMap<ROW, COL, NUM_LAYER> {
    /// Initialize a keymap from a matrix of actions
    pub fn new(action_map: [[[KeyAction; COL]; ROW]; NUM_LAYER]) -> KeyMap<ROW, COL, NUM_LAYER> {
        KeyMap {
            layers: action_map,
            layer_state: [false; NUM_LAYER],
            default_layer: 0,
            layer_cache: [[0; COL]; ROW],
        }
    }

    pub fn get_keymap_config(&self) -> (usize, usize, usize) {
        (ROW, COL, NUM_LAYER)
    }

    pub fn set_action_at(&mut self, row: usize, col: usize, layer_num: usize, action: KeyAction) {
        self.layers[layer_num][row][col] = action;
    }

    /// Fetch the action in keymap, with layer cache
    pub fn get_action_at(&mut self, row: usize, col: usize, layer_num: usize) -> KeyAction {
        self.layers[layer_num][row][col]
    }

    /// Fetch the action in keymap, with layer cache
    pub fn get_action_with_layer_cache(
        &mut self,
        row: usize,
        col: usize,
        key_state: KeyState,
    ) -> KeyAction {
        if !key_state.pressed && key_state.changed {
            // Releasing a pressed key, use cached layer and restore the cache
            let layer = self.pop_layer_from_cache(row, col);
            return self.layers[layer as usize][row][col];
        }

        // Iterate from higher layer to lower layer, the lowest checked layer is the default layer
        for (layer_idx, layer) in self.layers.iter().enumerate().rev() {
            if self.layer_state[layer_idx] || layer_idx as u8 == self.default_layer {
                // This layer is activated
                let action = layer[row][col];
                if action == KeyAction::Transparent || action == KeyAction::No {
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

    /// Activate given layer
    pub fn activate_layer(&mut self, layer_num: u8) {
        if layer_num as usize >= NUM_LAYER {
            warn!("Not a valid layer {layer_num}, keyboard supports only {NUM_LAYER} layers");
            return;
        }
        self.layer_state[layer_num as usize] = true;
    }

    /// Deactivate given layer
    pub fn deactivate_layer(&mut self, layer_num: u8) {
        if layer_num as usize >= NUM_LAYER {
            warn!("Not a valid layer {layer_num}, keyboard supports only {NUM_LAYER} layers");
            return;
        }
        self.layer_state[layer_num as usize] = false;
    }

    /// Toggle given layer
    pub fn toggle_layer(&mut self, layer_num: u8) {
        if layer_num as usize >= NUM_LAYER {
            warn!("Not a valid layer {layer_num}, keyboard supports only {NUM_LAYER} layers");
            return;
        }

        self.layer_state[layer_num as usize] = !self.layer_state[layer_num as usize];
    }
}
