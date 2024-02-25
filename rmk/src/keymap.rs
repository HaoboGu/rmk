use crate::{
    action::KeyAction,
    eeprom::{eeconfig::Eeconfig, Eeprom},
    matrix::KeyState,
};
use defmt::warn;
use embedded_alloc::Heap;
use embedded_storage::nor_flash::NorFlash;

#[global_allocator]
static HEAP: Heap = Heap::empty();

pub(crate) struct KeyMapConfig {
    /// Number of rows.
    pub(crate) row: usize,
    /// Number of columns.
    pub(crate) col: usize,
    /// Number of layer
    pub(crate) layer: usize,
}

/// Keymap represents the stack of layers.
///
/// The conception of Keymap in rmk is borrowed from qmk: <https://docs.qmk.fm/#/keymap>.
///
/// Keymap should be binded to the actual pcb matrix definition.
/// RMK detects hardware key strokes, uses tuple `(row, col, layer)` to retrieve the action from Keymap.
pub struct KeyMap<
    F: NorFlash,
    const EEPROM_SIZE: usize,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
> {
    /// Layers
    pub(crate) layers: [[[KeyAction; COL]; ROW]; NUM_LAYER],
    /// Current state of each layer
    layer_state: [bool; NUM_LAYER],
    /// Default layer number, max: 32
    default_layer: u8,
    /// Layer cache
    layer_cache: [[u8; COL]; ROW],
    /// Eeprom for storing keymap
    pub(crate) eeprom: Option<Eeprom<F, EEPROM_SIZE>>,
}

impl<
        F: NorFlash,
        const EEPROM_SIZE: usize,
        const ROW: usize,
        const COL: usize,
        const NUM_LAYER: usize,
    > KeyMap<F, EEPROM_SIZE, ROW, COL, NUM_LAYER>
{
    /// Initialize a keymap from a matrix of actions
    ///
    /// # Arguments
    ///
    /// * `action_map` - [KeyAction] matrix defined in keymap
    /// * `storage` - backend storage for eeprom, used for saving keyboard data persistently
    /// * `eeconfig` - keyboard configurations which should be stored in eeprom
    pub fn new(
        mut action_map: [[[KeyAction; COL]; ROW]; NUM_LAYER],
        storage: Option<F>,
        eeconfig: Option<Eeconfig>,
    ) -> KeyMap<F, EEPROM_SIZE, ROW, COL, NUM_LAYER> {
        // Initialize the allocator at the very beginning of the initialization of the keymap
        {
            use core::mem::MaybeUninit;
            // 512 bytes heap size
            const HEAP_SIZE: usize = 512;
            // Check page_size and heap size
            assert!(F::WRITE_SIZE < HEAP_SIZE);
            static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
            unsafe { HEAP.init(HEAP_MEM.as_ptr() as usize, HEAP_SIZE) }
        }

        // Initialize eeprom, if success, re-load keymap from it
        let eeprom = match storage {
            Some(s) => {
                let e = Eeprom::new(s, eeconfig, &action_map);
                // If eeprom is initialized, read keymap from it.
                match e {
                    Some(e) => {
                        e.read_keymap(&mut action_map);
                        Some(e)
                    }
                    None => None,
                }
            }
            None => None,
        };

        KeyMap {
            layers: action_map,
            layer_state: [false; NUM_LAYER],
            default_layer: 0,
            layer_cache: [[0; COL]; ROW],
            eeprom,
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
