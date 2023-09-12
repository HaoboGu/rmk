use crate::{
    action::{Action, Modifier},
    keycode::KeyCode,
    layout::KeyMap,
    matrix::Matrix,
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
        keymap: [[[Action; COL]; ROW]; NUM_LAYER],
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
        let changed_matrix = self.matrix.debouncer.key_state;
        for (col_idx, col) in changed_matrix.iter().enumerate() {
            for (row_idx, state) in col.iter().enumerate() {
                if state.changed {
                    self.process_action(row_idx, col_idx, state.pressed).await;
                    self.changed = true
                }
            }
        }

        Ok(())
    }

    // Process key changes at (row, col)
    async fn process_action(&mut self, row: usize, col: usize, pressed: bool) {
        let action = self.keymap.get_action(row, col);
        match action {
            Action::No | Action::Transparent => (),
            Action::Key(k) => {
                self.process_key(k, pressed);
            }
            Action::KeyWithModifier(k, modifier) => {
                self.process_key_modifier(k, modifier, pressed).await
            }

            Action::Modifier(modifier) => self.process_modifier_tap(modifier, pressed).await,
            Action::OneShotModifier(_) => todo!(),
            Action::ModifiertOrTapToggle(_) => todo!(),
            Action::ModifierOrTapKey(_, _) => todo!(),
            Action::LayerActivate(_) => todo!(),
            Action::LayerDeactivate(_) => todo!(),
            Action::LayerToggle(_) => todo!(),
            Action::OneShotLayer(_) => todo!(),
            Action::LayerMods(_, _) => todo!(),
            Action::LayerOrTapKey(_, _) => todo!(),
            Action::LayerOrTapToggle(_) => todo!(),
            Action::MouseKey(_) => todo!(),
            Action::SystemControl(_) => todo!(),
            Action::ConsumerControl(_) => todo!(),
            Action::SwapHands(_) => todo!(),
        }
    }

    // Process a single key press.
    fn process_key(&mut self, key: KeyCode, pressed: bool) {
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

    async fn process_key_modifier(&mut self, key: KeyCode, modifier: Modifier, pressed: bool) {
        // KeyWithModifier is a tap event, only pressed change is considered
        // For KeyWithModifier, accept basic keycode only?
        if pressed && key.is_basic() {
            // Find avaial keycode position
            self.register_keycode(key);
            self.register_modifier(modifier.as_keycode().as_modifier_bit());

            // TODO: trigger send
            // Wait 10ms, then send release
            Systick::delay(10.millis()).await;

            // Send release event then
            self.unregister_keycode(key);
            self.unregister_modifier(modifier.as_keycode().as_modifier_bit());
        }
    }

    async fn process_modifier_tap(&mut self, modifier: Modifier, pressed: bool) {
        // Modifer tap event, consider pressed change only
        if pressed {
            self.register_modifier(modifier.as_keycode().as_modifier_bit());

            // TODO: trigger send
            // Wait 10ms, then send release
            Systick::delay(10.millis()).await;

            self.unregister_modifier(modifier.as_keycode().as_modifier_bit());
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
