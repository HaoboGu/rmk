extern crate rmk;

/// Macro for testing key sequence.
///
/// # Arguments:
///     - keyboard: keyboard initialization
///     - sequence: key sequence: [row, col, pressed, press_delay], where press_delay is the time interval in ms between last key action and current key
///     - expected_reports: [modifiers, [keycodes; 6]], represents the hid report which will be sent to the host
#[macro_export]
macro_rules! key_sequence_test {
    (keyboard: $keyboard:expr, sequence: [$([$row:expr, $col:expr, $pressed:expr, $delay:expr]),* $(,)?], expected_reports: [$([$modifier:expr, $keys:expr]),* $(,)?]) => {
        block_on(async {
            let mut keyboard = $keyboard;
            let sequence = vec![
                $(
                    $crate::common::TestKeyPress {
                        row: $row,
                        col: $col,
                        pressed: $pressed,
                        delay: $delay,
                    },
                )*
            ];
            let expected_reports = vec![
                $(
                    rmk::descriptor::KeyboardReport {
                        modifier: $modifier,
                        keycodes: $keys,
                        leds: 0,
                        reserved: 0,
                    },
                )*
            ];

            $crate::common::run_key_sequence_test(&mut keyboard, &sequence, &expected_reports).await;
        });
    };
}

/// Convert a key `k!(key)` to the u8 representation in hid report.
/// For example, `KeyCode::A` will be converted to `0x04`.
#[macro_export]
macro_rules! kc_to_u8 {
    ($key: ident) => {
        rmk::keycode::KeyCode::$key as u8
    };
}
