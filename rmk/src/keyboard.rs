use crate::{
    action::{Action, KeyAction},
    keycode::{KeyCode, Modifier},
    keymap::KeyMap,
    matrix::{KeyState, Matrix},
    usb::KeyboardUsbDevice,
};
use core::convert::Infallible;
use embedded_hal::digital::v2::{InputPin, OutputPin};
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
        // Keys are processed in the following order:
        // process_key_change -> process_key_action_* -> process_action_*
        for row_idx in 0..ROW {
            for col_idx in 0..COL {
                let ks = self.matrix.get_key_state(row_idx, col_idx);
                if ks.changed {
                    self.process_key_change(row_idx, col_idx).await;
                    self.changed = true
                }
            }
        }

        Ok(())
    }

    /// Process key changes at (row, col)
    async fn process_key_change(&mut self, row: usize, col: usize) {
        // Matrix should process key pressed event first
        self.matrix.key_pressed(row, col);

        // Process key
        let key_state = self.matrix.get_key_state(row, col);
        let action = self.keymap.get_action(row, col, key_state);
        match action {
            KeyAction::No | KeyAction::Transparent => (),
            KeyAction::Single(a) => self.process_key_action_normal(a, key_state),
            KeyAction::WithModifier(a, m) => self.process_key_action_with_modifier(a, m, key_state),
            KeyAction::Tap(a) => self.process_key_action_tap(a, key_state).await,
            KeyAction::TapHold(tap_action, hold_action) => {
                self.process_key_action_tap_hold(tap_action, hold_action)
                    .await
            }
            KeyAction::OneShot(oneshot_action) => {
                self.process_key_action_oneshot(oneshot_action).await
            }
        }
    }

    fn process_key_action_normal(&mut self, action: Action, key_state: KeyState) {
        match action {
            Action::Key(key) => self.process_action_keycode(key, key_state),
            Action::LayerOn(layer_num) => self.process_action_layer_switch(layer_num, key_state),
            Action::LayerOff(layer_num) => {
                // We just turn off a layer when the key is pressed
                // TODO: Do we need this action?
                if key_state.changed && key_state.pressed {
                    self.keymap.deactivate_layer(layer_num);
                }
            }
            _ => (),
        }
    }

    fn process_key_action_with_modifier(
        &mut self,
        action: Action,
        modifier: Modifier,
        key_state: KeyState,
    ) {
        // Process modifier first
        // TODO: check the order when release a key
        self.process_action_keycode(modifier.as_keycode(), key_state);
        self.process_key_action_normal(action, key_state);
    }

    async fn process_key_action_tap(&mut self, action: Action, mut key_state: KeyState) {
        // TODO: when the tap is triggered, once the key is pressed or when it's released?
        if key_state.changed && key_state.pressed {
            key_state.pressed = true;
            self.process_key_action_normal(action, key_state);

            // TODO: need to trigger hid send manually, then, release the key to perform a tap operation
            // Wait 10ms, then send release
            Systick::delay(10.millis()).await;

            key_state.pressed = false;
            self.process_key_action_normal(action, key_state);
        }
    }

    async fn process_key_action_tap_hold(&mut self, _tap_action: Action, _hold_action: Action) {
        todo!("tap or hold not implemented");
    }

    async fn process_key_action_oneshot(&mut self, _oneshot_action: Action) {
        todo!("oneshot action not implemented");
    }

    // Process a single keycode.
    fn process_action_keycode(&mut self, key: KeyCode, key_state: KeyState) {
        if key.is_modifier() {
            let modifier_bit = key.as_modifier_bit();
            if key_state.pressed {
                self.register_modifier(modifier_bit);
            } else {
                self.unregister_modifier(modifier_bit);
            }
        } else if key.is_basic() {
            // 6KRO implementation
            if key_state.pressed {
                self.register_keycode(key);
            } else {
                self.unregister_keycode(key);
            }
        }
    }

    fn process_action_layer_switch(&mut self, layer_num: u8, key_state: KeyState) {
        // Change layer state only when the key's state is changed
        if !key_state.changed {
            return;
        }
        if key_state.pressed {
            self.keymap.activate_layer(layer_num);
        } else {
            self.keymap.deactivate_layer(layer_num);
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
