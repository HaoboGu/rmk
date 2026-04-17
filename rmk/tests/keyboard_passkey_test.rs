// This test requires the passkey_entry feature (which implies _ble).
// When compiled without it, the entire module is empty.
#![cfg(feature = "passkey_entry")]

pub mod common;

use embassy_futures::select::{Either, select};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Timer};
use futures::join;
use rmk::ble::passkey::{PASSKEY_RESPONSE, begin_passkey_entry_session, end_passkey_entry_session};
use rmk::channel::KEYBOARD_REPORT_CHANNEL;
use rmk::event::{AsyncEventPublisher, AsyncPublishableEvent, KeyboardEvent};
use rmk::input_device::Runnable;
use rmk::keyboard::Keyboard;

use crate::common::create_test_keyboard;

/// Helper: send key events while keyboard is running and verify the passkey result.
///
/// Verifies that:
///   - No keyboard reports are sent to the host while in passkey mode.
///   - The passkey response signal receives the expected value.
async fn run_passkey_test<'a>(
    keyboard: &mut Keyboard<'a>,
    key_sequence: &[common::TestKeyPress],
    expected_passkey: Option<u32>,
) {
    static TEST_DONE: Mutex<CriticalSectionRawMutex, bool> = Mutex::new(false);
    let sender = KeyboardEvent::publisher_async();
    sender.clear();
    KEYBOARD_REPORT_CHANNEL.clear();
    let max_timeout = Duration::from_secs(5);

    // Start passkey entry session before sending keys
    begin_passkey_entry_session();

    join!(
        // Run keyboard until the passkey test logic is done
        async {
            match select(
                Timer::after(max_timeout),
                select(keyboard.run(), async {
                    while !*TEST_DONE.lock().await {
                        Timer::after(Duration::from_millis(50)).await;
                    }
                }),
            )
            .await
            {
                Either::First(_) => panic!("ERROR: test timeout reached"),
                _ => (),
            }
        },
        // Send key events
        async {
            for key in key_sequence {
                Timer::after(Duration::from_millis(key.delay)).await;
                sender
                    .publish_async(KeyboardEvent::key(key.row, key.col, key.pressed))
                    .await;
            }

            // Small delay to let the keyboard process all events
            Timer::after(Duration::from_millis(100)).await;

            // Verify passkey response
            match select(Timer::after(Duration::from_secs(2)), PASSKEY_RESPONSE.wait()).await {
                Either::First(_) => panic!("ERROR: passkey response timeout"),
                Either::Second(got) => {
                    assert_eq!(
                        got, expected_passkey,
                        "Expected passkey {:?}, got {:?}",
                        expected_passkey, got
                    );
                }
            }

            // Verify no keyboard reports were sent during passkey mode
            // Try to receive — should timeout because no reports were sent
            match select(
                Timer::after(Duration::from_millis(200)),
                KEYBOARD_REPORT_CHANNEL.receive(),
            )
            .await
            {
                Either::First(_) => {
                    // Good — no reports sent
                }
                Either::Second(report) => {
                    panic!("Unexpected keyboard report sent during passkey mode: {:?}", report);
                }
            }

            end_passkey_entry_session();
            *TEST_DONE.lock().await = true;
        }
    );
}

/// Typing a full 6-digit passkey and pressing Enter submits the passkey
/// and no keyboard reports are sent to the host.
#[test]
fn test_passkey_entry_submits_passkey() {
    let mut keyboard = create_test_keyboard();

    // The test keymap has digit keys in row 0:
    //   col 1 = Kc1, col 2 = Kc2, col 3 = Kc3, col 4 = Kc4, col 5 = Kc5, col 6 = Kc6
    // Enter is at row 2, col 13
    // Passkey processes on key release, so we need press+release for each key
    common::test_block_on::test_block_on(run_passkey_test(
        &mut keyboard,
        &[
            // Type "123456"
            common::TestKeyPress {
                row: 0,
                col: 1,
                pressed: true,
                delay: 0,
            },
            common::TestKeyPress {
                row: 0,
                col: 1,
                pressed: false,
                delay: 50,
            },
            common::TestKeyPress {
                row: 0,
                col: 2,
                pressed: true,
                delay: 50,
            },
            common::TestKeyPress {
                row: 0,
                col: 2,
                pressed: false,
                delay: 50,
            },
            common::TestKeyPress {
                row: 0,
                col: 3,
                pressed: true,
                delay: 50,
            },
            common::TestKeyPress {
                row: 0,
                col: 3,
                pressed: false,
                delay: 50,
            },
            common::TestKeyPress {
                row: 0,
                col: 4,
                pressed: true,
                delay: 50,
            },
            common::TestKeyPress {
                row: 0,
                col: 4,
                pressed: false,
                delay: 50,
            },
            common::TestKeyPress {
                row: 0,
                col: 5,
                pressed: true,
                delay: 50,
            },
            common::TestKeyPress {
                row: 0,
                col: 5,
                pressed: false,
                delay: 50,
            },
            common::TestKeyPress {
                row: 0,
                col: 6,
                pressed: true,
                delay: 50,
            },
            common::TestKeyPress {
                row: 0,
                col: 6,
                pressed: false,
                delay: 50,
            },
            // Press Enter to submit
            common::TestKeyPress {
                row: 2,
                col: 13,
                pressed: true,
                delay: 50,
            },
            common::TestKeyPress {
                row: 2,
                col: 13,
                pressed: false,
                delay: 50,
            },
        ],
        Some(123456),
    ));
}

/// Pressing Escape during passkey entry cancels and signals None.
#[test]
fn test_passkey_entry_cancel() {
    let mut keyboard = create_test_keyboard();

    // Type a couple digits then cancel with Escape
    // Escape is at row 2, col 0
    common::test_block_on::test_block_on(run_passkey_test(
        &mut keyboard,
        &[
            common::TestKeyPress {
                row: 0,
                col: 1,
                pressed: true,
                delay: 0,
            },
            common::TestKeyPress {
                row: 0,
                col: 1,
                pressed: false,
                delay: 50,
            },
            common::TestKeyPress {
                row: 0,
                col: 2,
                pressed: true,
                delay: 50,
            },
            common::TestKeyPress {
                row: 0,
                col: 2,
                pressed: false,
                delay: 50,
            },
            // Press Escape to cancel
            common::TestKeyPress {
                row: 2,
                col: 0,
                pressed: true,
                delay: 50,
            },
            common::TestKeyPress {
                row: 2,
                col: 0,
                pressed: false,
                delay: 50,
            },
        ],
        None,
    ));
}
