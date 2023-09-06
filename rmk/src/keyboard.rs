use core::convert::Infallible;
use embedded_hal::digital::v2::{InputPin, OutputPin};
use log::info;
use usb_device::{class_prelude::UsbBus, UsbError};
use usbd_hid::hid_class::HIDClass;

use crate::{
    action::Action,
    keycode::{BaseKeyCode, KeyCode},
    layout::KeyMap,
    matrix::Matrix,
};
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

    /// Keyboard hid report
    report: [u8; 8],

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
            report: [0; 8],
            changed: false,
        }
    }

    pub fn send_report<B: UsbBus>(&mut self, hid: &HIDClass<B>) {
        if self.changed {
            match hid.push_raw_input(&self.report) {
                Ok(_) => (),
                Err(UsbError::WouldBlock) => (),
                Err(_) => panic!("push raw input error"),
            }
            // Reset report state
            self.report = [0; 8];
            self.changed = false;
        }

    }

    /// Main keyboard task, it scans matrix, process active keys
    /// If there is any change of keys, set self.changed=true
    /// TODO: Does it more elegant: Use channels to pass changes to hid?
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
        let k = key.to_base_keycode();
        if k.is_modifier() {
            let mut modifier_bit = k.as_modifier_bit();
            if pressed {
                self.report[0] |= modifier_bit;
            } else {
                // Release modifier
                modifier_bit = !modifier_bit;
                self.report[0] &= modifier_bit;
            }
        } else if k != BaseKeyCode::No {
            for i in 2..8 {
                // 6KRO implementation
                if pressed && self.report[i] == 0 {
                    self.report[i] = k as u8;
                    break;
                } else if !pressed && self.report[i] == k as u8 {
                    self.report[i] = 0;
                    break;
                }
            }
        }
    }
}
