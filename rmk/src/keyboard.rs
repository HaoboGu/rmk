use crate::boot;
use crate::channel::{KEYBOARD_REPORT_CHANNEL, KEY_EVENT_CHANNEL};
use crate::combo::{Combo, COMBO_MAX_LENGTH};
use crate::config::BehaviorConfig;
use crate::event::KeyEvent;
use crate::hid::Report;
use crate::input_device::Runnable;
use crate::usb::descriptor::KeyboardReport;
use crate::{
    action::{Action, KeyAction},
    keyboard_macro::{MacroOperation, NUM_MACRO},
    keycode::{KeyCode, ModifierCombination},
    keymap::KeyMap,
    usb::descriptor::ViaReport,
};
use core::cell::RefCell;
use embassy_futures::{select::select, yield_now};
use embassy_time::{Instant, Timer};
use heapless::{Deque, FnvIndexMap, Vec};
use usbd_hid::descriptor::{MediaKeyboardReport, MouseReport, SystemControlReport};

/// State machine for one shot keys
#[derive(Default)]
enum OneShotState<T> {
    /// First one shot key press
    Initial(T),
    /// One shot key was released before any other key, normal one shot behavior
    Single(T),
    /// Another key was pressed before one shot key was released, treat as a normal modifier/layer
    Held(T),
    /// One shot inactive
    #[default]
    None,
}

impl<T> OneShotState<T> {
    /// Get the current one shot value if any
    pub fn value(&self) -> Option<&T> {
        match self {
            OneShotState::Initial(v) | OneShotState::Single(v) | OneShotState::Held(v) => Some(v),
            OneShotState::None => None,
        }
    }
}

impl<const ROW: usize, const COL: usize, const NUM_LAYER: usize> Runnable
    for Keyboard<'_, ROW, COL, NUM_LAYER>
{
    /// Main keyboard processing task, it receives input devices result, processes keys.
    /// The report is sent using `send_report`.
    async fn run(&mut self) {
        loop {
            let key_event = KEY_EVENT_CHANNEL.receive().await;

            // Process the key change
            self.process_inner(key_event).await;

            // After processing the key change, check if there are unprocessed events
            // This will happen if there's recursion in key processing
            loop {
                if self.unprocessed_events.is_empty() {
                    break;
                }
                // Process unprocessed events
                let e = self.unprocessed_events.remove(0);
                self.process_inner(e).await;
            }
        }
    }
}

pub struct Keyboard<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize> {
    /// Keymap
    pub(crate) keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER>>,

    /// Unprocessed events
    unprocessed_events: Vec<KeyEvent, 16>,

    /// Timer which records the timestamp of key changes
    pub(crate) timer: [[Option<Instant>; ROW]; COL],

    /// Record the timestamp of last release, (event, is_modifier, timestamp)
    last_release: (KeyEvent, bool, Option<Instant>),

    /// Record whether the keyboard is in hold-after-tap state
    hold_after_tap: [Option<KeyEvent>; 6],

    /// Options for configurable action behavior
    behavior: BehaviorConfig,

    /// One shot modifier state
    osm_state: OneShotState<ModifierCombination>,

    /// One shot layer state
    osl_state: OneShotState<u8>,

    /// Registered key position
    registered_keys: [Option<(u8, u8)>; 6],

    /// Keyboard internal hid report buf
    report: KeyboardReport,

    /// Internal mouse report buf
    mouse_report: MouseReport,

    /// Internal media report buf
    media_report: MediaKeyboardReport,

    /// Internal system control report buf
    system_control_report: SystemControlReport,

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

    /// Buffer for pressed `KeyAction` and `KeyEvents` in combos
    combo_actions_buffer: Deque<(KeyAction, KeyEvent), COMBO_MAX_LENGTH>,

    /// Used for temporarily disabling combos
    combo_on: bool,
}

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize>
    Keyboard<'a, ROW, COL, NUM_LAYER>
{
    pub fn new(
        keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER>>,
        behavior: BehaviorConfig,
    ) -> Self {
        Keyboard {
            keymap,
            timer: [[None; ROW]; COL],
            last_release: (
                KeyEvent {
                    row: 0,
                    col: 0,
                    pressed: false,
                },
                false,
                None,
            ),
            hold_after_tap: Default::default(),
            behavior,
            osm_state: OneShotState::default(),
            osl_state: OneShotState::default(),
            unprocessed_events: Vec::new(),
            registered_keys: [None; 6],
            report: KeyboardReport::default(),
            mouse_report: MouseReport {
                buttons: 0,
                x: 0,
                y: 0,
                wheel: 0,
                pan: 0,
            },
            media_report: MediaKeyboardReport { usage_id: 0 },
            system_control_report: SystemControlReport { usage_id: 0 },
            via_report: ViaReport {
                input_data: [0; 32],
                output_data: [0; 32],
            },
            last_mouse_tick: FnvIndexMap::new(),
            mouse_key_move_delta: 8,
            mouse_wheel_move_delta: 1,
            combo_actions_buffer: Deque::new(),
            combo_on: true,
        }
    }

    async fn send_report(&self, report: Report) {
        KEYBOARD_REPORT_CHANNEL.sender().send(report).await
    }

    /// Process key changes at (row, col)
    async fn process_inner(&mut self, key_event: KeyEvent) {
        // Matrix should process key pressed event first, record the timestamp of key changes
        if key_event.pressed {
            self.timer[key_event.col as usize][key_event.row as usize] = Some(Instant::now());
        }

        // Process key
        let key_action = self
            .keymap
            .borrow_mut()
            .get_action_with_layer_cache(key_event);

        if self.combo_on {
            if let Some(key_action) = self.process_combo(key_action, key_event).await {
                self.process_key_action(key_action, key_event).await;
            }
        } else {
            self.process_key_action(key_action, key_event).await;
        }
    }

    pub(crate) async fn send_keyboard_report(&mut self) {
        self.send_report(Report::KeyboardReport(self.report)).await;
        // Yield once after sending the report to channel
        yield_now().await;
    }

    /// Send system control report if needed
    pub(crate) async fn send_system_control_report(&mut self) {
        self.send_report(Report::SystemControlReport(self.system_control_report))
            .await;
        self.system_control_report.usage_id = 0;
        yield_now().await;
    }

    /// Send media report if needed
    pub(crate) async fn send_media_report(&mut self) {
        self.send_report(Report::MediaKeyboardReport(self.media_report))
            .await;
        self.media_report.usage_id = 0;
        yield_now().await;
    }

    /// Send mouse report if needed
    pub(crate) async fn send_mouse_report(&mut self) {
        // Prevent mouse report flooding, set maximum mouse report rate to 50 HZ
        self.send_report(Report::MouseReport(self.mouse_report))
            .await;
        yield_now().await;
    }

    fn update_osm(&mut self, key_event: KeyEvent) {
        match self.osm_state {
            OneShotState::Initial(m) => self.osm_state = OneShotState::Held(m),
            OneShotState::Single(_) => {
                if !key_event.pressed {
                    self.osm_state = OneShotState::None;
                }
            }
            _ => (),
        }
    }

    fn update_osl(&mut self, key_event: KeyEvent) {
        match self.osl_state {
            OneShotState::Initial(l) => self.osl_state = OneShotState::Held(l),
            OneShotState::Single(layer_num) => {
                if !key_event.pressed {
                    self.keymap.borrow_mut().deactivate_layer(layer_num);
                    self.osl_state = OneShotState::None;
                }
            }
            _ => (),
        }
    }

    async fn process_key_action(&mut self, key_action: KeyAction, key_event: KeyEvent) {
        match key_action {
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
                self.process_key_action_oneshot(oneshot_action, key_event)
                    .await
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

        // Record release of current key, which will be used in tap/hold processing
        if !key_event.pressed {
            // Check key release only
            let mut is_mod = false;
            if let KeyAction::Single(Action::Key(k)) = key_action {
                if k.is_modifier() {
                    is_mod = true;
                }
            }
            // Record the last release event
            self.last_release = (key_event, is_mod, Some(Instant::now()));
        }
    }

    async fn process_combo(
        &mut self,
        key_action: KeyAction,
        key_event: KeyEvent,
    ) -> Option<KeyAction> {
        let mut is_combo_action = false;
        let current_layer = self.keymap.borrow().get_activated_layer();
        for combo in self.keymap.borrow_mut().combos.iter_mut() {
            is_combo_action |= combo.update(key_action, key_event, current_layer);
        }

        if key_event.pressed && is_combo_action {
            if self
                .combo_actions_buffer
                .push_back((key_action, key_event))
                .is_err()
            {
                error!("Combo actions buffer overflowed! This is a bug and should not happen!");
            }

            let next_action = self
                .keymap
                .borrow_mut()
                .combos
                .iter()
                .find_map(|combo| combo.done().then_some(combo.output));

            if next_action.is_some() {
                self.combo_actions_buffer.clear();
            } else {
                let timeout =
                    embassy_time::Timer::after(self.keymap.borrow().behavior.combo.timeout);
                match select(timeout, KEY_EVENT_CHANNEL.receive()).await {
                    embassy_futures::select::Either::First(_) => self.dispatch_combos().await,
                    embassy_futures::select::Either::Second(event) => {
                        self.unprocessed_events.push(event).unwrap()
                    }
                }
            }
            next_action
        } else {
            if !key_event.pressed {
                for combo in self.keymap.borrow_mut().combos.iter_mut() {
                    if combo.done() && combo.actions.contains(&key_action) {
                        combo.reset();
                        return Some(combo.output);
                    }
                }
            }

            self.dispatch_combos().await;
            Some(key_action)
        }
    }

    async fn dispatch_combos(&mut self) {
        while let Some((action, event)) = self.combo_actions_buffer.pop_front() {
            self.process_key_action(action, event).await;
        }

        self.keymap
            .borrow_mut()
            .combos
            .iter_mut()
            .filter(|combo| !combo.done())
            .for_each(Combo::reset);
    }

    async fn process_key_action_normal(&mut self, action: Action, key_event: KeyEvent) {
        match action {
            Action::Key(key) => {
                self.process_action_keycode(key, key_event).await;
                self.update_osm(key_event);
                self.update_osl(key_event);
            }
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
            Action::Modifier(modifiers) => {
                if key_event.pressed {
                    self.register_modifiers(modifiers);
                } else {
                    self.unregister_modifiers(modifiers);
                }
                //report the modifier press/release in its own hid report
                self.send_keyboard_report().await;
                self.update_osl(key_event);
            }
        }
    }

    async fn process_key_action_with_modifier(
        &mut self,
        action: Action,
        modifiers: ModifierCombination,
        key_event: KeyEvent,
    ) {
        if key_event.pressed {
            // The modifiers are prepared in the hid report, so will be pressed same time (same hid report) as the key
            self.register_modifiers(modifiers);
        } else {
            // The modifiers are removed from the prepared hid report, so will be released same time (same hid report) as the key
            self.unregister_modifiers(modifiers);
        }
        self.process_key_action_normal(action, key_event).await;
    }

    /// Tap action, send a key when the key is pressed, then release the key.
    async fn process_key_action_tap(&mut self, action: Action, mut key_event: KeyEvent) {
        if key_event.pressed {
            self.process_key_action_normal(action, key_event).await;

            // Wait 10ms, then send release
            Timer::after_millis(10).await;

            key_event.pressed = false;
            self.process_key_action_normal(action, key_event).await;

            // Record the release event
            let mut is_mod = false;
            if let Action::Key(k) = action {
                if k.is_modifier() {
                    is_mod = true;
                }
            }
            self.last_release = (key_event, is_mod, Some(Instant::now()));
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
    async fn process_key_action_tap_hold(
        &mut self,
        tap_action: Action,
        hold_action: Action,
        key_event: KeyEvent,
    ) {
        if self.keymap.borrow().behavior.tap_hold.enable_hrm {
            // If HRM is enabled, check whether it's a different key is in key streak
            if let Some(last_release_time) = self.last_release.2 {
                if key_event.pressed {
                    if last_release_time.elapsed()
                        < self.keymap.borrow().behavior.tap_hold.prior_idle_time
                        && !(key_event.row == self.last_release.0.row
                            && key_event.col == self.last_release.0.col)
                    {
                        // The previous key is a different key and released within `prior_idle_time`, it's in key streak
                        debug!("Key streak detected, trigger tap action");
                        self.process_key_action_tap(tap_action, key_event).await;
                        return;
                    } else if last_release_time.elapsed()
                        < self.keymap.borrow().behavior.tap_hold.hold_timeout
                        && key_event.row == self.last_release.0.row
                        && key_event.col == self.last_release.0.col
                    {
                        // Pressed a same key after tapped it within `hold_timeout`
                        // Trigger the tap action just as it's pressed
                        self.process_key_action_normal(tap_action, key_event).await;
                        if let Some(index) = self.hold_after_tap.iter().position(|&k| k.is_none()) {
                            self.hold_after_tap[index] = Some(key_event);
                        }
                        return;
                    }
                }
            }
        }

        let row = key_event.row as usize;
        let col = key_event.col as usize;
        if key_event.pressed {
            // Press
            self.timer[col][row] = Some(Instant::now());

            let hold_timeout = embassy_time::Timer::after_millis(
                self.keymap
                    .borrow()
                    .behavior
                    .tap_hold
                    .hold_timeout
                    .as_millis(),
            );
            match select(hold_timeout, KEY_EVENT_CHANNEL.receive()).await {
                embassy_futures::select::Either::First(_) => {
                    // Timeout, trigger hold
                    debug!("Hold timeout, got HOLD: {:?}, {:?}", hold_action, key_event);
                    self.process_key_action_normal(hold_action, key_event).await;
                }
                embassy_futures::select::Either::Second(e) => {
                    if e.row == key_event.row && e.col == key_event.col {
                        // If it's same key event and releasing within `hold_timeout`, trigger tap
                        if !e.pressed {
                            let elapsed = self.timer[col][row].unwrap().elapsed().as_millis();
                            debug!("TAP action: {:?}, time elapsed: {}ms", tap_action, elapsed);
                            self.process_key_action_tap(tap_action, key_event).await;

                            // Clear timer
                            self.timer[col][row] = None;
                        }
                    } else {
                        // A different key comes
                        // If it's a release event, the key is pressed BEFORE tap/hold key, so it should be regarded as a normal key
                        self.unprocessed_events.push(e).ok();
                        if !e.pressed {
                            // we push the current tap/hold event again, the loop will process the release first, then re-process current tap/hold
                            self.unprocessed_events.push(key_event).ok();
                            return;
                        }

                        // Wait for key release, record all pressed keys during this
                        loop {
                            let next_key_event = KEY_EVENT_CHANNEL.receive().await;
                            self.unprocessed_events.push(next_key_event).ok();
                            if !next_key_event.pressed {
                                break;
                            }
                        }

                        // Process hold action
                        self.process_key_action_normal(hold_action, key_event).await;

                        // All other unprocessed events will be processed later
                    }
                }
            }
        } else {
            // Release

            // find holding_after_tap key_event
            if let Some(index) = self.hold_after_tap.iter().position(|&k| {
                if let Some(ke) = k {
                    return ke.row == key_event.row && ke.col == key_event.col;
                }
                false
            }) {
                // Release the hold after tap key
                info!("Releasing hold after tap: {:?} {:?}", tap_action, key_event);
                self.process_key_action_normal(tap_action, key_event).await;
                self.hold_after_tap[index] = None;
                return;
            }
            if self.timer[col][row].is_some() {
                // Release hold action, wait for `post_wait_time`, then clear timer
                debug!(
                    "HOLD releasing: {:?}, {}, wait for `post_wait_time` for new releases",
                    hold_action, key_event.pressed
                );
                let wait_release = async {
                    loop {
                        let next_key_event = KEY_EVENT_CHANNEL.receive().await;
                        if !next_key_event.pressed {
                            self.unprocessed_events.push(next_key_event).ok();
                        } else {
                            break next_key_event;
                        }
                    }
                };

                let wait_timeout = embassy_time::Timer::after_millis(
                    self.keymap
                        .borrow()
                        .behavior
                        .tap_hold
                        .post_wait_time
                        .as_millis(),
                );
                match select(wait_timeout, wait_release).await {
                    embassy_futures::select::Either::First(_) => {
                        // Wait timeout, release the hold key finally
                        self.process_key_action_normal(hold_action, key_event).await;
                    }
                    embassy_futures::select::Either::Second(next_press) => {
                        // Next press event comes, add hold release to unprocessed list first, then add next press
                        self.unprocessed_events.push(key_event).ok();
                        self.unprocessed_events.push(next_press).ok();
                    }
                };
                // Clear timer
                self.timer[col][row] = None;
            } else {
                // The timer has been reset, fire hold release event
                debug!("HOLD releasing: {:?}, {}", hold_action, key_event.pressed);
                self.process_key_action_normal(hold_action, key_event).await;
            }
        }
    }

    /// Process one shot action.
    async fn process_key_action_oneshot(&mut self, oneshot_action: Action, key_event: KeyEvent) {
        match oneshot_action {
            Action::Modifier(m) => self.process_action_osm(m, key_event).await,
            Action::LayerOn(l) => self.process_action_osl(l, key_event).await,
            _ => {
                self.process_key_action_normal(oneshot_action, key_event)
                    .await
            }
        }
    }

    async fn process_action_osm(&mut self, modifiers: ModifierCombination, key_event: KeyEvent) {
        // Update one shot state
        if key_event.pressed {
            // Add new modifier combination to existing one shot or init if none
            self.osm_state = match self.osm_state {
                OneShotState::None => OneShotState::Initial(modifiers),
                OneShotState::Initial(m) => OneShotState::Initial(m | modifiers),
                OneShotState::Single(m) => OneShotState::Single(m | modifiers),
                OneShotState::Held(m) => OneShotState::Held(m | modifiers),
            };

            self.update_osl(key_event);
        } else {
            match self.osm_state {
                OneShotState::Initial(m) | OneShotState::Single(m) => {
                    self.osm_state = OneShotState::Single(m);

                    let timeout =
                        embassy_time::Timer::after(self.keymap.borrow().behavior.one_shot.timeout);
                    match select(timeout, KEY_EVENT_CHANNEL.receive()).await {
                        embassy_futures::select::Either::First(_) => {
                            // Timeout, release modifiers
                            self.update_osl(key_event);
                            self.osm_state = OneShotState::None;
                        }
                        embassy_futures::select::Either::Second(e) => {
                            // New event, send it to queue
                            if self.unprocessed_events.push(e).is_err() {
                                warn!("unprocessed event queue is full, dropping event");
                            }
                        }
                    }
                }
                OneShotState::Held(_) => {
                    // Release modifier
                    self.update_osl(key_event);
                    self.osm_state = OneShotState::None;

                    // This sends a separate hid report with the
                    // currently registered modifiers except the
                    // one shoot modifies this way "releasing" them.
                    self.send_keyboard_report().await;
                }
                _ => (),
            };
        }
    }

    async fn process_action_osl(&mut self, layer_num: u8, key_event: KeyEvent) {
        // Update one shot state
        if key_event.pressed {
            // Deactivate old layer if any
            if let Some(&l) = self.osl_state.value() {
                self.keymap.borrow_mut().deactivate_layer(l);
            }

            // Update layer of one shot
            self.osl_state = match self.osl_state {
                OneShotState::None => OneShotState::Initial(layer_num),
                OneShotState::Initial(_) => OneShotState::Initial(layer_num),
                OneShotState::Single(_) => OneShotState::Single(layer_num),
                OneShotState::Held(_) => OneShotState::Held(layer_num),
            };

            // Activate new layer
            self.keymap.borrow_mut().activate_layer(layer_num);
        } else {
            match self.osl_state {
                OneShotState::Initial(l) | OneShotState::Single(l) => {
                    self.osl_state = OneShotState::Single(l);

                    let timeout =
                        embassy_time::Timer::after(self.keymap.borrow().behavior.one_shot.timeout);
                    match select(timeout, KEY_EVENT_CHANNEL.receive()).await {
                        embassy_futures::select::Either::First(_) => {
                            // Timeout, deactivate layer
                            self.keymap.borrow_mut().deactivate_layer(layer_num);
                            self.osl_state = OneShotState::None;
                        }
                        embassy_futures::select::Either::Second(e) => {
                            // New event, send it to queue
                            if self.unprocessed_events.push(e).is_err() {
                                warn!("unprocessed event queue is full, dropping event");
                            }
                        }
                    }
                }
                OneShotState::Held(layer_num) => {
                    self.osl_state = OneShotState::None;
                    self.keymap.borrow_mut().deactivate_layer(layer_num);
                }
                _ => (),
            };
        }
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
            use {crate::ble::nrf::profile::BleProfileAction, crate::channel::BLE_PROFILE_CHANNEL};
            #[cfg(feature = "_nrf_ble")]
            if !key_event.pressed {
                // Get user key id
                let id = key as u8 - KeyCode::User0 as u8;
                if id < 8 {
                    info!("Switch to profile: {}", id);
                    // User0~7: Swtich to the specific profile
                    BLE_PROFILE_CHANNEL
                        .send(BleProfileAction::SwitchProfile(id))
                        .await;
                } else if id == 8 {
                    // User8: Next profile
                    BLE_PROFILE_CHANNEL
                        .send(BleProfileAction::NextProfile)
                        .await;
                } else if id == 9 {
                    // User9: Previous profile
                    BLE_PROFILE_CHANNEL
                        .send(BleProfileAction::PreviousProfile)
                        .await;
                } else if id == 10 {
                    // User10: Clear profile
                    BLE_PROFILE_CHANNEL
                        .send(BleProfileAction::ClearProfile)
                        .await;
                } else if id == 11 {
                    // User11:
                    BLE_PROFILE_CHANNEL
                        .send(BleProfileAction::ToggleConnection)
                        .await;
                }
            }
        } else if key.is_basic() {
            self.process_basic(key, key_event).await;
        } else if key.is_macro() {
            // Process macro
            self.process_action_macro(key, key_event).await;
        } else if key.is_combo() {
            self.process_action_combo(key, key_event).await;
        } else if key.is_boot() {
            self.process_boot(key, key_event);
        } else {
            warn!("Unsupported key: {:?}", key);
        }
    }

    // precess a basic keypress/release and also take care of applying one shot modifiers
    async fn process_basic(&mut self, key: KeyCode, key_event: KeyEvent) {
        if key_event.pressed {
            self.register_key(key, key_event);
            //if one shot modifier is active, decorate the hid report of keypress with those modifiers
            if let Some(modifiers) = self.osm_state.value() {
                let old = self.report.modifier;
                self.report.modifier |= modifiers.to_hid_modifier_bits();
                self.send_keyboard_report().await;
                self.report.modifier = old;
            } else {
                self.send_keyboard_report().await;
            }
        } else {
            // One shot modifiers are "released" together key release,
            // except when in one shoot is in "held mode" (to allow Alt+Tab like use cases)
            // In that later case Held -> None state change will report
            // the "modifier released" change in a separate hid report
            self.unregister_key(key, key_event);
            if let OneShotState::Held(modifiers) = self.osm_state {
                // OneShotState::Held keeps the temporary modifiers active
                let old = self.report.modifier;
                self.report.modifier |= modifiers.to_hid_modifier_bits();
                self.send_keyboard_report().await;
                self.report.modifier = old;
            } else {
                self.send_keyboard_report().await;
            }
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

    /// Process combo action.
    async fn process_action_combo(&mut self, key: KeyCode, key_event: KeyEvent) {
        if key_event.pressed {
            match key {
                KeyCode::ComboOn => self.combo_on = true,
                KeyCode::ComboOff => self.combo_on = false,
                KeyCode::ComboToggle => self.combo_on = !self.combo_on,
                _ => (),
            }
        }
    }

    /// Process consumer control action. Consumer control keys are keys in hid consumer page, such as media keys.
    async fn process_action_consumer_control(&mut self, key: KeyCode, key_event: KeyEvent) {
        if key.is_consumer() {
            self.media_report.usage_id = if key_event.pressed {
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
                    self.system_control_report.usage_id = system_key as u8;
                    self.send_system_control_report().await;
                }
            } else {
                self.system_control_report.usage_id = 0;
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
                    // TODO: Add accelerated mode when pressing the mouse key
                    // https://github.com/qmk/qmk_firmware/blob/master/docs/feature_mouse_keys.md#accelerated-mode
                    KeyCode::MouseUp => {
                        self.mouse_report.y = -self.mouse_key_move_delta;
                    }
                    KeyCode::MouseDown => {
                        self.mouse_report.y = self.mouse_key_move_delta;
                    }
                    KeyCode::MouseLeft => {
                        self.mouse_report.x = -self.mouse_key_move_delta;
                    }
                    KeyCode::MouseRight => {
                        self.mouse_report.x = self.mouse_key_move_delta;
                    }
                    KeyCode::MouseWheelUp => {
                        self.mouse_report.wheel = self.mouse_wheel_move_delta;
                    }
                    KeyCode::MouseWheelDown => {
                        self.mouse_report.wheel = -self.mouse_wheel_move_delta;
                    }
                    KeyCode::MouseBtn1 => self.mouse_report.buttons |= 1 << 0,
                    KeyCode::MouseBtn2 => self.mouse_report.buttons |= 1 << 1,
                    KeyCode::MouseBtn3 => self.mouse_report.buttons |= 1 << 2,
                    KeyCode::MouseBtn4 => self.mouse_report.buttons |= 1 << 3,
                    KeyCode::MouseBtn5 => self.mouse_report.buttons |= 1 << 4,
                    KeyCode::MouseBtn6 => self.mouse_report.buttons |= 1 << 5,
                    KeyCode::MouseBtn7 => self.mouse_report.buttons |= 1 << 6,
                    KeyCode::MouseBtn8 => self.mouse_report.buttons |= 1 << 7,
                    KeyCode::MouseWheelLeft => {
                        self.mouse_report.pan = -self.mouse_wheel_move_delta;
                    }
                    KeyCode::MouseWheelRight => {
                        self.mouse_report.pan = self.mouse_wheel_move_delta;
                    }
                    KeyCode::MouseAccel0 => {}
                    KeyCode::MouseAccel1 => {}
                    KeyCode::MouseAccel2 => {}
                    _ => {}
                }
            } else {
                match key {
                    KeyCode::MouseUp | KeyCode::MouseDown => {
                        self.mouse_report.y = 0;
                    }
                    KeyCode::MouseLeft | KeyCode::MouseRight => {
                        self.mouse_report.x = 0;
                    }
                    KeyCode::MouseWheelUp | KeyCode::MouseWheelDown => {
                        self.mouse_report.wheel = 0;
                    }
                    KeyCode::MouseWheelLeft | KeyCode::MouseWheelRight => {
                        self.mouse_report.pan = 0;
                    }
                    KeyCode::MouseBtn1 => self.mouse_report.buttons &= !(1 << 0),
                    KeyCode::MouseBtn2 => self.mouse_report.buttons &= !(1 << 1),
                    KeyCode::MouseBtn3 => self.mouse_report.buttons &= !(1 << 2),
                    KeyCode::MouseBtn4 => self.mouse_report.buttons &= !(1 << 3),
                    KeyCode::MouseBtn5 => self.mouse_report.buttons &= !(1 << 4),
                    KeyCode::MouseBtn6 => self.mouse_report.buttons &= !(1 << 5),
                    KeyCode::MouseBtn7 => self.mouse_report.buttons &= !(1 << 6),
                    KeyCode::MouseBtn8 => self.mouse_report.buttons &= !(1 << 7),
                    _ => {}
                }
            }
            self.send_mouse_report().await;

            if self
                .last_mouse_tick
                .insert(key, (key_event.pressed, Instant::now()))
                .is_err()
            {
                error!("The buffer for last mouse tick is full");
            }

            // Send the key event back to channel again, to keep processing the mouse key until release
            if key_event.pressed {
                // FIXME: The ideal approach is to spawn another task and send the event after 20ms.
                // But it requires embassy-executor, which is not available for esp-idf-svc.
                // So now we just block for 20ms for mouse keys.
                // In the future, we're going to use esp-hal once it have good support for BLE
                embassy_time::Timer::after_millis(20).await;
                KEY_EVENT_CHANNEL.try_send(key_event).ok();
            }
        }
    }

    fn process_boot(&mut self, key: KeyCode, key_event: KeyEvent) {
        if key_event.pressed {
            match key {
                KeyCode::Bootloader => {
                    boot::jump_to_bootloader();
                }
                KeyCode::Reboot => {
                    boot::reboot_keyboard();
                }
                _ => (), // unreachable, do nothing
            };
        }
    }

    async fn process_action_macro(&mut self, key: KeyCode, key_event: KeyEvent) {
        // Execute the macro only when releasing the key
        if key_event.pressed {
            return;
        }

        // Get macro index
        if let Some(macro_idx) = key.as_macro_index() {
            if macro_idx as usize >= NUM_MACRO {
                error!("Macro idx invalid: {}", macro_idx);
                return;
            }
            // Read macro operations until the end of the macro
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
                            self.register_key(k, key_event);
                        }
                        MacroOperation::Release(k) => {
                            self.unregister_key(k, key_event);
                        }
                        MacroOperation::Tap(k) => {
                            self.register_key(k, key_event);
                            self.send_keyboard_report().await;
                            embassy_time::Timer::after_millis(2).await;
                            self.unregister_key(k, key_event);
                        }
                        MacroOperation::Text(k, is_cap) => {
                            if is_cap {
                                // If it's a capital letter, send the pressed report with a shift modifier included
                                self.register_modifier_key(KeyCode::LShift);
                            }
                            self.register_keycode(k, key_event);
                            self.send_keyboard_report().await;
                            embassy_time::Timer::after_millis(2).await;
                            self.unregister_keycode(k, key_event);
                            if is_cap {
                                // If it was a capital letter, send the release report with the shift modifier released too
                                self.unregister_modifier_key(KeyCode::LShift);
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
    fn register_key(&mut self, key: KeyCode, key_event: KeyEvent) {
        if key.is_modifier() {
            self.register_modifier_key(key);
        } else if key.is_basic() {
            self.register_keycode(key, key_event);
        }
    }

    /// Unregister a key, the key can be a basic keycode or a modifier.
    fn unregister_key(&mut self, key: KeyCode, key_event: KeyEvent) {
        if key.is_modifier() {
            self.unregister_modifier_key(key);
        } else if key.is_basic() {
            self.unregister_keycode(key, key_event);
        }
    }

    /// Register a key to be sent in hid report.
    fn register_keycode(&mut self, key: KeyCode, key_event: KeyEvent) {
        // First, find the key event slot according to the position
        let slot = self.registered_keys.iter().enumerate().find_map(|(i, k)| {
            if let Some((row, col)) = k {
                if key_event.row == *row && key_event.col == *col {
                    return Some(i);
                }
            }
            None
        });

        // If the slot is found, update the key in the slot
        if let Some(index) = slot {
            self.report.keycodes[index] = key as u8;
            self.registered_keys[index] = Some((key_event.row, key_event.col));
        } else {
            // Otherwise, find the first free slot
            if let Some(index) = self.report.keycodes.iter().position(|&k| k == 0) {
                self.report.keycodes[index] = key as u8;
                self.registered_keys[index] = Some((key_event.row, key_event.col));
            }
        }
    }

    /// Unregister a key from hid report.
    fn unregister_keycode(&mut self, key: KeyCode, key_event: KeyEvent) {
        // First, find the key event slot according to the position
        let slot = self.registered_keys.iter().enumerate().find_map(|(i, k)| {
            if let Some((row, col)) = k {
                if key_event.row == *row && key_event.col == *col {
                    return Some(i);
                }
            }
            None
        });

        // If the slot is found, update the key in the slot
        if let Some(index) = slot {
            self.report.keycodes[index] = 0;
            self.registered_keys[index] = None;
        } else {
            // Otherwise, release the first same key
            if let Some(index) = self.report.keycodes.iter().position(|&k| k == key as u8) {
                self.report.keycodes[index] = 0;
                self.registered_keys[index] = None;
            }
        }
    }

    /// Register a modifier to be sent in hid report.
    fn register_modifier_key(&mut self, key: KeyCode) {
        self.report.modifier |= key.to_hid_modifier_bit();
    }

    /// Unregister a modifier from hid report.
    fn unregister_modifier_key(&mut self, key: KeyCode) {
        self.report.modifier &= !{ key.to_hid_modifier_bit() };
    }

    /// Register a modifier combination to be sent in hid report.
    fn register_modifiers(&mut self, modifiers: ModifierCombination) {
        self.report.modifier |= modifiers.to_hid_modifier_bits();
    }

    /// Unregister a modifier combination from hid report.
    fn unregister_modifiers(&mut self, modifiers: ModifierCombination) {
        self.report.modifier &= !modifiers.to_hid_modifier_bits();
    }
}
