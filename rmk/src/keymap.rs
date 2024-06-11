use crate::{action::KeyAction, matrix::KeyState, storage::Storage};
use defmt::{error, warn};
use embedded_storage_async::nor_flash::NorFlash;

/// Keymap represents the stack of layers.
///
/// The conception of Keymap in rmk is borrowed from qmk: <https://docs.qmk.fm/#/keymap>.
///
/// Keymap should be binded to the actual pcb matrix definition.
/// RMK detects hardware key strokes, uses tuple `(row, col, layer)` to retrieve the action from Keymap.
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
    pub async fn new(action_map: [[[KeyAction; COL]; ROW]; NUM_LAYER]) -> Self {
        KeyMap {
            layers: action_map,
            layer_state: [false; NUM_LAYER],
            default_layer: 0,
            layer_cache: [[0; COL]; ROW],
        }
    }
    pub async fn new_from_storage<F: NorFlash>(
        mut action_map: [[[KeyAction; COL]; ROW]; NUM_LAYER],
        storage: Option<&mut Storage<F>>,
    ) -> Self {
        // If the storage is initialized, read keymap from storage
        if let Some(storage) = storage {
            if storage.read_keymap(&mut action_map).await.is_err() {
                error!("Keymap reading aborted by an error, clearing the storage...");
                sequential_storage::erase_all(&mut storage.flash, storage.storage_range.clone()).await.ok();
                // TODO: The storage is erased, reset the device
            }
        }

        KeyMap {
            layers: action_map,
            layer_state: [false; NUM_LAYER],
            default_layer: 0,
            layer_cache: [[0; COL]; ROW],
        }
    }

    pub(crate) fn get_keymap_config(&self) -> (usize, usize, usize) {
        (ROW, COL, NUM_LAYER)
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
    pub(crate) fn get_action_with_layer_cache(
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

        if !key_state.changed && key_state.pressed {
            // If current key_state doesn't change, and the key is pressed, use the cached layer.
            // 
            // This situation is specifically for tap/hold, when the key is held, the layer could be continuously checked.
            // The layer cache should not be popped in this case.
            // This function should return the cached layer to make layer tap/hold action performs correctly.
            let layer = self.layer_cache[row][col];
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
    pub(crate) fn activate_layer(&mut self, layer_num: u8) {
        if layer_num as usize >= NUM_LAYER {
            warn!(
                "Not a valid layer {}, keyboard supports only {} layers",
                layer_num, NUM_LAYER
            );
            return;
        }
        self.layer_state[layer_num as usize] = true;
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
}
