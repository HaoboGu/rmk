extern crate rmk;

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

            run_key_sequence_test(&mut keyboard, &sequence, &expected_reports).await;
        });
    };
}

// a rust macro to map a str to k!(a) as u8
#[macro_export]
macro_rules! kc8 {
    ($key: ident) => {
        rmk::keycode::KeyCode::$key as u8
    };
}

// a rust macro to create a key sequence to simulate key presses
#[macro_export]
macro_rules! key_sequence {
    ($([$row:expr, $col:expr, $pressed:expr, $delay:expr]),* $(,)?) => {
        vec![
            $(
                $crate::common::TestKeyPress {
                    row: $row,
                    col: $col,
                    pressed: $pressed,
                    delay: $delay,
                },
            )*
        ]
    };
}

// a rust macro to create a key report that simulates key status change in hid
#[macro_export]
macro_rules! key_report {
    ($([$modifier:expr, $keys:expr]),* $(,)?) => {
        vec![
            $(
                rmk::descriptor::KeyboardReport {
                    modifier: $modifier,
                    keycodes: $keys,
                    leds: 0,
                    reserved: 0,
                },
            )*
        ]
    };
}
