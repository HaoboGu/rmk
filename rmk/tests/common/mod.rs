pub mod test_macro;

use core::cell::RefCell;

use embassy_futures::block_on;
use embassy_futures::select::{select, Either};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use futures::{join, FutureExt};
use heapless::Vec;
use log::debug;
use rmk::action::KeyAction;
use rmk::channel::{KEYBOARD_REPORT_CHANNEL, KEY_EVENT_CHANNEL};
use rmk::combo::Combo;
use rmk::config::{BehaviorConfig, CombosConfig};
use rmk::descriptor::KeyboardReport;
use rmk::event::KeyEvent;
use rmk::hid::Report;
use rmk::input_device::Runnable;
use rmk::keyboard::Keyboard;
use rmk::keymap::KeyMap;
use rmk::{a, k, layer, mo, th};

// Init logger for tests
#[ctor::ctor]
pub fn init_log() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .is_test(true)
        .try_init();
}

pub(crate) const KC_LSHIFT: u8 = 1 << 1;
pub(crate) const KC_LGUI: u8 = 1 << 3;

#[derive(Debug, Clone)]
pub struct TestKeyPress {
    pub row: u8,
    pub col: u8,
    pub pressed: bool,
    pub delay: u64, // Delay before this key event in milliseconds
}

// run a keyboard  test input is seq of key input with delay, use expected report to verify
pub async fn run_key_sequence_test<'a, const N: usize, const ROW: usize, const COL: usize, const NUM_LAYER: usize>(
    keyboard: &mut Keyboard<'a, ROW, COL, NUM_LAYER>,
    key_sequence: &[TestKeyPress],
    expected_reports: Vec<KeyboardReport, N>,
) {
    static REPORTS_DONE: Mutex<CriticalSectionRawMutex, bool> = Mutex::new(false);

    KEY_EVENT_CHANNEL.clear();
    KEYBOARD_REPORT_CHANNEL.clear();
    static MAX_TEST_TIMEOUT: Duration = Duration::from_secs(5);

    join!(
        // Run keyboard until all reports are received
        async {
            select(keyboard.run(), async {
                select(
                    Timer::after(MAX_TEST_TIMEOUT).then(|_| async {
                        panic!("Test timeout reached");
                    }),
                    async {
                        while !*REPORTS_DONE.lock().await {
                            // polling reports
                            Timer::after(Duration::from_millis(50)).await;
                        }
                    },
                )
                .await;
            })
            .await;
        },
        // Send all key events with delays
        async {
            for key in key_sequence {
                Timer::after(Duration::from_millis(key.delay)).await;
                KEY_EVENT_CHANNEL
                    .send(KeyEvent {
                        row: key.row,
                        col: key.col,
                        pressed: key.pressed,
                    })
                    .await;
            }
        },
        // Verify reports
        async {
            let mut report_index = -1;
            for expected in expected_reports {
                match select(Timer::after(Duration::from_secs(1)), KEYBOARD_REPORT_CHANNEL.receive()).await {
                    Either::First(_) => panic!("report wait timeout reached"),
                    Either::Second(Report::KeyboardReport(report)) => {
                        report_index += 1;
                        // println!("Received {}th report from channel: {:?}", report_index, report);
                        assert_eq!(
                            expected, report,
                            "on #{} reports, expected left but actually right",
                            report_index
                        );
                    }
                    Either::Second(report) => {
                        debug!("other reports {:?}", report)
                    }
                }
            }
            // Set done flag after all reports are verified
            *REPORTS_DONE.lock().await = true;
        }
    );
}

#[rustfmt::skip]
pub const fn get_keymap() -> [[[KeyAction; 14]; 5]; 2] {
    [
        layer!([
            [k!(Grave), k!(Kc1), k!(Kc2), k!(Kc3), k!(Kc4), k!(Kc5), k!(Kc6), k!(Kc7), k!(Kc8), k!(Kc9), k!(Kc0), k!(Minus), k!(Equal), k!(Backspace)],
            [k!(Tab), k!(Q), k!(W), k!(E), k!(R), k!(T), k!(Y), k!(U), k!(I), k!(O), k!(P), k!(LeftBracket), k!(RightBracket), k!(Backslash)],
            [k!(Escape), th!(A, LShift), th!(S, LGui), k!(D), k!(F), k!(G), k!(H), k!(J), k!(K), k!(L), k!(Semicolon), k!(Quote), a!(No), k!(Enter)],
            [k!(LShift), k!(Z), k!(X), k!(C), k!(V), k!(B), k!(N), k!(M), k!(Comma), k!(Dot), k!(Slash), a!(No), a!(No), k!(RShift)],
            [k!(LCtrl), k!(LGui), k!(LAlt), a!(No), a!(No), k!(Space), a!(No), a!(No), a!(No), mo!(1), k!(RAlt), a!(No), k!(RGui), k!(RCtrl)]
        ]),
        layer!([
            [k!(Grave), k!(F1), k!(F2), k!(F3), k!(F4), k!(F5), k!(F6), k!(F7), k!(F8), k!(F9), k!(F10), k!(F11), k!(F12), k!(Delete)],
            [a!(No), a!(Transparent), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No)],
            [k!(CapsLock), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No)],
            [a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), k!(Up)],
            [a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), k!(Left), a!(No), k!(Down), k!(Right)]
        ]),
    ]
}

#[rustfmt::skip]
pub fn get_combos_config() -> CombosConfig {
    // Define the function to return the appropriate combo configuration
    CombosConfig {
        combos: Vec::from_iter([
            Combo::new(
                [
                    k!(V), //3,4
                    k!(B), //3,5
                ]
                .to_vec(),
                k!(LShift),
                Some(0),
            ),
            Combo::new(
                [
                    k!(R), //1,4
                    k!(T), //1,5
                ]
                .to_vec(),
                k!(LAlt),
                Some(0),
            ),
        ]),
        timeout: Duration::from_millis(100),
    }
}

pub fn create_test_keyboard_with_config(config: BehaviorConfig) -> Keyboard<'static, 5, 14, 2> {
    Keyboard::new(wrap_keymap(get_keymap(), config))
}

pub fn wrap_keymap<'a, const R: usize, const C: usize, const L: usize>(
    keymap: [[[KeyAction; C]; R]; L],
    config: BehaviorConfig,
) -> &'a mut RefCell<KeyMap<'static, R, C, L>> {
    // Box::leak is acceptable in tests
    let leaked_keymap = Box::leak(Box::new(keymap));

    let keymap = block_on(KeyMap::new(leaked_keymap, None, config));
    let keymap_cell = RefCell::new(keymap);
    Box::leak(Box::new(keymap_cell))
}

pub fn create_test_keyboard() -> Keyboard<'static, 5, 14, 2> {
    create_test_keyboard_with_config(BehaviorConfig::default())
}

// pub fn key_event(row: u8, col: u8, pressed: bool) -> KeyEvent {
//     KeyEvent { row, col, pressed }
// }
