use crate::{
    action::{Action, KeyAction},
    hid::{ConnectionType, HidWriterWrapper},
    keyboard_macro::{MacroOperation, NUM_MACRO},
    keycode::{KeyCode, ModifierCombination},
    keymap::KeyMap,
    usb::descriptor::{CompositeReport, CompositeReportType, ViaReport},
    KEYBOARD_STATE,
};
use core::cell::RefCell;
use defmt::{debug, error, warn, Format};
use embassy_futures::{select::select, yield_now};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, Receiver, Sender},
};
use embassy_time::{Instant, Timer};
use heapless::{FnvIndexMap, Vec};
use postcard::experimental::max_size::MaxSize;
use serde::{Deserialize, Serialize};
use usbd_hid::descriptor::KeyboardReport;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Format, MaxSize)]
pub(crate) struct KeyEvent {
    pub(crate) row: u8,
    pub(crate) col: u8,
    pub(crate) pressed: bool,
}
pub(crate) const EVENT_CHANNEL_SIZE: usize = 32;
pub(crate) const REPORT_CHANNEL_SIZE: usize = 32;

pub(crate) static key_event_channel: Channel<
    CriticalSectionRawMutex,
    KeyEvent,
    EVENT_CHANNEL_SIZE,
> = Channel::new();
pub(crate) static keyboard_report_channel: Channel<
    CriticalSectionRawMutex,
    KeyboardReportMessage,
    REPORT_CHANNEL_SIZE,
> = Channel::new();

/// Matrix scanning task sends this [KeyboardReportMessage] to communication task.
pub(crate) enum KeyboardReportMessage {
    /// Normal keyboard hid report
    KeyboardReport(KeyboardReport),
    /// Other types of keyboard reports: mouse + media(consumer) + system control
    CompositeReport(CompositeReport, CompositeReportType),
}

/// This task processes all keyboard reports and send them to the host
pub(crate) async fn communication_task<'a, W: HidWriterWrapper, W2: HidWriterWrapper>(
    receiver: &Receiver<'a, CriticalSectionRawMutex, KeyboardReportMessage, REPORT_CHANNEL_SIZE>,
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
    pub(crate) keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER>>,

    /// Report Sender
    pub(crate) sender:
        &'a Sender<'a, CriticalSectionRawMutex, KeyboardReportMessage, REPORT_CHANNEL_SIZE>,

    /// Unprocessed events
    unprocessed_events: Vec<KeyEvent, 16>,

    /// Timer which records the timestamp of key changes
    pub(crate) timer: [[Option<Instant>; ROW]; COL],

    /// Keyboard internal hid report buf
    report: KeyboardReport,

    /// Internal composite report: mouse + media(consumer) + system control
    other_report: CompositeReport,

    /// Via report
    via_report: ViaReport,

    /// Mouse key is different from other keyboard keys, it should be sent continuously while the key is pressed.
    /// `last_mouse_tick` tracks at most 8 mouse keys, with its recent state.
    /// It can be used to control the mouse report rate and release mouse key properly.
    /// The key is mouse keycode, the value is the last action and its timestamp.
    last_mouse_tick: FnvIndexMap<KeyCode, (bool, Instant), 4>,

    /// The current distance of mouse key moving
    mouse_key_move_delta: i8,
    mouse_wheel_move_delta: i8,
}

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize>
    Keyboard<'a, ROW, COL, NUM_LAYER>
{
    pub(crate) fn new(
        keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER>>,
        sender: &'a Sender<'a, CriticalSectionRawMutex, KeyboardReportMessage, REPORT_CHANNEL_SIZE>,
    ) -> Self {
        Keyboard {
            keymap,
            sender,
            timer: [[None; ROW]; COL],
            unprocessed_events: Vec::new(),
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
            last_mouse_tick: FnvIndexMap::new(),
            mouse_key_move_delta: 8,
            mouse_wheel_move_delta: 1,
        }
    }

    pub(crate) async fn send_keyboard_report(&mut self) {
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
    pub(crate) async fn send_mouse_report(&mut self) {
        // Prevent mouse report flooding, set maximum mouse report rate to 50 HZ
        self.sender
            .send(KeyboardReportMessage::CompositeReport(
                self.other_report,
                CompositeReportType::Mouse,
            ))
            .await;
        yield_now().await;
    }

    /// Main keyboard task, it receives input devices result, processes keys.
    /// The report is sent to communication task via keyboard_report_channel, and finally sent to the host
    pub(crate) async fn run(&mut self) {
        KEYBOARD_STATE.store(true, core::sync::atomic::Ordering::Release);
        loop {
            let key_event = key_event_channel.receive().await;

            // Process the key change
            self.process_key_change(key_event).await;

            // After processing the key change, check if there are unprocessed events
            // This will happen if there's recursion in key processing
            loop {
                if self.unprocessed_events.is_empty() {
                    break;
                }
                // Process unprocessed events
                if let Some(e) = self.unprocessed_events.pop() {
                    self.process_key_change(e).await;
                }
            }
        }
    }

    /// Process key changes at (row, col)
    async fn process_key_change(&mut self, key_event: KeyEvent) {
        // Matrix should process key pressed event first, record the timestamp of key changes
        if key_event.pressed {
            self.timer[key_event.col as usize][key_event.row as usize] = Some(Instant::now());
        }

        // Process key
        let action = self
            .keymap
            .borrow_mut()
            .get_action_with_layer_cache(key_event);
        match action {
            KeyAction::No | KeyAction::Transparent => (),
            KeyAction::Single(a) => self.process_key_action_normal(a, key_event).await,
            KeyAction::WithModifier(a, m) => {
                self.process_key_action_with_modifier(a, m, key_event).await
            }
            KeyAction::Tap(a) => self.process_key_action_tap(a, key_event).await,
            KeyAction::TapHold(tap_action, hold_action) => {
                self.process_key_action_tap_hold(tap_action, hold_action, key_event)
                    .await;
            }
            KeyAction::OneShot(oneshot_action) => {
                self.process_key_action_oneshot(oneshot_action).await
            }
            KeyAction::LayerTapHold(tap_action, layer_num) => {
                let layer_action = Action::LayerOn(layer_num);
                self.process_key_action_tap_hold(tap_action, layer_action, key_event)
                    .await;
            }
            KeyAction::ModifierTapHold(tap_action, modifier) => {
                let modifier_action = Action::Modifier(modifier);
                self.process_key_action_tap_hold(tap_action, modifier_action, key_event)
                    .await;
            }
        }
    }

    async fn process_key_action_normal(&mut self, action: Action, key_event: KeyEvent) {
        match action {
            Action::Key(key) => self.process_action_keycode(key, key_event).await,
            Action::LayerOn(layer_num) => self.process_action_layer_switch(layer_num, key_event),
            Action::LayerOff(layer_num) => {
                // Turn off a layer temporarily when the key is pressed
                // Reactivate the layer after the key is released
                if key_event.pressed {
                    self.keymap.borrow_mut().deactivate_layer(layer_num);
                }
            }
            Action::LayerToggle(layer_num) => {
                // Toggle a layer when the key is release
                if !key_event.pressed {
                    self.keymap.borrow_mut().toggle_layer(layer_num);
                }
            }
            Action::LayerToggleOnly(layer_num) => {
                // Activate a layer and deactivate all other layers(except default layer)
                if key_event.pressed {
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
                    self.process_action_keycode(*kc, key_event).await;
                }
            }
        }
    }

    async fn process_key_action_with_modifier(
        &mut self,
        action: Action,
        modifier: ModifierCombination,
        key_event: KeyEvent,
    ) {
        if key_event.pressed {
            // Process modifier
            let (keycodes, n) = modifier.to_modifier_keycodes();
            for kc in keycodes.iter().take(n) {
                self.process_action_keycode(*kc, key_event).await;
            }
            // Send the modifier first, then send the key
            self.send_keyboard_report().await;
            self.process_key_action_normal(action, key_event).await;
        } else {
            // Releasing, release the key first, then release the modifier
            self.process_key_action_normal(action, key_event).await;
            self.send_keyboard_report().await;
            let (keycodes, n) = modifier.to_modifier_keycodes();
            for kc in keycodes.iter().take(n) {
                self.process_action_keycode(*kc, key_event).await;
            }
        }
    }

    /// Tap action, send a key when the key is pressed, then release the key.
    async fn process_key_action_tap(&mut self, action: Action, mut key_event: KeyEvent) {
        if key_event.pressed {
            self.process_key_action_normal(action, key_event).await;

            // Wait 10ms, then send release
            Timer::after_millis(10).await;

            key_event.pressed = false;
            self.process_key_action_normal(action, key_event).await;
        }
    }

    /// Process tap/hold action for home row mods(HRM)
    /// 
    /// For HRMs, the "tap" action actually has higher priority, especially when typing fast.
    /// 
    /// There are only several cases that we should trigger "hold":
    /// 
    /// - When another key is pressed and released within the tapping-term, or released at approximately the same time with the tap/hold key
    /// - When the holding threshold is expired(a relatively longer holding threshold should be set)
    /// - When mouse keys are triggered
    /// 
    /// Furthermore, the "tap" action can be resolved immediately in the following cases, to increase the speed:
    /// - the key is in the "key streak", similar with setting `require-prior-idle-ms` in zmk. The previous key should be non-modifier.
    /// - the next key is on the same side of the keyboard
    /// 
    /// When do we make the decision of tap/hold?
    /// - When the key is pressed("key streak", or position based tap/hold)
    /// - When the next key is releasing
    /// - When current tap/hold key is releasing
    /// - When tap/hold key is expired
    async fn process_key_action_tap_hold_hrm(
        &mut self,
        tap_action: Action,
        hold_action: Action,
        key_event: KeyEvent,
    ) {
        todo!()
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
        key_event: KeyEvent,
    ) {
        let row = key_event.row as usize;
        let col = key_event.col as usize;
        if key_event.pressed {
            self.timer[col][row] = Some(Instant::now());
            let hold_timeout = embassy_time::Timer::after_millis(200);
            match select(hold_timeout, key_event_channel.receive()).await {
                embassy_futures::select::Either::First(_) => {
                    // Timeout, trigger hold
                    debug!("Hold timeout, got HOLD: {}, {}", hold_action, key_event);
                    self.process_key_action_normal(hold_action, key_event).await;
                }
                embassy_futures::select::Either::Second(e) => {
                    if e.row == key_event.row && e.col == key_event.col {
                        // If it's same key event and releasing within 200ms, trigger tap
                        if !e.pressed {
                            let elapsed = self.timer[col][row].unwrap().elapsed().as_millis();
                            debug!("TAP action: {}, time elapsed: {}ms", tap_action, elapsed);
                            self.process_key_action_tap(tap_action, key_event).await;

                            // Clear timer
                            self.timer[col][row] = None;
                        }
                    } else {
                        // A different key comes within the threshold, trigger hold + that key

                        // Process hold action first
                        self.process_key_action_normal(hold_action, key_event).await;

                        // The actual processing is postponed because we cannot do recursion on async function without alloc
                        // After current key processing is done, we can process events in queue until the queue is empty
                        if self.unprocessed_events.push(e).is_err() {
                            warn!("unprocessed event queue is full, dropping event");
                        }
                    }
                }
            }
        } else {
            if let Some(_) = self.timer[col][row] {
                // Release hold action, then clear timer
                debug!("HOLD releasing: {}, {}", hold_action, key_event.pressed);
                self.process_key_action_normal(hold_action, key_event).await;
                self.timer[col][row] = None;
            }
        }
    }

    async fn process_key_action_oneshot(&mut self, _oneshot_action: Action) {
        warn!("oneshot action not implemented");
    }

    // Process a single keycode, typically a basic key or a modifier key.
    async fn process_action_keycode(&mut self, key: KeyCode, key_event: KeyEvent) {
        if key.is_consumer() {
            self.process_action_consumer_control(key, key_event).await;
        } else if key.is_system() {
            self.process_action_system_control(key, key_event).await;
        } else if key.is_mouse_key() {
            self.process_action_mouse(key, key_event).await;
        } else if key.is_user() {
            #[cfg(feature = "_nrf_ble")]
            use crate::ble::nrf::profile::{BleProfileAction, BLE_PROFILE_CHANNEL};
            #[cfg(feature = "_nrf_ble")]
            if !key_event.pressed {
                // Get user key id
                let id = key as u8 - KeyCode::User0 as u8;
                if id < 8 {
                    // Swtich to a specific profile
                    BLE_PROFILE_CHANNEL
                        .send(BleProfileAction::SwitchProfile(id))
                        .await;
                } else if id == 8 {
                    // Next profile
                    BLE_PROFILE_CHANNEL
                        .send(BleProfileAction::NextProfile)
                        .await;
                } else if id == 9 {
                    // Previous profile
                    BLE_PROFILE_CHANNEL
                        .send(BleProfileAction::PreviousProfile)
                        .await;
                } else if id == 10 {
                    // Clear profile
                    BLE_PROFILE_CHANNEL
                        .send(BleProfileAction::ClearProfile)
                        .await;
                }
            }
        } else if key.is_basic() {
            if key_event.pressed {
                self.register_key(key);
            } else {
                self.unregister_key(key);
            }
            self.send_keyboard_report().await;
        } else if key.is_macro() {
            // Process macro
            self.process_action_macro(key, key_event).await;
        } else {
            warn!("Unsupported key: {:?}", key);
        }
    }

    /// Process layer switch action.
    fn process_action_layer_switch(&mut self, layer_num: u8, key_event: KeyEvent) {
        // Change layer state only when the key's state is changed
        if key_event.pressed {
            self.keymap.borrow_mut().activate_layer(layer_num);
        } else {
            self.keymap.borrow_mut().deactivate_layer(layer_num);
        }
    }

    /// Process consumer control action. Consumer control keys are keys in hid consumer page, such as media keys.
    async fn process_action_consumer_control(&mut self, key: KeyCode, key_event: KeyEvent) {
        if key.is_consumer() {
            self.other_report.media_usage_id = if key_event.pressed {
                key.as_consumer_control_usage_id() as u16
            } else {
                0
            };

            self.send_media_report().await;
        }
    }

    /// Process system control action. System control keys are keys in system page, such as power key.
    async fn process_action_system_control(&mut self, key: KeyCode, key_event: KeyEvent) {
        if key.is_system() {
            if key_event.pressed {
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
    async fn process_action_mouse(&mut self, key: KeyCode, key_event: KeyEvent) {
        if key.is_mouse_key() {
            // Check whether the key is held, or it's released within the time interval
            if let Some((pressed, last_tick)) = self.last_mouse_tick.get(&key) {
                if !pressed && last_tick.elapsed().as_millis() <= 30 {
                    // The key is just released, ignore the key event, ues a slightly longer time interval
                    self.last_mouse_tick.remove(&key);
                    return;
                }
            }
            // Reference(qmk): https://github.com/qmk/qmk_firmware/blob/382c3bd0bd49fc0d53358f45477c48f5ae47f2ff/quantum/mousekey.c#L410
            // https://github.com/qmk/qmk_firmware/blob/fb598e7e617692be0bf562afaf3c852c8db1c349/quantum/action.c#L332
            if key_event.pressed {
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

            if let Err(_) = self
                .last_mouse_tick
                .insert(key, (key_event.pressed, Instant::now()))
            {
                error!("The buffer for last moust tick is full");
            }

            // Send the key event back to channel again, to keep processing the mouse key until release
            if key_event.pressed {
                // FIXME: The ideal approach is to spawn another task and send the event after 20ms.
                // But it requires embassy-executor, which is not available for esp-idf-svc.
                // So now we just block for 20ms for mouse keys.
                // In the future, we're going to use esp-hal once it have good support for BLE
                embassy_time::Timer::after_millis(20).await;
                key_event_channel.try_send(key_event).ok();
            }
        }
    }

    async fn process_action_macro(&mut self, key: KeyCode, key_event: KeyEvent) {
        // Execute the macro only when releasing the key
        if !!key_event.pressed {
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
