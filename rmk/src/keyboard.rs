use crate::{
    action::{Action, KeyAction},
    eeprom::{eeconfig::Eeconfig, Eeprom, EepromStorageConfig},
    keycode::{KeyCode, ModifierCombination},
    keymap::KeyMap,
    matrix::{KeyState, Matrix},
    usb::KeyboardUsbDevice,
    via::{descriptor::ViaReport, process::process_via_packet},
};
use core::convert::Infallible;
use embedded_alloc::Heap;
use embedded_hal::digital::v2::{InputPin, OutputPin};
use embedded_storage::nor_flash::NorFlash;
use log::{debug, warn};
use rtic_monotonics::systick::*;
use usb_device::class_prelude::UsbBus;
use usbd_hid::descriptor::{KeyboardReport, MediaKeyboardReport, SystemControlReport};

#[global_allocator]
static HEAP: Heap = Heap::empty();

pub struct Keyboard<
    In: InputPin,
    Out: OutputPin,
    F: NorFlash,
    const EEPROM_SIZE: usize,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
> {
    /// Keyboard matrix, use COL2ROW by default
    #[cfg(feature = "col2row")]
    pub matrix: Matrix<In, Out, ROW, COL>,
    #[cfg(not(feature = "col2row"))]
    matrix: Matrix<In, Out, COL, ROW>,

    /// Keymap
    pub keymap: KeyMap<ROW, COL, NUM_LAYER>,

    /// Keyboard internal hid report buf
    report: KeyboardReport,

    /// Media internal report
    media_report: MediaKeyboardReport,

    eeprom: Option<Eeprom<F, EEPROM_SIZE>>,

    /// System control internal report
    system_control_report: SystemControlReport,

    /// Via report
    via_report: ViaReport,

    /// Should send a new report?
    need_send_key_report: bool,

    /// Should send a consumer control report?
    need_send_consumer_control_report: bool,

    /// Should send a system control report?
    need_send_system_control_report: bool,
}

impl<
        In: InputPin<Error = Infallible>,
        Out: OutputPin<Error = Infallible>,
        F: NorFlash,
        const EEPROM_SIZE: usize,
        const ROW: usize,
        const COL: usize,
        const NUM_LAYER: usize,
    > Keyboard<In, Out, F, EEPROM_SIZE, ROW, COL, NUM_LAYER>
{
    #[cfg(feature = "col2row")]
    pub fn new(
        input_pins: [In; ROW],
        output_pins: [Out; COL],
        storage: Option<F>,
        eeprom_storage_config: EepromStorageConfig,
        eeconfig: Option<Eeconfig>,
        mut keymap: [[[KeyAction; COL]; ROW]; NUM_LAYER],
    ) -> Self {
        // Initialize the allocator at the very beginning of the initialization of the keyboard
        {
            use core::mem::MaybeUninit;
            // 1KB heap size
            const HEAP_SIZE: usize = 1024;
            // Check page_size and heap size
            assert!((eeprom_storage_config.page_size as usize) < HEAP_SIZE);
            static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
            unsafe { HEAP.init(HEAP_MEM.as_ptr() as usize, HEAP_SIZE) }
        }

        let eeprom = match storage {
            Some(s) => {
                let e = Eeprom::new(s, eeprom_storage_config, eeconfig, &keymap);
                // If eeprom is initialized, read keymap from it.
                match e {
                    Some(e) => {
                        e.read_keymap(&mut keymap);
                        Some(e)
                    }
                    None => None,
                }
            }
            None => None,
        };

        Keyboard {
            matrix: Matrix::new(input_pins, output_pins),
            keymap: KeyMap::new(keymap),
            eeprom,
            report: KeyboardReport {
                modifier: 0,
                reserved: 0,
                leds: 0,
                keycodes: [0; 6],
            },
            media_report: MediaKeyboardReport { usage_id: 0 },
            system_control_report: SystemControlReport { usage_id: 0 },
            via_report: ViaReport {
                input_data: [0; 32],
                output_data: [0; 32],
            },
            need_send_key_report: false,
            need_send_consumer_control_report: false,
            need_send_system_control_report: false,
        }
    }

    #[cfg(not(feature = "col2row"))]
    pub fn new(
        input_pins: [In; COL],
        output_pins: [Out; ROW],
        keymap: [[[KeyAction; COL]; ROW]; NUM_LAYER],
    ) -> Self {
        let eeprom = match storage {
            Some(s) => {
                let e = Eeprom::new(s, eeprom_storage_config, &keymap);
                // If eeprom is initialized, read keymap from it.
                match e {
                    Some(e) => {
                        e.read_keymap(&mut keymap);
                        Some(e)
                    }
                    None => None,
                }
            }
            None => None,
        };

        Keyboard {
            matrix: Matrix::new(input_pins, output_pins),
            keymap: KeyMap::new(keymap),
            eeprom,
            report: KeyboardReport {
                modifier: 0,
                reserved: 0,
                leds: 0,
                keycodes: [0; 6],
            },
            media_report: MediaKeyboardReport { usage_id: 0 },
            system_control_report: SystemControlReport { usage_id: 0 },
            via_report: ViaReport {
                input_data: [0; 32],
                output_data: [0; 32],
            },
            need_send_key_report: false,
            need_send_consumer_control_report: false,
            need_send_system_control_report: false,
        }
    }

    /// Send hid report. The report is sent only when key state changes.
    pub fn send_report<B: UsbBus>(&mut self, usb_device: &KeyboardUsbDevice<'_, B>) {
        // TODO: refine changed, separate hid/media/system
        if self.need_send_key_report {
            usb_device.send_keyboard_report(&self.report);
            // Reset report key states
            for bit in &mut self.report.keycodes {
                *bit = 0;
            }
            self.need_send_key_report = false;
        }

        if self.need_send_consumer_control_report {
            debug!("Sending consumer report: {:?}", self.media_report);
            usb_device.send_consumer_control_report(&self.media_report);
            self.media_report.usage_id = 0;
            self.need_send_consumer_control_report = false;
        }
    }

    /// Read hid report.
    pub fn process_via_report<B: UsbBus>(&mut self, usb_device: &mut KeyboardUsbDevice<'_, B>) {
        if usb_device.read_via_report(&mut self.via_report) > 0 {
            process_via_packet(&mut self.via_report, &mut self.keymap, &mut self.eeprom);

            // Send via report back after processing
            usb_device.send_via_report(&self.via_report);
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
                }
            }
        }

        Ok(())
    }

    /// Process key changes at (row, col)
    async fn process_key_change(&mut self, row: usize, col: usize) {
        // Matrix should process key pressed event first, record the timestamp of key changes
        self.matrix.key_pressed(row, col);

        // Process key
        let key_state = self.matrix.get_key_state(row, col);
        let action = self.keymap.get_action_with_layer_cache(row, col, key_state);
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
            KeyAction::LayerTapHold(tap_action, layer_num) => {
                let layer_action = Action::LayerOn(layer_num);
                self.process_key_action_tap_hold(tap_action, layer_action)
                    .await;
            }
            KeyAction::ModifierTapHold(tap_action, modifier) => {
                let modifier_action = Action::Modifier(modifier);
                self.process_key_action_tap_hold(tap_action, modifier_action)
                    .await;
            }
        }
    }

    fn process_key_action_normal(&mut self, action: Action, key_state: KeyState) {
        match action {
            Action::Key(key) => self.process_action_keycode(key, key_state),
            Action::LayerOn(layer_num) => self.process_action_layer_switch(layer_num, key_state),
            Action::LayerOff(layer_num) => {
                // Turn off a layer temporarily when the key is pressed
                // Reactivate the layer after the key is released
                if key_state.changed && key_state.pressed {
                    self.keymap.deactivate_layer(layer_num);
                }
            }
            Action::LayerToggle(layer_num) => {
                // Toggle a layer when the key is release
                if key_state.changed && !key_state.pressed {
                    self.keymap.toggle_layer(layer_num);
                }
            }
            _ => (),
        }
    }

    fn process_key_action_with_modifier(
        &mut self,
        action: Action,
        modifier: ModifierCombination,
        key_state: KeyState,
    ) {
        // Process modifier first
        let (keycodes, n) = modifier.to_modifier_keycodes();
        for i in 0..n {
            self.process_action_keycode(keycodes[i], key_state);
        }
        self.process_key_action_normal(action, key_state);
    }

    /// Tap action, send a key when the key is pressed, then release the key.
    async fn process_key_action_tap(&mut self, action: Action, mut key_state: KeyState) {
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
        warn!("tap or hold not implemented");
    }

    async fn process_key_action_oneshot(&mut self, _oneshot_action: Action) {
        warn!("oneshot action not implemented");
    }

    // Process a single keycode, typically a basic key or a modifier key.
    fn process_action_keycode(&mut self, key: KeyCode, key_state: KeyState) {
        self.need_send_key_report = true;
        if key.is_consumer() {
            self.process_action_consumer_control(key, key_state);
        } else if key.is_system() {
            self.process_action_system_control(key, key_state);
        } else if key.is_modifier() {
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

    /// Process layer switch action.
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

    /// Process consumer control action. Consumer control keys are keys in hid consumer page, such as media keys.
    fn process_action_consumer_control(&mut self, key: KeyCode, key_state: KeyState) {
        if key.is_consumer() {
            if key_state.pressed {
                let media_key = key.as_consumer_control_usage_id();
                self.media_report.usage_id = media_key as u16;
                self.need_send_consumer_control_report = true;
            } else {
                self.media_report.usage_id = 0;
                self.need_send_consumer_control_report = true;
            }
        }
    }

    /// Process system control action. System control keys are keys in system page, such as power key.
    fn process_action_system_control(&mut self, key: KeyCode, key_state: KeyState) {
        if key.is_system() {
            if key_state.pressed {
                if let Some(system_key) = key.as_system_control_usage_id() {
                    self.system_control_report.usage_id = system_key as u8;
                    self.need_send_system_control_report = true;
                }
            } else {
                self.system_control_report.usage_id = 0;
                self.need_send_system_control_report = true;
            }
        }
    }

    /// Register a key to be sent in hid report.
    fn register_keycode(&mut self, key: KeyCode) {
        for bit in &mut self.report.keycodes {
            if *bit == 0 {
                *bit = key as u8;
                break;
            }
        }
    }

    /// Unregister a key from hid report.
    fn unregister_keycode(&mut self, key: KeyCode) {
        for bit in &mut self.report.keycodes {
            if *bit == (key as u8) {
                *bit = 0;
                break;
            }
        }
    }

    /// Register a modifier to be sent in hid report.
    fn register_modifier(&mut self, modifier_bit: u8) {
        self.report.modifier |= modifier_bit;
    }

    /// Unregister a modifier from hid report.
    fn unregister_modifier(&mut self, modifier_bit: u8) {
        self.report.modifier &= !modifier_bit;
    }
}
