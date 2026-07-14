use rmk::sim::{KeymapOverride, SimKeyboardSetup};
use rmk::types::action::{Action, KeyAction};
use rmk::types::keycode::{HidKeyCode, KeyCode};
use rmk::types::modifier::ModifierCombination;
use rmk::types::morse::MorseProfile;
use rmk::{k, lt, mt, td};

pub const SIMPLE_MORSE_KEY_OVERRIDES: [KeymapOverride; 10] = [
    KeymapOverride::new(0, 0, 0, k!(A)),
    KeymapOverride::new(0, 0, 1, mt!(B, ModifierCombination::LSHIFT)),
    KeymapOverride::new(0, 0, 2, mt!(C, ModifierCombination::LGUI)),
    KeymapOverride::new(0, 0, 3, lt!(1, D)),
    KeymapOverride::new(0, 0, 4, td!(0)),
    KeymapOverride::new(1, 0, 0, k!(Kp1)),
    KeymapOverride::new(1, 0, 1, k!(Kp2)),
    KeymapOverride::new(1, 0, 2, k!(Kp3)),
    KeymapOverride::new(1, 0, 3, k!(Kp4)),
    KeymapOverride::new(1, 0, 4, k!(Kp5)),
];

pub const HRM_MORSE_KEY_OVERRIDES: [KeymapOverride; 10] = [
    KeymapOverride::new(0, 0, 0, k!(A)),
    KeymapOverride::new(0, 0, 1, mt!(B, ModifierCombination::LSHIFT)),
    KeymapOverride::new(0, 0, 2, mt!(C, ModifierCombination::LGUI)),
    KeymapOverride::new(0, 0, 3, lt!(1, D)),
    KeymapOverride::new(0, 0, 4, mt!(E, ModifierCombination::LALT)),
    KeymapOverride::new(1, 0, 0, k!(Kp1)),
    KeymapOverride::new(1, 0, 1, k!(Kp2)),
    KeymapOverride::new(1, 0, 2, k!(Kp3)),
    KeymapOverride::new(1, 0, 3, k!(Kp4)),
    KeymapOverride::new(1, 0, 4, k!(Kp5)),
];

pub const TEST_MORSE_PATTERNS: [(u16, Action); 8] = [
    (0b1_01, Action::Key(KeyCode::Hid(HidKeyCode::A))),
    (0b1_1000, Action::Key(KeyCode::Hid(HidKeyCode::B))),
    (0b1_1010, Action::Key(KeyCode::Hid(HidKeyCode::C))),
    (0b1_101, Action::Key(KeyCode::Hid(HidKeyCode::K))),
    (0b1_11, Action::Key(KeyCode::Hid(HidKeyCode::M))),
    (0b1_111, Action::Key(KeyCode::Hid(HidKeyCode::O))),
    (0b1_010, Action::Key(KeyCode::Hid(HidKeyCode::R))),
    (0b1_000, Action::Key(KeyCode::Hid(HidKeyCode::S))),
];

pub const SIMPLE_MORSE_SETUP: SimKeyboardSetup = SimKeyboardSetup::new()
    .keys(&SIMPLE_MORSE_KEY_OVERRIDES)
    .morse_patterns(&TEST_MORSE_PATTERNS);

pub const HRM_MORSE_SETUP: SimKeyboardSetup = SimKeyboardSetup::new()
    .keys(&HRM_MORSE_KEY_OVERRIDES)
    .morse_patterns(&TEST_MORSE_PATTERNS);

pub const MORSE_COMBO_KEY: KeyAction = KeyAction::TapHold(
    Action::Key(KeyCode::Hid(HidKeyCode::B)),
    Action::Modifier(ModifierCombination::LSHIFT),
    MorseProfile::const_default(),
);
pub const MORSE_COMBO_KEY_2: KeyAction = KeyAction::TapHold(
    Action::Key(KeyCode::Hid(HidKeyCode::C)),
    Action::Modifier(ModifierCombination::LGUI),
    MorseProfile::const_default(),
);
pub const MORSE_COMBO_KEY_3: KeyAction = KeyAction::TapHold(
    Action::Key(KeyCode::Hid(HidKeyCode::D)),
    Action::LayerOn(1),
    MorseProfile::const_default(),
);

pub const MORSE_2_KEY_COMBOS: [([KeyAction; 2], KeyAction); 2] = [
    ([MORSE_COMBO_KEY, MORSE_COMBO_KEY_2], k!(X)),
    ([k!(A), MORSE_COMBO_KEY], k!(Y)),
];

pub const MORSE_3_KEY_COMBOS: [([KeyAction; 3], KeyAction); 1] =
    [([MORSE_COMBO_KEY, MORSE_COMBO_KEY_2, MORSE_COMBO_KEY_3], k!(Z))];
