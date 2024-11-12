use crate::{
    action::KeyAction,
    keyboard::KeyEvent,
    keyboard_macro::{MacroOperation, MACRO_SPACE_SIZE},
    keycode::KeyCode,
    reboot_keyboard,
    storage::Storage,
};
use defmt::{error, warn};
use embedded_storage_async::nor_flash::NorFlash;
use num_enum::FromPrimitive;

/// Keymap represents the stack of layers.
///
/// The conception of Keymap in rmk is borrowed from qmk: <https://docs.qmk.fm/#/keymap>.
///
/// Keymap should be binded to the actual pcb matrix definition.
/// RMK detects hardware key strokes, uses tuple `(row, col, layer)` to retrieve the action from Keymap.
pub(crate) struct KeyMap<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize> {
    /// Layers
    pub(crate) layers: &'a mut [[[KeyAction; COL]; ROW]; NUM_LAYER],
    /// Current state of each layer
    layer_state: [bool; NUM_LAYER],
    /// Default layer number, max: 32
    default_layer: u8,
    /// Layer cache
    layer_cache: [[u8; COL]; ROW],
    /// Macro cache
    pub(crate) macro_cache: [u8; MACRO_SPACE_SIZE],
}

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize>
    KeyMap<'a, ROW, COL, NUM_LAYER>
{
    pub(crate) async fn new(action_map: &'a mut [[[KeyAction; COL]; ROW]; NUM_LAYER]) -> Self {
        KeyMap {
            layers: action_map,
            layer_state: [false; NUM_LAYER],
            default_layer: 0,
            layer_cache: [[0; COL]; ROW],
            macro_cache: [0; MACRO_SPACE_SIZE],
        }
    }

    pub(crate) async fn new_from_storage<F: NorFlash>(
        action_map: &'a mut [[[KeyAction; COL]; ROW]; NUM_LAYER],
        storage: Option<&mut Storage<F, ROW, COL, NUM_LAYER>>,
    ) -> Self {
        // If the storage is initialized, read keymap from storage
        let mut macro_cache = [0; MACRO_SPACE_SIZE];
        if let Some(storage) = storage {
            // Read keymap to `action_map`
            if storage.read_keymap(action_map).await.is_err() {
                error!("Keymap reading aborted by an error, clearing the storage...");
                // Dont sent flash message here, since the storage task is not running yet
                sequential_storage::erase_all(&mut storage.flash, storage.storage_range.clone())
                    .await
                    .ok();

                reboot_keyboard();
            } else {
                // Read macro cache
                if storage.read_macro_cache(&mut macro_cache).await.is_err() {
                    error!("Wrong macro cache, clearing the storage...");
                    sequential_storage::erase_all(
                        &mut storage.flash,
                        storage.storage_range.clone(),
                    )
                    .await
                    .ok();

                    reboot_keyboard();
                }
            }
        }

        KeyMap {
            layers: action_map,
            layer_state: [false; NUM_LAYER],
            default_layer: 0,
            layer_cache: [[0; COL]; ROW],
            macro_cache,
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

    fn get_activated_layer(&self) -> u8 {
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
