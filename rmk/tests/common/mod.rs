pub mod morse;
#[cfg(feature = "rynk")]
pub mod rynk_hid_link;
#[cfg(feature = "rynk")]
pub mod rynk_link;
pub mod test_block_on;

use rmk::types::action::KeyAction;
use rmk::types::modifier::ModifierCombination;
use rmk::{a, k, layer, lt, mo, shifted, th, wm};

// `embassy-time`'s MockDriver is a process-global singleton, so running the
// suite under plain `cargo test` lets tests race on it and hang at the 60 s
// virtual-time kill switch in `test_block_on`. Abort at test-binary startup
// with a pointer to the right runner instead of making the user wait for that
// timeout.
#[ctor::ctor(unsafe)]
fn require_nextest() {
    if std::env::var_os("NEXTEST").is_none() {
        eprintln!(
            "\nrmk tests must run under cargo-nextest (embassy-time's MockDriver \
             is a process-global singleton and needs per-test process isolation).\n\
             \n  cargo install cargo-nextest --locked\n\n\
             Then from rmk/:\n\n  \
             cargo nextest run --no-default-features \
             --features=split,vial,storage,async_matrix,_ble\n\n\
             Or for the full feature matrix: `sh scripts/test_all.sh` from the repo root.\n"
        );
        std::process::exit(1);
    }
}

// Init logger for tests
#[ctor::ctor(unsafe)]
pub fn init_log() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .is_test(true)
        .try_init();
}

pub const KC_LCTRL: u8 = 1 << 0;
pub const KC_LSHIFT: u8 = 1 << 1;
pub const KC_LALT: u8 = 1 << 2;
pub const KC_LGUI: u8 = 1 << 3;

#[rustfmt::skip]
pub const TEST_KEYMAP: [[[KeyAction; 14]; 5]; 2] =
    [
        layer!([
            [k!(Grave), k!(Kc1), k!(Kc2), k!(Kc3), k!(Kc4), k!(Kc5), k!(Kc6), k!(Kc7), k!(Kc8), k!(Kc9), k!(Kc0), k!(Minus), k!(Equal), k!(Backspace)],
            [k!(Tab), k!(Q), k!(W), k!(E), k!(R), k!(T), k!(Y), k!(U), k!(I), k!(O), k!(P), k!(LeftBracket), k!(RightBracket), k!(Backslash)],
            [k!(Escape), th!(A, LShift), th!(S, LGui), k!(D), k!(F), k!(G), k!(H), k!(J), k!(K), k!(L), k!(Semicolon), k!(Quote), a!(No), k!(Enter)],
            [k!(LShift), th!(Z, LAlt), k!(X), k!(C), k!(V), k!(B), k!(N), k!(M), k!(Comma), k!(Dot), k!(Slash), a!(No), a!(No), k!(RShift)],
            [k!(LCtrl), k!(LGui), k!(LAlt), a!(No), a!(No), lt!(1, Space), a!(No), a!(No), a!(No), mo!(1), k!(RAlt), a!(No), k!(RGui), k!(RCtrl)]
        ]),
        layer!([
            [k!(Grave), k!(F1), k!(F2), k!(F3), k!(F4), k!(F5), k!(F6), k!(F7), k!(F8), k!(F9), k!(F10), k!(F11), k!(F12), k!(Delete)],
            [a!(No), a!(Transparent), k!(E), k!(W), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No)],
            [k!(CapsLock), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No)],
            [a!(No), a!(No), shifted!(X), wm!(X, ModifierCombination::new_from(false, false, false, true, false)), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), k!(Up)],
            [a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), k!(Left), a!(No), k!(Down), k!(Right)]
        ]),
    ];
