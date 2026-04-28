pub mod morse;
pub mod test_block_on;
pub mod test_macro;

use embassy_futures::select::{Either, select};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use futures::join;
use log::debug;
use rmk::channel::USB_REPORT_CHANNEL;
use rmk::config::{BehaviorConfig, PositionalConfig};
use rmk::core_traits::Runnable;
use rmk::event::{AsyncEventPublisher, AsyncPublishableEvent, KeyboardEvent};
use rmk::hid::{KeyboardReport, Report};
use rmk::keyboard::Keyboard;
use rmk::keymap::KeyMap;
use rmk::state::{UsbState, set_usb_state};
use rmk::types::action::KeyAction;
use rmk::types::modifier::ModifierCombination;
use rmk::{KeymapData, a, k, layer, lt, mo, shifted, th, wm};

// `embassy-time`'s MockDriver is a process-global singleton, so running the
// suite under plain `cargo test` lets tests race on it and hang at the 60 s
// virtual-time kill switch in `test_block_on`. Abort at test-binary startup
// with a pointer to the right runner instead of making the user wait for that
// timeout.
#[ctor::ctor]
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
#[ctor::ctor]
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

#[derive(Debug, Clone)]
pub struct TestKeyPress {
    pub row: u8,
    pub col: u8,
    pub pressed: bool,
    pub delay: u64, // Delay before this key event in milliseconds
}

// run a keyboard, test input is seq of key input with delay, use expected report to verify
pub async fn run_key_sequence_test<'a>(
    keyboard: &mut Keyboard<'a>,
    key_sequence: &[TestKeyPress],
    expected_reports: &[KeyboardReport],
) {
    static REPORTS_DONE: Mutex<CriticalSectionRawMutex, bool> = Mutex::new(false);
    static SEQ_SEND_DONE: Mutex<CriticalSectionRawMutex, bool> = Mutex::new(false);

    let sender = KeyboardEvent::publisher_async();
    sender.clear();
    USB_REPORT_CHANNEL.clear();
    // Default `preferred = Usb` + Configured usb makes the cascade pick Usb,
    // routing reports to `USB_REPORT_CHANNEL` for assertions below.
    set_usb_state(UsbState::Configured);
    static MAX_TEST_TIMEOUT: Duration = Duration::from_secs(5);

    join!(
        // Run keyboard until all reports are received
        async {
            match select(
                Timer::after(MAX_TEST_TIMEOUT),
                select(keyboard.run(), async {
                    while !*REPORTS_DONE.lock().await {
                        // polling reports
                        Timer::after(Duration::from_millis(50)).await;
                    }
                }),
            )
            .await
            {
                Either::First(_) => panic!("ERROR: report done timeout reached"),
                _ => (),
            }
        },
        // Send all key events with delays
        async {
            for key in key_sequence {
                Timer::after(Duration::from_millis(key.delay)).await;
                sender
                    .publish_async(KeyboardEvent::key(key.row, key.col, key.pressed))
                    .await;
            }

            // Set done flag after all key events are sent
            *SEQ_SEND_DONE.lock().await = true;
        },
        // Verify reports
        async {
            match select(Timer::after(MAX_TEST_TIMEOUT), async {
                let mut report_index = -1;
                for expected in expected_reports {
                    match select(Timer::after(Duration::from_secs(2)), USB_REPORT_CHANNEL.receive()).await {
                        Either::First(_) => panic!("ERROR: report wait timeout reached"),
                        Either::Second(Report::KeyboardReport(report)) => {
                            report_index += 1;
                            // println!("Received {}th report from channel: {:?}", report_index, report);
                            assert_eq!(
                                *expected, report,
                                "on #{} reports, expected left but actually right",
                                report_index
                            );
                        }
                        Either::Second(report) => {
                            debug!("Other reports {:?}", report)
                        }
                    }
                }

                // Wait for all key events to be sent
                while !*SEQ_SEND_DONE.lock().await {
                    Timer::after(Duration::from_millis(50)).await;
                }

                // Set done flag after all reports are verified
                *REPORTS_DONE.lock().await = true;
            })
            .await
            {
                Either::First(_) => panic!("Read report timeout"),
                Either::Second(_) => (),
            }
        }
    );
    if !keyboard.held_buffer.is_empty() {
        panic!("leak after buffer cleanup, buffer contains {:?}", keyboard.held_buffer);
    }
}

#[rustfmt::skip]
pub const fn get_keymap() -> [[[KeyAction; 14]; 5]; 2] {
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
    ]
}

pub fn create_test_keyboard_with_config(config: BehaviorConfig) -> Keyboard<'static> {
    let behavior_config: &'static mut BehaviorConfig = Box::leak(Box::new(config));
    let per_key_config: &'static PositionalConfig<5, 14> = Box::leak(Box::new(PositionalConfig::default()));
    Keyboard::new(wrap_keymap(get_keymap(), per_key_config, behavior_config))
}

pub fn wrap_keymap<'a, const R: usize, const C: usize, const L: usize>(
    keymap: [[[KeyAction; C]; R]; L],
    per_key_config: &'static PositionalConfig<R, C>,
    config: &'static mut BehaviorConfig,
) -> &'a KeyMap<'static> {
    // Box::leak is acceptable in tests
    let data = Box::leak(Box::new(KeymapData::new(keymap)));
    let keymap = test_block_on::test_block_on(KeyMap::new(data, config, per_key_config));
    Box::leak(Box::new(keymap))
}

pub fn create_test_keyboard() -> Keyboard<'static> {
    create_test_keyboard_with_config(BehaviorConfig::default())
}
