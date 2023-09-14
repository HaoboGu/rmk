use crate::{
    action::{Action, KeyAction},
    keycode::{KeyCode, Modifier},
    keymap::KeyMap,
    matrix::Matrix,
    usb::KeyboardUsbDevice,
};
use core::convert::Infallible;
use embedded_hal::digital::v2::{InputPin, OutputPin};
use log::info;
use rtic_monotonics::systick::*;
use usb_device::class_prelude::UsbBus;
use usbd_hid::descriptor::KeyboardReport;

pub struct Keyboard<
    In: InputPin,
    Out: OutputPin,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
> {
    /// Keyboard matrix, use COL2ROW by default
    #[cfg(not(feature = "ROW2COL"))]
    pub matrix: Matrix<In, Out, ROW, COL>,
    #[cfg(feature = "ROW2COL")]
    matrix: Matrix<In, Out, COL, ROW>,

    /// Keymap
    keymap: KeyMap<ROW, COL, NUM_LAYER>,

    /// Keyboard internal hid report buf
    report: KeyboardReport,

    /// Should send a new report?
    changed: bool,
}

impl<
        In: InputPin<Error = Infallible>,
        Out: OutputPin<Error = Infallible>,
        const ROW: usize,
        const COL: usize,
        const NUM_LAYER: usize,
    > Keyboard<In, Out, ROW, COL, NUM_LAYER>
{
    pub fn new(
        input_pins: [In; ROW],
        output_pins: [Out; COL],
        keymap: [[[KeyAction; COL]; ROW]; NUM_LAYER],
    ) -> Self {
        Keyboard {
            matrix: Matrix::new(input_pins, output_pins),
            keymap: KeyMap::new(keymap),
            report: KeyboardReport {
                modifier: 0,
                reserved: 0,
                leds: 0,
                keycodes: [0; 6],
            },
            changed: false,
        }
    }

    /// Send hid report. The report is sent only when key state changes.
    pub fn send_report<B: UsbBus>(&mut self, usb_device: &KeyboardUsbDevice<'_, B>) {
        if self.changed {
            usb_device.send_keyboard_report(&self.report);

            // Reset report key states
            for bit in &mut self.report.keycodes {
                *bit = 0;
            }
            self.changed = false;
        }
    }

    /// Main keyboard task, it scans matrix, processes active keys
    /// If there is any change of key states, set self.changed=true
    pub async fn keyboard_task(&mut self) -> Result<(), Infallible> {
        // Matrix scan
        self.matrix.scan().await?;

        // Check matrix states, process key if there is a key state change
        for row_idx in 0..ROW {
            for col_idx in 0..COL {
                if self.matrix.key_states[col_idx][row_idx].changed {
                    self.process_action(
                        row_idx,
                        col_idx,
                        self.matrix.key_states[col_idx][row_idx].pressed,
                    )
                    .await;
                    self.changed = true
                }
            }
        }

        Ok(())
    }

    // Process key changes at (row, col)
    async fn process_action(&mut self, row: usize, col: usize, pressed: bool) {
        if pressed {
            self.matrix.key_states[col][row].start_timer();
        } else {
            info!("{:?}", self.matrix.key_states[col][row].elapsed());
        }
        let action = self.keymap.get_action(row, col);
        match action {
            KeyAction::No | KeyAction::Transparent => (),
            KeyAction::Single(a) => self.process_normal_action(a, pressed),
            KeyAction::WithModifier(a, m) => self.process_action_with_modifier(a, m, pressed),
            KeyAction::Tap(a) => self.process_tap(a).await,
            KeyAction::TapHold(tap_action, hold_action) => {
                self.process_tap_or_hold(tap_action, hold_action).await
            }
            KeyAction::OneShot(_) => todo!(),
        }
    }

    fn process_normal_action(&mut self, action: Action, pressed: bool) {
        match action {
            Action::Key(key) => self.process_keycode(key, pressed),
            _ => (),
        }
    }

    fn process_action_with_modifier(&mut self, action: Action, modifier: Modifier, pressed: bool) {
        self.process_keycode(modifier.as_keycode(), pressed);
        if pressed {
            self.process_normal_action(action, pressed);
        }
    }

    async fn process_tap(&mut self, action: Action) {
        self.process_normal_action(action, true);

        // TODO: need to trigger hid send manually, then, release the key to perform a tap operation
        // Wait 10ms, then send release
        Systick::delay(10.millis()).await;

        self.process_normal_action(action, false);
    }

    async fn process_tap_or_hold(&mut self, _tap_action: Action, _hold_action: Action) {
        todo!("tap or hold not implemented");
    }

    // Process a single normal key press.
    fn process_keycode(&mut self, key: KeyCode, pressed: bool) {
        if key.is_modifier() {
            let modifier_bit = key.as_modifier_bit();
            if pressed {
                self.register_modifier(modifier_bit);
            } else {
                self.unregister_modifier(modifier_bit);
            }
        } else if key.is_basic() {
            // 6KRO implementation
            if pressed {
                self.register_keycode(key);
            } else {
                self.unregister_keycode(key);
            }
        }
    }

    fn register_keycode(&mut self, key: KeyCode) {
        for bit in &mut self.report.keycodes {
            if *bit == 0 {
                *bit = key as u8;
                break;
            }
        }
    }

    fn unregister_keycode(&mut self, key: KeyCode) {
        for bit in &mut self.report.keycodes {
            if *bit == (key as u8) {
                *bit = 0;
                break;
            }
        }
    }

    fn register_modifier(&mut self, modifier_bit: u8) {
        self.report.modifier |= modifier_bit;
    }

    fn unregister_modifier(&mut self, modifier_bit: u8) {
        self.report.modifier &= !modifier_bit;
    }
}
