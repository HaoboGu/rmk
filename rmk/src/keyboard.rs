use crate::{
    action::{Action, KeyAction},
    hid::{ConnectionType, HidWriterWrapper},
    keyboard_macro::{MacroOperation, NUM_MACRO},
    keycode::{KeyCode, ModifierCombination},
    keymap::KeyMap,
    matrix::{KeyState, MatrixTrait},
    usb::descriptor::{CompositeReport, CompositeReportType, ViaReport},
    KEYBOARD_STATE,
};
use core::cell::RefCell;
use defmt::{debug, error, warn};
use embassy_futures::yield_now;
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Receiver, Sender},
};
use embassy_time::{Instant, Timer};
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
    M: MatrixTrait,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    keyboard: &mut Keyboard<'a, M, ROW, COL, NUM_LAYER>,
    sender: &Sender<'a, CriticalSectionRawMutex, KeyboardReportMessage, 8>,
) {
    KEYBOARD_STATE.store(true, core::sync::atomic::Ordering::Release);
    loop {
        keyboard.scan_matrix(sender).await;
        keyboard.send_keyboard_report(sender).await;
        keyboard.send_media_report(sender).await;
        keyboard.send_mouse_report(sender).await;
        keyboard.send_system_control_report(sender).await;
        Timer::after_micros(100).await;
    }
}

/// This task processes all keyboard reports and send them to the host
pub(crate) async fn communication_task<'a, W: HidWriterWrapper, W2: HidWriterWrapper>(
    receiver: &Receiver<'a, CriticalSectionRawMutex, KeyboardReportMessage, 8>,
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
            if let Err(e) = match other_hid_writer.get_conn_type() {
                ConnectionType::Usb => other_hid_writer.write(&buf[0..s + 1]).await,
                ConnectionType::Ble => other_hid_writer.write(&buf[1..s + 1]).await,
            } {
                error!("Send other report error: {}", e);
            }
        }
        Err(_) => error!("Serialize other report error"),
    }
}
pub(crate) struct Keyboard<
    'a,
    M: MatrixTrait,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
> {
    /// Keyboard matrix, use COL2ROW by default
    pub(crate) matrix: M,

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

impl<'a, M: MatrixTrait, const ROW: usize, const COL: usize, const NUM_LAYER: usize>
    Keyboard<'a, M, ROW, COL, NUM_LAYER>
{
    pub(crate) fn new(matrix: M, keymap: &'a RefCell<KeyMap<ROW, COL, NUM_LAYER>>) -> Self {
        Keyboard {
            matrix,
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
        sender: &Sender<'a, CriticalSectionRawMutex, KeyboardReportMessage, 8>,
    ) {
        if self.need_send_key_report {
            debug!(
                "Sending keyboard report: {=[u8]:#X}, modifier: {:b}",
                self.report.keycodes, self.report.modifier
            );
            sender
                .send(KeyboardReportMessage::KeyboardReport(self.report))
                .await;
            self.need_send_key_report = false;
            // Yield once after sending the report to channel
            yield_now().await;
        }
    }

    /// Send system control report if needed
    pub(crate) async fn send_system_control_report(
        &mut self,
        sender: &Sender<'a, CriticalSectionRawMutex, KeyboardReportMessage, 8>,
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
            yield_now().await;
        }
    }

    /// Send media report if needed
    pub(crate) async fn send_media_report(
        &mut self,
        sender: &Sender<'a, CriticalSectionRawMutex, KeyboardReportMessage, 8>,
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
            yield_now().await;
        }
    }

    /// Send mouse report if needed
    pub(crate) async fn send_mouse_report(
        &mut self,
        sender: &Sender<'a, CriticalSectionRawMutex, KeyboardReportMessage, 8>,
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
            yield_now().await;
        }
    }

    /// Main keyboard task, it scans matrix, processes active keys.
    /// If there is any change of key states, set self.changed=true.
    ///
    /// `sender` is required because when there's tap action, the report should be sent immediately and then continue scanning
    pub(crate) async fn scan_matrix(
        &mut self,
        sender: &Sender<'a, CriticalSectionRawMutex, KeyboardReportMessage, 8>,
    ) {
        #[cfg(feature = "async_matrix")]
        self.matrix.wait_for_key().await;

        // Matrix scan
        self.matrix.scan().await;

        // Check matrix states, process key if there is a key state change
        // Keys are processed in the following order:
        // process_key_change -> process_key_action_* -> process_action_*
        for row_idx in 0..self.matrix.get_row_num() {
            for col_idx in 0..self.matrix.get_col_num() {
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
        sender: &Sender<'a, CriticalSectionRawMutex, KeyboardReportMessage, 8>,
    ) {
        let key_state = self.matrix.get_key_state(row, col);

        // Matrix should process key pressed event first, record the timestamp of key changes
        if key_state.pressed {
            self.matrix.update_key_state(row, col, |ks| {
                ks.start_timer();
            });
        }

        // Process key
        let action = self
            .keymap
            .borrow_mut()
            .get_action_with_layer_cache(row, col, key_state);
        match action {
            KeyAction::No | KeyAction::Transparent => (),
            KeyAction::Single(a) => self.process_key_action_normal(a, key_state, sender).await,
            KeyAction::WithModifier(a, m) => {
                self.process_key_action_with_modifier(a, m, key_state, sender)
                    .await
            }
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

    async fn process_key_action_normal(
        &mut self,
        action: Action,
        key_state: KeyState,
        sender: &Sender<'a, CriticalSectionRawMutex, KeyboardReportMessage, 8>,
    ) {
        match action {
            Action::Key(key) => self.process_action_keycode(key, key_state, sender).await,
            Action::LayerOn(layer_num) => self.process_action_layer_switch(layer_num, key_state),
            Action::LayerOff(layer_num) => {
                // Turn off a layer temporarily when the key is pressed
                // Reactivate the layer after the key is released
                if key_state.is_pressing() {
                    self.keymap.borrow_mut().deactivate_layer(layer_num);
                }
            }
            Action::LayerToggle(layer_num) => {
                // Toggle a layer when the key is release
                if key_state.is_releasing() {
                    self.keymap.borrow_mut().toggle_layer(layer_num);
                }
            }
            Action::Modifier(modifier) => {
                let (keycodes, n) = modifier.to_modifier_keycodes();
                for kc in keycodes.iter().take(n) {
                    self.process_action_keycode(*kc, key_state, sender).await;
                }
            }
            _ => (),
        }
    }

    async fn process_key_action_with_modifier(
        &mut self,
        action: Action,
        modifier: ModifierCombination,
        key_state: KeyState,
        sender: &Sender<'a, CriticalSectionRawMutex, KeyboardReportMessage, 8>,
    ) {
        if key_state.is_pressing() {
            // Process modifier
            let (keycodes, n) = modifier.to_modifier_keycodes();
            for kc in keycodes.iter().take(n) {
                self.process_action_keycode(*kc, key_state, sender).await;
            }
            // Send the modifier first, then send the key
            self.send_keyboard_report(sender).await;
            self.process_key_action_normal(action, key_state, sender)
                .await;
        } else {
            // Releasing, release the key first, then release the modifier
            self.process_key_action_normal(action, key_state, sender)
                .await;
            self.send_keyboard_report(sender).await;
            let (keycodes, n) = modifier.to_modifier_keycodes();
            for kc in keycodes.iter().take(n) {
                self.process_action_keycode(*kc, key_state, sender).await;
            }
        }
    }

    /// Tap action, send a key when the key is pressed, then release the key.
    async fn process_key_action_tap(
        &mut self,
        action: Action,
        mut key_state: KeyState,
        sender: &Sender<'a, CriticalSectionRawMutex, KeyboardReportMessage, 8>,
    ) {
        if key_state.is_pressing() {
            self.process_key_action_normal(action, key_state, sender)
                .await;

            // Wait 10ms, then send release
            Timer::after_millis(10).await;
            // Manually trigger send report
            self.send_keyboard_report(sender).await;
            self.send_media_report(sender).await;
            self.send_system_control_report(sender).await;
            self.send_mouse_report(sender).await;

            key_state.pressed = false;
            self.process_key_action_normal(action, key_state, sender)
                .await;
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
        sender: &Sender<'a, CriticalSectionRawMutex, KeyboardReportMessage, 8>,
    ) {
        if key_state.is_releasing() {
            // Case 1, the key is released
            match key_state.hold_start {
                Some(s) => {
                    // Released: tap
                    // The hold_start isn't cleared, means that the key is released within the tap/hold threshold
                    key_state.pressed = true;
                    debug!(
                        "Release tap hold, got TAP: {}, {}, time elapsed: {}ms",
                        tap_action,
                        key_state,
                        s.elapsed().as_millis()
                    );
                    self.process_key_action_tap(tap_action, key_state, sender)
                        .await;

                    // Reset timer after release
                    self.matrix.update_key_state(row, col, |ks| {
                        ks.clear_timer();
                    });
                }
                None => {
                    // Released: hold action
                    // The hold_start is cleared, means that the key is released after the tap/hold threshold
                    debug!("Release tap hold, got HOLD: {}, {}", hold_action, key_state);
                    self.process_key_action_normal(hold_action, key_state, sender)
                        .await;
                }
            }
        } else if key_state.pressed && !key_state.changed {
            // Case 2, the key is held
            if let Some(s) = key_state.hold_start {
                let d = s.elapsed().as_millis();
                if d > 200 {
                    // The key is held for more than 200ms, send hold action, then clear timer
                    self.process_key_action_normal(hold_action, key_state, sender)
                        .await;

                    // Clear timer if the key is held
                    self.matrix.update_key_state(row, col, |ks| {
                        ks.clear_timer();
                    });
                }
            }
        }
    }

    async fn process_key_action_oneshot(&mut self, _oneshot_action: Action) {
        warn!("oneshot action not implemented");
    }

    // Process a single keycode, typically a basic key or a modifier key.
    async fn process_action_keycode(
        &mut self,
        key: KeyCode,
        key_state: KeyState,
        sender: &Sender<'a, CriticalSectionRawMutex, KeyboardReportMessage, 8>,
    ) {
        if key.is_consumer() {
            self.process_action_consumer_control(key, key_state);
        } else if key.is_system() {
            self.process_action_system_control(key, key_state);
        } else if key.is_mouse_key() {
            self.process_action_mouse(key, key_state);
        } else if key.is_basic() {
            self.need_send_key_report = true;
            if key_state.pressed {
                self.register_key(key);
            } else {
                self.unregister_key(key);
            }
        } else if key.is_macro() {
            // Process macro
            self.process_action_macro(key, key_state, sender).await;
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
                    KeyCode::MouseBtn1 => self.other_report.buttons &= !0b1,
                    KeyCode::MouseBtn2 => self.other_report.buttons &= !0b10,
                    KeyCode::MouseBtn3 => self.other_report.buttons &= !0b100,
                    KeyCode::MouseBtn4 => self.other_report.buttons &= !0b1000,
                    KeyCode::MouseBtn5 => self.other_report.buttons &= !0b10000,
                    KeyCode::MouseBtn6 => self.other_report.buttons &= !0b100000,
                    KeyCode::MouseBtn7 => self.other_report.buttons &= !0b1000000,
                    KeyCode::MouseBtn8 => self.other_report.buttons &= !0b10000000,
                    _ => {}
                }
            }
            self.need_send_mouse_report = true;
        }
    }

    async fn process_action_macro(
        &mut self,
        key: KeyCode,
        key_state: KeyState,
        sender: &Sender<'a, CriticalSectionRawMutex, KeyboardReportMessage, 8>,
    ) {
        // Execute the macro only when releasing the key
        if !key_state.is_releasing() {
            return;
        }

        // Get macro index
        if let Some(macro_idx) = key.as_macro_index() {
            if macro_idx as usize >= NUM_MACRO {
                error!("Macro idx invalid: {}", macro_idx);
                return;
            }
            // Read macro operations untill the end of the macro
            if let Some(macro_start_idx) = self.keymap.borrow().get_macro_start(macro_idx) {
                let mut offset = 0;
                loop {
                    // First, get the next macro operation
                    let (operation, new_offset) = self
                        .keymap
                        .borrow()
                        .get_next_macro_operation(macro_start_idx, offset);
                    // Execute the operation
                    match operation {
                        MacroOperation::Press(k) => {
                            self.need_send_key_report = true;
                            self.register_key(k);
                        }
                        MacroOperation::Release(k) => {
                            self.need_send_key_report = true;
                            self.unregister_key(k);
                        }
                        MacroOperation::Tap(k) => {
                            self.need_send_key_report = true;
                            self.register_key(k);
                            self.send_keyboard_report(sender).await;
                            embassy_time::Timer::after_millis(2).await;
                            self.need_send_key_report = true;
                            self.unregister_key(k)
                        }
                        MacroOperation::Text(k, is_cap) => {
                            self.need_send_key_report = true;
                            if is_cap {
                                // If it's a capital letter, send shift first
                                self.register_modifier(KeyCode::LShift.as_modifier_bit());
                                self.send_keyboard_report(sender).await;
                                self.need_send_key_report = true;
                            }
                            self.register_keycode(k);
                            self.send_keyboard_report(sender).await;
                            self.need_send_key_report = true;
                            self.unregister_keycode(k);
                            if is_cap {
                                self.send_keyboard_report(sender).await;
                                self.need_send_key_report = true;
                                self.unregister_modifier(KeyCode::LShift.as_modifier_bit());
                            }
                        }
                        MacroOperation::Delay(t) => {
                            embassy_time::Timer::after_millis(t as u64).await;
                        }
                        MacroOperation::End => {
                            self.send_keyboard_report(sender).await;
                            break;
                        }
                    };

                    // Send the item in the macro sequence
                    self.send_keyboard_report(sender).await;

                    offset = new_offset;
                    if offset > self.keymap.borrow().macro_cache.len() {
                        break;
                    }
                }
            } else {
                error!("Macro not found");
            }
        }
    }

    /// Register a key, the key can be a basic keycode or a modifier.
    fn register_key(&mut self, key: KeyCode) {
        if key.is_modifier() {
            self.register_modifier(key.as_modifier_bit());
        } else if key.is_basic() {
            self.register_keycode(key);
        }
    }

    /// Unregister a key, the key can be a basic keycode or a modifier.
    fn unregister_key(&mut self, key: KeyCode) {
        if key.is_modifier() {
            self.unregister_modifier(key.as_modifier_bit());
        } else if key.is_basic() {
            self.unregister_keycode(key);
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
