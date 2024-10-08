use crate::{
    action::{Action, KeyAction},
    hid::{ConnectionType, HidWriterWrapper},
    keyboard_macro::{MacroOperation, NUM_MACRO},
    keycode::{KeyCode, ModifierCombination},
    keymap::KeyMap,
    matrix::KeyState,
    usb::descriptor::{CompositeReport, CompositeReportType, ViaReport},
    KEYBOARD_STATE,
};
use core::cell::RefCell;
use defmt::{debug, error, warn};
use embassy_futures::{select::select, yield_now};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, Receiver, Sender},
};
use embassy_time::{Instant, Timer};
use usbd_hid::descriptor::KeyboardReport;

pub(crate) struct KeyEvent {
    pub(crate) row: u8,
    pub(crate) col: u8,
    pub(crate) key_state: KeyState,
}

pub(crate) static key_event_channel: Channel<CriticalSectionRawMutex, KeyEvent, 16> =
    Channel::new();
pub(crate) static keyboard_report_channel: Channel<
    CriticalSectionRawMutex,
    KeyboardReportMessage,
    8,
> = Channel::new();

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
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    keyboard: &mut Keyboard<'a, ROW, COL, NUM_LAYER>,
) {
    KEYBOARD_STATE.store(true, core::sync::atomic::Ordering::Release);
    loop {
        keyboard.process().await;
        keyboard.send_keyboard_report().await;
        keyboard.send_media_report().await;
        keyboard.send_mouse_report().await;
        keyboard.send_system_control_report().await;
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
pub(crate) struct Keyboard<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize> {
    /// Keymap
    pub(crate) keymap: &'a RefCell<KeyMap<ROW, COL, NUM_LAYER>>,

    /// Report Sender
    pub(crate) sender: &'a Sender<'a, CriticalSectionRawMutex, KeyboardReportMessage, 8>,

    /// Unprocessed events
    unprocessed_events: heapless::Vec<KeyEvent, 8>,

    /// Timer which records the timestamp of key changes
    pub(crate) timer: [[Option<Instant>; ROW]; COL],

    /// Keyboard internal hid report buf
    report: KeyboardReport,

    /// Internal composite report: mouse + media(consumer) + system control
    other_report: CompositeReport,

    /// Via report
    via_report: ViaReport,

    /// Mouse key is different from other keyboard keys, it should be sent continuously while the key is pressed.
    /// The last tick of mouse is recorded to control the reporting rate.
    last_mouse_tick: u64,

    /// The current distance of mouse key moving
    mouse_key_move_delta: i8,
    mouse_wheel_move_delta: i8,
}

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize>
    Keyboard<'a, ROW, COL, NUM_LAYER>
{
    pub(crate) fn new(
        keymap: &'a RefCell<KeyMap<ROW, COL, NUM_LAYER>>,
        sender: &'a Sender<'a, CriticalSectionRawMutex, KeyboardReportMessage, 8>,
    ) -> Self {
        Keyboard {
            keymap,
            sender,
            timer: [[None; ROW]; COL],
            unprocessed_events: heapless::Vec::new(),
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
            last_mouse_tick: 0,
            mouse_key_move_delta: 8,
            mouse_wheel_move_delta: 1,
        }
    }

    pub(crate) async fn send_keyboard_report(&mut self) {
        debug!(
            "Sending keyboard report: {=[u8]:#X}, modifier: {:b}",
            self.report.keycodes, self.report.modifier
        );
        self.sender
            .send(KeyboardReportMessage::KeyboardReport(self.report))
            .await;
        // Yield once after sending the report to channel
        yield_now().await;
    }

    /// Send system control report if needed
    pub(crate) async fn send_system_control_report(&mut self) {
        self.sender
            .send(KeyboardReportMessage::CompositeReport(
                self.other_report,
                CompositeReportType::System,
            ))
            .await;
        self.other_report.system_usage_id = 0;
        yield_now().await;
    }

    /// Send media report if needed
    pub(crate) async fn send_media_report(&mut self) {
        self.sender
            .send(KeyboardReportMessage::CompositeReport(
                self.other_report,
                CompositeReportType::Media,
            ))
            .await;
        self.other_report.media_usage_id = 0;
        yield_now().await;
    }

    /// Send mouse report if needed
    /// TODO: mouse report rework
    pub(crate) async fn send_mouse_report(&mut self) {
        // Prevent mouse report flooding, set maximum mouse report rate to 100 HZ
        let cur_tick = Instant::now().as_millis();
        // The default internal of sending mouse report is 20 ms, same as qmk accelerated mode
        // TODO: make it configurable
        if cur_tick - self.last_mouse_tick > 20 {
            self.sender
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
            // self.need_send_mouse_report = false;
        }
        yield_now().await;
    }

    /// Main keyboard task, it receives input devices result, processes active keys.
    pub(crate) async fn process(&mut self) {
        let KeyEvent {
            row,
            col,
            key_state: ks,
        } = key_event_channel.receive().await;
        let row_idx = row as usize;
        let col_idx = col as usize;

        // Process the key change
        self.process_key_change(row_idx, col_idx, ks).await;

        // After processing the key change, check if there are unprocessed events
        // This will happen if there's recursion in key processing
        loop {
            if self.unprocessed_events.is_empty() {
                break;
            }
            // Process unprocessed events
            if let Some(KeyEvent {
                row,
                col,
                key_state,
            }) = self.unprocessed_events.pop()
            {
                self.process_key_change(row as usize, col as usize, key_state)
                    .await;
            }
        }
    }

    /// Process key changes at (row, col)
    async fn process_key_change(&mut self, row: usize, col: usize, key_state: KeyState) {
        // Matrix should process key pressed event first, record the timestamp of key changes
        if key_state.pressed {
            self.timer[col][row] = Some(Instant::now());
        }

        // Process key
        let action = self
            .keymap
            .borrow_mut()
            .get_action_with_layer_cache(row, col, key_state);
        match action {
            KeyAction::No | KeyAction::Transparent => (),
            KeyAction::Single(a) => self.process_key_action_normal(a, key_state).await,
            KeyAction::WithModifier(a, m) => {
                self.process_key_action_with_modifier(a, m, key_state).await
            }
            KeyAction::Tap(a) => self.process_key_action_tap(a, key_state).await,
            KeyAction::TapHold(tap_action, hold_action) => {
                self.process_key_action_tap_hold(tap_action, hold_action, row, col, key_state)
                    .await;
            }
            KeyAction::OneShot(oneshot_action) => {
                self.process_key_action_oneshot(oneshot_action).await
            }
            KeyAction::LayerTapHold(tap_action, layer_num) => {
                let layer_action = Action::LayerOn(layer_num);
                self.process_key_action_tap_hold(tap_action, layer_action, row, col, key_state)
                    .await;
            }
            KeyAction::ModifierTapHold(tap_action, modifier) => {
                let modifier_action = Action::Modifier(modifier);
                self.process_key_action_tap_hold(tap_action, modifier_action, row, col, key_state)
                    .await;
            }
        }
    }

    async fn process_key_action_normal(&mut self, action: Action, key_state: KeyState) {
        match action {
            Action::Key(key) => self.process_action_keycode(key, key_state).await,
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
            Action::LayerToggleOnly(layer_num) => {
                // Activate a layer and deactivate all other layers(except default layer)
                if key_state.is_pressing() {
                    // Disable all layers except the default layer
                    let default_layer = self.keymap.borrow().get_default_layer();
                    for i in 0..NUM_LAYER as u8 {
                        if i != default_layer {
                            self.keymap.borrow_mut().deactivate_layer(i);
                        }
                    }
                    // Activate the target layer
                    self.keymap.borrow_mut().activate_layer(layer_num);
                }
            }
            Action::DefaultLayer(layer_num) => {
                // Set the default layer
                self.keymap.borrow_mut().set_default_layer(layer_num);
            }
            Action::Modifier(modifier) => {
                let (keycodes, n) = modifier.to_modifier_keycodes();
                for kc in keycodes.iter().take(n) {
                    self.process_action_keycode(*kc, key_state).await;
                }
            }
        }
    }

    async fn process_key_action_with_modifier(
        &mut self,
        action: Action,
        modifier: ModifierCombination,
        key_state: KeyState,
    ) {
        if key_state.is_pressing() {
            // Process modifier
            let (keycodes, n) = modifier.to_modifier_keycodes();
            for kc in keycodes.iter().take(n) {
                self.process_action_keycode(*kc, key_state).await;
            }
            // Send the modifier first, then send the key
            self.send_keyboard_report().await;
            self.process_key_action_normal(action, key_state).await;
        } else {
            // Releasing, release the key first, then release the modifier
            self.process_key_action_normal(action, key_state).await;
            self.send_keyboard_report().await;
            let (keycodes, n) = modifier.to_modifier_keycodes();
            for kc in keycodes.iter().take(n) {
                self.process_action_keycode(*kc, key_state).await;
            }
        }
    }

    /// Tap action, send a key when the key is pressed, then release the key.
    async fn process_key_action_tap(&mut self, action: Action, mut key_state: KeyState) {
        if key_state.is_pressing() {
            self.process_key_action_normal(action, key_state).await;

            // Wait 10ms, then send release
            Timer::after_millis(10).await;
            // FIXME: double check whether the tap report is sent in process_key_action_normal
            // // Manually trigger send report
            // self.send_keyboard_report().await;
            // self.send_media_report().await;
            // self.send_system_control_report().await;
            // self.send_mouse_report().await;

            key_state.pressed = false;
            self.process_key_action_normal(action, key_state).await;
        }
    }

    /// Process tap/hold action.
    ///
    /// This function will wait until timeout or a new key event comes:
    /// - timeout: trigger hold action
    /// - new key event: means there's another key within the threshold
    ///     - if it's the same key, trigger tap action
    ///     - if it's another key, trigger hold action + that key
    ///
    /// This behavior is same as "Hold On Other Key Press" in qmk or "hold-preferred" in zmk
    ///     
    /// TODO: make tap/hold threshold customizable
    async fn process_key_action_tap_hold(
        &mut self,
        tap_action: Action,
        hold_action: Action,
        row: usize,
        col: usize,
        mut key_state: KeyState,
    ) {
        if key_state.is_pressing() {
            self.timer[col][row] = Some(Instant::now());
            let hold_timeout = embassy_time::Timer::after_millis(200);
            match select(hold_timeout, key_event_channel.receive()).await {
                embassy_futures::select::Either::First(_) => {
                    // Timeout, trigger hold
                    debug!("Hold timeout, got HOLD: {}, {}", hold_action, key_state);
                    self.process_key_action_normal(hold_action, key_state).await;
                }
                embassy_futures::select::Either::Second(key_event) => {
                    if key_event.row == row as u8 && key_event.col == col as u8 {
                        // If it's same key event and releasing within 200ms, trigger tap
                        if key_event.key_state.is_releasing() {
                            key_state.pressed = true;
                            let elapsed = self.timer[col][row].unwrap().elapsed().as_millis();
                            debug!("TAP action: {}, time elapsed: {}ms", tap_action, elapsed);
                            self.process_key_action_tap(tap_action, key_state).await;

                            // Clear timer
                            self.timer[col][row] = None;
                        }
                    } else {
                        // A different key comes within the threshold, trigger hold + that key

                        // Process hold action first
                        self.process_key_action_normal(hold_action, key_state).await;

                        // The actual processing is postponed because we cannot do recursion on async function without alloc
                        // After current key processing is done, we can process events in queue until the queue is empty
                        if self.unprocessed_events.push(key_event).is_err() {
                            warn!("unprocessed event queue is full, dropping event");
                        }
                    }
                }
            }
        } else if key_state.is_releasing() {
            if let Some(start) = self.timer[col][row] {
                let elapsed = start.elapsed().as_millis();
                if elapsed > 200 {
                    // Release hold action, then clear timer
                    debug!(
                        "HOLD releasing: {}, {}, time elapsed: {}ms",
                        hold_action, key_state, elapsed
                    );
                    self.process_key_action_normal(hold_action, key_state).await;
                    self.timer[col][row] = None;
                }
            }
        }
    }

    async fn process_key_action_oneshot(&mut self, _oneshot_action: Action) {
        warn!("oneshot action not implemented");
    }

    // Process a single keycode, typically a basic key or a modifier key.
    async fn process_action_keycode(&mut self, key: KeyCode, key_state: KeyState) {
        if key.is_consumer() {
            self.process_action_consumer_control(key, key_state).await;
        } else if key.is_system() {
            self.process_action_system_control(key, key_state).await;
        } else if key.is_mouse_key() {
            self.process_action_mouse(key, key_state).await;
        } else if key.is_basic() {
            if key_state.pressed {
                self.register_key(key);
            } else {
                self.unregister_key(key);
            }
            self.send_keyboard_report().await;
        } else if key.is_macro() {
            // Process macro
            self.process_action_macro(key, key_state).await;
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
    async fn process_action_consumer_control(&mut self, key: KeyCode, key_state: KeyState) {
        if key.is_consumer() {
            self.other_report.media_usage_id = if key_state.pressed {
                key.as_consumer_control_usage_id() as u16
            } else {
                0
            };

            self.send_media_report().await;
        }
    }

    /// Process system control action. System control keys are keys in system page, such as power key.
    async fn process_action_system_control(&mut self, key: KeyCode, key_state: KeyState) {
        if key.is_system() {
            if key_state.pressed {
                if let Some(system_key) = key.as_system_control_usage_id() {
                    self.other_report.system_usage_id = system_key as u8;
                    self.send_system_control_report().await;
                }
            } else {
                self.other_report.system_usage_id = 0;
                self.send_system_control_report().await;
            }
        }
    }

    /// Process mouse key action.
    async fn process_action_mouse(&mut self, key: KeyCode, key_state: KeyState) {
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
            self.send_mouse_report().await;
        }
    }

    async fn process_action_macro(&mut self, key: KeyCode, key_state: KeyState) {
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
            let macro_idx = self.keymap.borrow().get_macro_start(macro_idx);
            if let Some(macro_start_idx) = macro_idx {
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
                            self.register_key(k);
                        }
                        MacroOperation::Release(k) => {
                            self.unregister_key(k);
                        }
                        MacroOperation::Tap(k) => {
                            self.register_key(k);
                            self.send_keyboard_report().await;
                            embassy_time::Timer::after_millis(2).await;
                            self.unregister_key(k);
                        }
                        MacroOperation::Text(k, is_cap) => {
                            if is_cap {
                                // If it's a capital letter, send shift first
                                self.register_modifier(KeyCode::LShift.as_modifier_bit());
                                self.send_keyboard_report().await;
                            }
                            self.register_keycode(k);
                            self.send_keyboard_report().await;

                            self.unregister_keycode(k);
                            if is_cap {
                                self.send_keyboard_report().await;
                                self.unregister_modifier(KeyCode::LShift.as_modifier_bit());
                            }
                        }
                        MacroOperation::Delay(t) => {
                            embassy_time::Timer::after_millis(t as u64).await;
                        }
                        MacroOperation::End => {
                            self.send_keyboard_report().await;
                            break;
                        }
                    };

                    // Send the item in the macro sequence
                    self.send_keyboard_report().await;

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
