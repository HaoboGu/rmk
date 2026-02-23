use rmk::types::action::KeyAction;
use rmk::keymap;

pub(crate) const COL: usize = 3;
pub(crate) const ROW: usize = 4;
pub(crate) const NUM_LAYER: usize = 2;

const DEFAULT_KEYMAP: [[[KeyAction; COL]; ROW]; NUM_LAYER] = keymap! {
    matrix_map: "
        (0,0) (0,1) (0,2)
        (1,0) (1,1) (1,2)
        (2,0) (2,1) (2,2)
        (3,0) (3,1) (3,2)
    ",
    aliases: {
        fn_layer = "MO(fn)",
    },
    layers: [
        {
            layer: 0,
            name: "base",
            layout: "
                AudioVolUp  B           AudioVolDown
                Kp4         LShift      Kp6
                @fn_layer   Kp2         Kp3
                @fn_layer   No          Kp0
            "
        },
        {
            layer: 1,
            name: "fn",
            layout: "
                Kp7     Kp8     Kp9
                Kp4     LCtrl   Kp6
                MO(1)   Kp2     Kp3
                MO(1)   No      Kp0
            "
        }
    ]
};

pub const fn get_default_keymap() -> [[[KeyAction; COL]; ROW]; NUM_LAYER] {
    DEFAULT_KEYMAP
}
