use crate::action::KeyAction;

/// KeyMap represents the stack of layers.
/// The conception of KeyMap in rmk is borrowed from qmk: https://docs.qmk.fm/#/keymap.
/// Keymap should be bind to the actual pcb matrix definition by KeyPos.
/// RMK detects hardware key strokes, uses KeyPos to retrieve the action from KeyMap.
pub struct KeyMap<const ROW: usize, const COL: usize, const NUM_LAYER: usize> {
    /// Layers
    layers: [[[KeyAction; COL]; ROW]; NUM_LAYER],
    /// Current state of each layer
    layer_state: [bool; NUM_LAYER],
    /// Default layer number, max: 32
    default_layer: u8,
}

impl<const ROW: usize, const COL: usize, const NUM_LAYER: usize> KeyMap<ROW, COL, NUM_LAYER> {
    /// Initialize a keymap from a matrix of actions
    pub fn new(action_map: [[[KeyAction; COL]; ROW]; NUM_LAYER]) -> KeyMap<ROW, COL, NUM_LAYER> {
        KeyMap {
            layers: action_map,
            layer_state: [true; NUM_LAYER],
            default_layer: 0,
        }
    }

    /// Fetch the action in keymap
    /// FIXME: When the layer is changed, release event should be processed in the original layer(layer cache)
    /// See https://github.com/qmk/qmk_firmware/blob/master/quantum/action_layer.c#L299
    pub fn get_action(&self, row: usize, col: usize) -> KeyAction {
        for (layer_idx, layer) in self.layers.iter().enumerate() {
            if self.layer_state[layer_idx] {
                // This layer is activated
                let action = layer[col][row];
                if action == KeyAction::Transparent || action == KeyAction::No {
                    continue;
                }
                return action;
            }
        }

        KeyAction::No
    }
}
