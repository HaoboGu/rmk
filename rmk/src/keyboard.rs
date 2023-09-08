use crate::{
    action::Action, keycode::KeyCode, layout::KeyMap, matrix::Matrix, usb::KeyboardUsbDevice,
};
use core::convert::Infallible;
use embedded_hal::digital::v2::{InputPin, OutputPin};
use log::info;
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
    matrix: Matrix<In, Out, ROW, COL>,
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

    /// Main keyboard task, it scans matrix, process active keys
    /// If there is any change of keys, set self.changed=true
    pub async fn keyboard_task(&mut self) -> Result<(), Infallible> {
        self.matrix.scan().await?;
        let changed_matrix = self.matrix.changed;
        for (col_idx, col) in changed_matrix.iter().enumerate() {
            for (row_idx, changed) in col.iter().enumerate() {
                if *changed {
                    self.process_action(row_idx, col_idx, self.matrix.key_state[col_idx][row_idx]);
                    self.changed = true
                }
            }
        }

        Ok(())
    }

    fn process_action(&mut self, row: usize, col: usize, pressed: bool) {
        let action = self.keymap.get_action(row, col);
        match action {
            Action::No | Action::Transparent => (),
            Action::Key(k) => {
                self.process_key(k, pressed);
            }
            Action::KeyWithModifier(_, _) => info!("not implemented"),
            Action::Modifier(_) => todo!(),
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

    fn process_key(&mut self, key: KeyCode, pressed: bool) {
        if key.is_modifier() {
            let mut modifier_bit = key.as_modifier_bit();
            if pressed {
                self.report.modifier |= modifier_bit;
            } else {
                // Release modifier
                modifier_bit = !modifier_bit;
                self.report.modifier &= modifier_bit;
            }
        } else if key.is_basic() {
            // 6KRO implementation
            for bit in &mut self.report.keycodes {
                if pressed && (*bit == 0) {
                    *bit = key as u8;
                    break;
                } else if !pressed && (*bit == (key as u8)) {
                    *bit = 0;
                    break;
                }
            }
        }
    }
}
