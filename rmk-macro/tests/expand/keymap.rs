use rmk_macro::keymap;

const ROW: usize = 2;
const COL: usize = 3;
const NUM_LAYER: usize = 2;

const KEYMAP: [[[rmk::types::action::KeyAction; COL]; ROW]; NUM_LAYER] = keymap! {
    matrix_map: "
        (0,0) (0,1) (0,2)
        (1,0) (1,1) (1,2)
    ",
    layers: [
        {
            layer: 0,
            name: "base",
            layout: "
                A B C
                D E F
            "
        },
        {
            layer: 1,
            name: "fn",
            layout: "
                F1 F2 F3
                F4 F5 F6
            "
        }
    ]
};

fn main() {}
