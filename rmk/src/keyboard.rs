#[cfg(not(feature = "rapid_debouncer"))]
use crate::debounce::default_bouncer::DefaultDebouncer;
#[cfg(feature = "rapid_debouncer")]
use crate::debounce::fast_debouncer::RapidDebouncer;
use crate::{
    action::{Action, KeyAction},
    hid::{ConnectionType, HidWriterWrapper},
    keycode::{KeyCode, ModifierCombination},
    keymap::KeyMap,
    matrix::{KeyState, Matrix},
    usb::descriptor::{CompositeReport, CompositeReportType, ViaReport},
};
use core::cell::RefCell;
use defmt::{debug, error, warn};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Receiver, Sender},
};
use embassy_time::{Instant, Timer};
use embedded_hal::digital::{InputPin, OutputPin};
use usbd_hid::descriptor::KeyboardReport;

/// Matrix scanning task sends this [KeyboardReportMessage] to communication task.
pub(crate) enum KeyboardReportMessage {
    /// Normal keyboard hid report
    KeyboardReport(KeyboardReport),
    /// Other types of keyboard reports: mouse + media(consumer) + system control
    CompositeReport(CompositeReport, CompositeReportType),
}

/// This is the main keyboard task, this task do the matrix scanning and key processing
/// The report is sent to communication task, and finally sent to the host
pub(crate) async fn keyboard_task<
    'a,
    In: InputPin,
    Out: OutputPin,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    keyboard: &mut Keyboard<'a, In, Out, ROW, COL, NUM_LAYER>,
    sender: &mut Sender<'a, CriticalSectionRawMutex, KeyboardReportMessage, 8>,
) {
    loop {
        let _ = keyboard.scan_matrix(sender).await;
        keyboard.send_keyboard_report(sender).await;
        keyboard.send_media_report(sender).await;
        keyboard.send_mouse_report(sender).await;
        keyboard.send_system_control_report(sender).await;
    }
}

/// This task processes all keyboard reports and send them to the host
pub(crate) async fn communication_task<'a, W: HidWriterWrapper, W2: HidWriterWrapper>(
    receiver: &mut Receiver<'a, CriticalSectionRawMutex, KeyboardReportMessage, 8>,
    keybooard_hid_writer: &mut W,
    other_hid_writer: &mut W2,
) {
    loop {
        match receiver.receive().await {
            KeyboardReportMessage::KeyboardReport(report) => {
                match keybooard_hid_writer.write_serialize(&report).await {
                    Ok(()) => {}
                    Err(e) => error!("Send keyboard report error: {}", e),
                };
            }
            KeyboardReportMessage::CompositeReport(report, report_type) => {
                write_other_report_to_host(report, report_type, other_hid_writer).await;
            }
        }
    }
}

pub(crate) async fn write_other_report_to_host<W: HidWriterWrapper>(
    report: CompositeReport,
    report_type: CompositeReportType,
    other_hid_writer: &mut W,
) {
    let mut buf: [u8; 9] = [0; 9];
    // Prepend report id
    buf[0] = report_type as u8;
    match report.serialize(&mut buf[1..], report_type) {
        Ok(s) => {
            debug!("Sending other report: {=[u8]:#X}", buf[0..s + 1]);
            match match other_hid_writer.get_conn_type() {
                ConnectionType::Usb => other_hid_writer.write(&buf[0..s + 1]).await,
                ConnectionType::Ble => other_hid_writer.write(&buf[1..s + 1]).await,
            } {
                Ok(_) => {}
                Err(e) => error!("Send other report error: {}", e),
            }
        }
        Err(_) => error!("Serialize other report error"),
    }
}
pub(crate) struct Keyboard<
    'a,
    In: InputPin,
    Out: OutputPin,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
> {
    /// Keyboard matrix, use COL2ROW by default
    #[cfg(all(feature = "col2row", feature = "rapid_debouncer"))]
    pub(crate) matrix: Matrix<In, Out, RapidDebouncer<ROW, COL>, ROW, COL>,
    #[cfg(all(feature = "col2row", not(feature = "rapid_debouncer")))]
    pub(crate) matrix: Matrix<In, Out, DefaultDebouncer<ROW, COL>, ROW, COL>,
    #[cfg(all(not(feature = "col2row"), feature = "rapid_debouncer"))]
    pub(crate) matrix: Matrix<In, Out, RapidDebouncer<COL, ROW>, COL, ROW>,
    #[cfg(all(not(feature = "col2row"), not(feature = "rapid_debouncer")))]
    pub(crate) matrix: Matrix<In, Out, DefaultDebouncer<COL, ROW>, COL, ROW>,

    /// Keymap
    pub(crate) keymap: &'a RefCell<KeyMap<ROW, COL, NUM_LAYER>>,

    /// Keyboard internal hid report buf
    report: KeyboardReport,

    /// Internal composite report: mouse + media(consumer) + system control
    other_report: CompositeReport,

    /// Via report
    via_report: ViaReport,

    /// Should send a new keyboard report?
    need_send_key_report: bool,

    /// Should send a consumer control report?
    need_send_consumer_control_report: bool,

    /// Should send a system control report?
    need_send_system_control_report: bool,

    /// Should send a mouse report?
    need_send_mouse_report: bool,

    /// Mouse key is different from other keyboard keys, it should be sent continuously while the key is pressed.
    /// The last tick of mouse is recorded to control the reporting rate.
    last_mouse_tick: u64,

    /// The current distance of mouse key moving
    mouse_key_move_delta: i8,
    mouse_wheel_move_delta: i8,
}

impl<
        'a,
        In: InputPin,
        Out: OutputPin,
        const ROW: usize,
        const COL: usize,
        const NUM_LAYER: usize,
    > Keyboard<'a, In, Out, ROW, COL, NUM_LAYER>
{
    pub(crate) fn new(
        #[cfg(feature = "col2row")] input_pins: [In; ROW],
        #[cfg(not(feature = "col2row"))] input_pins: [In; COL],
        #[cfg(feature = "col2row")] output_pins: [Out; COL],
        #[cfg(not(feature = "col2row"))] output_pins: [Out; ROW],
        keymap: &'a RefCell<KeyMap<ROW, COL, NUM_LAYER>>,
    ) -> Self {
        Keyboard {
            matrix: Matrix::new(input_pins, output_pins),
            keymap,
            report: KeyboardReport {
                modifier: 0,
                reserved: 0,
                leds: 0,
                keycodes: [0; 6],
            },
            other_report: CompositeReport::default(),
            via_report: ViaReport {
                input_data: [0; 32],
                output_data: [0; 32],
            },
            need_send_key_report: false,
            need_send_consumer_control_report: false,
            need_send_system_control_report: false,
            need_send_mouse_report: false,
            last_mouse_tick: 0,
            mouse_key_move_delta: 8,
            mouse_wheel_move_delta: 1,
        }
    }

    pub(crate) async fn send_keyboard_report(
        &mut self,
        sender: &mut Sender<'a, CriticalSectionRawMutex, KeyboardReportMessage, 8>,
    ) {
        if self.need_send_key_report {
            debug!("Sending keyboard report: {=[u8]:#X}", self.report.keycodes);
            sender
                .send(KeyboardReportMessage::KeyboardReport(self.report))
                .await;
            self.need_send_key_report = false;
        }
    }

    /// Send system control report if needed
    pub(crate) async fn send_system_control_report(
        &mut self,
        sender: &mut Sender<'a, CriticalSectionRawMutex, KeyboardReportMessage, 8>,
    ) {
        if self.need_send_system_control_report {
            sender
                .send(KeyboardReportMessage::CompositeReport(
                    self.other_report,
                    CompositeReportType::System,
                ))
                .await;
            self.other_report.system_usage_id = 0;
            self.need_send_system_control_report = false;
        }
    }

    /// Send media report if needed
    pub(crate) async fn send_media_report(
        &mut self,
        sender: &mut Sender<'a, CriticalSectionRawMutex, KeyboardReportMessage, 8>,
    ) {
        if self.need_send_consumer_control_report {
            sender
                .send(KeyboardReportMessage::CompositeReport(
                    self.other_report,
                    CompositeReportType::Media,
                ))
                .await;
            self.other_report.media_usage_id = 0;
            self.need_send_consumer_control_report = false;
        }
    }

    /// Send mouse report if needed
    pub(crate) async fn send_mouse_report(
        &mut self,
        sender: &mut Sender<'a, CriticalSectionRawMutex, KeyboardReportMessage, 8>,
    ) {
        if self.need_send_mouse_report {
            // Prevent mouse report flooding, set maximum mouse report rate to 100 HZ
            let cur_tick = Instant::now().as_millis();
            // The default internal of sending mouse report is 20 ms, same as qmk accelerated mode
            // TODO: make it configurable
            if cur_tick - self.last_mouse_tick > 20 {
                sender
                    .send(KeyboardReportMessage::CompositeReport(
                        self.other_report,
                        CompositeReportType::Mouse,
                    ))
                    .await;
                self.last_mouse_tick = cur_tick;
            }
            // Do nothing
            if self.other_report.x == 0
                && self.other_report.y == 0
                && self.other_report.wheel == 0
                && self.other_report.pan == 0
            {
                // Release, stop report mouse report
                self.need_send_mouse_report = false;
            }
        }
    }

    /// Main keyboard task, it scans matrix, processes active keys.
    /// If there is any change of key states, set self.changed=true.
    ///
    /// `sender` is required because when there's tap action, the report should be sent immediately and then continue scanning
    pub(crate) async fn scan_matrix(
        &mut self,
        sender: &mut Sender<'a, CriticalSectionRawMutex, KeyboardReportMessage, 8>,
    ) {
        // Matrix scan
        self.matrix.scan().await;

        // Check matrix states, process key if there is a key state change
        // Keys are processed in the following order:
        // process_key_change -> process_key_action_* -> process_action_*
        for row_idx in 0..ROW {
            for col_idx in 0..COL {
                let ks = self.matrix.get_key_state(row_idx, col_idx);
                if ks.changed {
                    self.process_key_change(row_idx, col_idx, sender).await;
                } else if ks.pressed {
                    // When there's no key change, only tap/hold action needs to be processed
                    // Continuously check the hold state
                    let action = self
                        .keymap
                        .borrow_mut()
                        .get_action_with_layer_cache(row_idx, col_idx, ks);
                    debug!("Pressed and no change, action: {}", action);
                    let tap_hold = match action {
                        KeyAction::TapHold(tap_action, hold_action) => {
                            Some((tap_action, hold_action))
                        }
                        KeyAction::LayerTapHold(tap_action, layer_num) => {
                            Some((tap_action, Action::LayerOn(layer_num)))
                        }
                        KeyAction::ModifierTapHold(tap_action, modifier) => {
                            Some((tap_action, Action::Modifier(modifier)))
                        }
                        _ => None,
                    };
                    if let Some((tap_action, hold_action)) = tap_hold {
                        self.process_key_action_tap_hold(
                            tap_action,
                            hold_action,
                            row_idx,
                            col_idx,
                            ks,
                            sender,
                        )
                        .await;
                    };
                }
            }
        }
    }

    /// Process key changes at (row, col)
    async fn process_key_change(
        &mut self,
        row: usize,
        col: usize,
        sender: &mut Sender<'a, CriticalSectionRawMutex, KeyboardReportMessage, 8>,
    ) {
        let key_state = self.matrix.get_key_state(row, col);

        // Matrix should process key pressed event first, record the timestamp of key changes
        if key_state.pressed {
            // Start timer
            self.matrix.update_timer(row, col);
        }

        // Process key
        let action = self
            .keymap
            .borrow_mut()
            .get_action_with_layer_cache(row, col, key_state);
        match action {
            KeyAction::No | KeyAction::Transparent => (),
            KeyAction::Single(a) => self.process_key_action_normal(a, key_state),
            KeyAction::WithModifier(a, m) => self.process_key_action_with_modifier(a, m, key_state),
            KeyAction::Tap(a) => self.process_key_action_tap(a, key_state, sender).await,
            KeyAction::TapHold(tap_action, hold_action) => {
                self.process_key_action_tap_hold(
                    tap_action,
                    hold_action,
                    row,
                    col,
                    key_state,
                    sender,
                )
                .await;
            }
            KeyAction::OneShot(oneshot_action) => {
                self.process_key_action_oneshot(oneshot_action).await
            }
            KeyAction::LayerTapHold(tap_action, layer_num) => {
                let layer_action = Action::LayerOn(layer_num);
                self.process_key_action_tap_hold(
                    tap_action,
                    layer_action,
                    row,
                    col,
                    key_state,
                    sender,
                )
                .await;
            }
            KeyAction::ModifierTapHold(tap_action, modifier) => {
                let modifier_action = Action::Modifier(modifier);
                self.process_key_action_tap_hold(
                    tap_action,
                    modifier_action,
                    row,
                    col,
                    key_state,
                    sender,
                )
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
                    self.keymap.borrow_mut().deactivate_layer(layer_num);
                }
            }
            Action::LayerToggle(layer_num) => {
                // Toggle a layer when the key is release
                if key_state.changed && !key_state.pressed {
                    self.keymap.borrow_mut().toggle_layer(layer_num);
                }
            }
            Action::Modifier(modifier) => {
                let (keycodes, n) = modifier.to_modifier_keycodes();
                for kc in keycodes.iter().take(n) {
                    self.process_action_keycode(*kc, key_state);
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
        for kc in keycodes.iter().take(n) {
            self.process_action_keycode(*kc, key_state);
        }
        self.process_key_action_normal(action, key_state);
    }

    /// Tap action, send a key when the key is pressed, then release the key.
    async fn process_key_action_tap(
        &mut self,
        action: Action,
        mut key_state: KeyState,
        sender: &mut Sender<'a, CriticalSectionRawMutex, KeyboardReportMessage, 8>,
    ) {
        if key_state.changed && key_state.pressed {
            self.process_key_action_normal(action, key_state);

            // Wait 10ms, then send release
            Timer::after_millis(10).await;
            // Manually trigger send report
            self.send_keyboard_report(sender).await;
            self.send_media_report(sender).await;
            self.send_system_control_report(sender).await;
            self.send_mouse_report(sender).await;

            key_state.pressed = false;
            self.process_key_action_normal(action, key_state);
        }
    }

    /// Process tap/hold action.
    /// There are several cases:
    ///
    /// 1. `key_state.changed` is true, and `key_state.pressed` is false,
    ///     which means the key is released. Then the duration time should be checked
    ///
    /// 2. `key_state.changed` is false, and `key_state.pressed` is true,
    ///     which means that the key is held. The duration time should to be checked.
    ///     
    /// TODO: make tap/hold threshold customizable
    async fn process_key_action_tap_hold(
        &mut self,
        tap_action: Action,
        hold_action: Action,
        row: usize,
        col: usize,
        mut key_state: KeyState,
        sender: &mut Sender<'a, CriticalSectionRawMutex, KeyboardReportMessage, 8>,
    ) {
        if !key_state.pressed && key_state.changed {
            // Case 1, the key is released
            if let Some(s) = key_state.hold_start {
                let d = s.elapsed().as_millis();
                if d < 200 {
                    // Released: tap
                    key_state.pressed = true;
                    debug!("Release tap hold, got TAP: {}, {}", tap_action, key_state);
                    self.process_key_action_tap(tap_action, key_state, sender)
                        .await;
                } else {
                    // Released: hold action
                    debug!("Release tap hold, got HOLD: {}, {}", hold_action, key_state);
                    self.process_key_action_normal(hold_action, key_state);
                }
                // Reset timer after release
                self.matrix.update_timer(row, col);
            }
        } else if key_state.pressed && !key_state.changed {
            // Case 2, the key is held
            if let Some(s) = key_state.hold_start {
                let d = s.elapsed().as_millis();
                if d > 200 {
                    self.process_key_action_normal(hold_action, key_state);
                }
            }
        }
    }

    async fn process_key_action_oneshot(&mut self, _oneshot_action: Action) {
        warn!("oneshot action not implemented");
    }

    // Process a single keycode, typically a basic key or a modifier key.
    fn process_action_keycode(&mut self, key: KeyCode, key_state: KeyState) {
        if key.is_consumer() {
            self.process_action_consumer_control(key, key_state);
        } else if key.is_system() {
            self.process_action_system_control(key, key_state);
        } else if key.is_modifier() {
            self.need_send_key_report = true;
            let modifier_bit = key.as_modifier_bit();
            if key_state.pressed {
                self.register_modifier(modifier_bit);
            } else {
                self.unregister_modifier(modifier_bit);
            }
        } else if key.is_mouse_key() {
            self.process_action_mouse(key, key_state);
        } else if key.is_basic() {
            self.need_send_key_report = true;
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
        if key_state.pressed {
            self.keymap.borrow_mut().activate_layer(layer_num);
        } else {
            self.keymap.borrow_mut().deactivate_layer(layer_num);
        }
    }

    /// Process consumer control action. Consumer control keys are keys in hid consumer page, such as media keys.
    fn process_action_consumer_control(&mut self, key: KeyCode, key_state: KeyState) {
        if key.is_consumer() {
            self.other_report.media_usage_id = if key_state.pressed {
                key.as_consumer_control_usage_id() as u16
            } else {
                0
            };
            self.need_send_consumer_control_report = true;
        }
    }

    /// Process system control action. System control keys are keys in system page, such as power key.
    fn process_action_system_control(&mut self, key: KeyCode, key_state: KeyState) {
        if key.is_system() {
            if key_state.pressed {
                if let Some(system_key) = key.as_system_control_usage_id() {
                    self.other_report.system_usage_id = system_key as u8;
                    self.need_send_system_control_report = true;
                }
            } else {
                self.other_report.system_usage_id = 0;
                self.need_send_system_control_report = true;
            }
        }
    }

    /// Process mouse key action.
    fn process_action_mouse(&mut self, key: KeyCode, key_state: KeyState) {
        if key.is_mouse_key() {
            // Reference(qmk): https://github.com/qmk/qmk_firmware/blob/382c3bd0bd49fc0d53358f45477c48f5ae47f2ff/quantum/mousekey.c#L410
            if key_state.pressed {
                match key {
                    // TODO: Add accerated mode when pressing the mouse key
                    // https://github.com/qmk/qmk_firmware/blob/master/docs/feature_mouse_keys.md#accelerated-mode
                    KeyCode::MouseUp => {
                        self.other_report.y = -self.mouse_key_move_delta;
                    }
                    KeyCode::MouseDown => {
                        self.other_report.y = self.mouse_key_move_delta;
                    }
                    KeyCode::MouseLeft => {
                        self.other_report.x = -self.mouse_key_move_delta;
                    }
                    KeyCode::MouseRight => {
                        self.other_report.x = self.mouse_key_move_delta;
                    }
                    KeyCode::MouseWheelUp => {
                        self.other_report.wheel = self.mouse_wheel_move_delta;
                    }
                    KeyCode::MouseWheelDown => {
                        self.other_report.wheel = -self.mouse_wheel_move_delta;
                    }
                    KeyCode::MouseBtn1 => self.other_report.buttons |= 0b1,
                    KeyCode::MouseBtn2 => self.other_report.buttons |= 0b10,
                    KeyCode::MouseBtn3 => self.other_report.buttons |= 0b100,
                    KeyCode::MouseBtn4 => self.other_report.buttons |= 0b1000,
                    KeyCode::MouseBtn5 => self.other_report.buttons |= 0b10000,
                    KeyCode::MouseBtn6 => self.other_report.buttons |= 0b100000,
                    KeyCode::MouseBtn7 => self.other_report.buttons |= 0b1000000,
                    KeyCode::MouseBtn8 => self.other_report.buttons |= 0b10000000,
                    KeyCode::MouseWheelLeft => {
                        self.other_report.pan = -self.mouse_wheel_move_delta;
                    }
                    KeyCode::MouseWheelRight => {
                        self.other_report.pan = self.mouse_wheel_move_delta;
                    }
                    KeyCode::MouseAccel0 => {}
                    KeyCode::MouseAccel1 => {}
                    KeyCode::MouseAccel2 => {}
                    _ => {}
                }
            } else {
                match key {
                    KeyCode::MouseUp | KeyCode::MouseDown => {
                        self.other_report.y = 0;
                    }
                    KeyCode::MouseLeft | KeyCode::MouseRight => {
                        self.other_report.x = 0;
                    }
                    KeyCode::MouseWheelUp | KeyCode::MouseWheelDown => {
                        self.other_report.wheel = 0;
                    }
                    KeyCode::MouseWheelLeft | KeyCode::MouseWheelRight => {
                        self.other_report.pan = 0;
                    }
                    KeyCode::MouseBtn1 => self.other_report.buttons &= 0b0,
                    KeyCode::MouseBtn2 => self.other_report.buttons &= 0b01,
                    KeyCode::MouseBtn3 => self.other_report.buttons &= 0b011,
                    KeyCode::MouseBtn4 => self.other_report.buttons &= 0b0111,
                    KeyCode::MouseBtn5 => self.other_report.buttons &= 0b01111,
                    KeyCode::MouseBtn6 => self.other_report.buttons &= 0b011111,
                    KeyCode::MouseBtn7 => self.other_report.buttons &= 0b0111111,
                    KeyCode::MouseBtn8 => self.other_report.buttons &= 0b01111111,
                    _ => {}
                }
            }
            self.need_send_mouse_report = true;
        }
    }

    /// Register a key to be sent in hid report.
    fn register_keycode(&mut self, key: KeyCode) {
        if let Some(index) = self.report.keycodes.iter().position(|&k| k == 0) {
            self.report.keycodes[index] = key as u8;
        }
    }

    /// Unregister a key from hid report.
    fn unregister_keycode(&mut self, key: KeyCode) {
        if let Some(index) = self.report.keycodes.iter().position(|&k| k == key as u8) {
            self.report.keycodes[index] = 0;
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
