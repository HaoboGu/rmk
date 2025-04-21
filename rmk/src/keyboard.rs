use core::cell::RefCell;

use embassy_futures::select::{select, Either};
use embassy_futures::yield_now;
use embassy_time::{Instant, Timer};
use heapless::{Deque, FnvIndexMap, Vec};
use usbd_hid::descriptor::{MediaKeyboardReport, MouseReport, SystemControlReport};

use crate::action::{Action, KeyAction};
use crate::boot;
use crate::channel::{KEYBOARD_REPORT_CHANNEL, KEY_EVENT_CHANNEL};
use crate::combo::{Combo, COMBO_MAX_LENGTH};
use crate::config::BehaviorConfig;
use crate::event::KeyEvent;
use crate::fork::{ActiveFork, StateBits, FORK_MAX_NUM};
use crate::hid::Report;
use crate::hid_state::{HidModifiers, HidMouseButtons};
use crate::input_device::Runnable;
use crate::keyboard_macro::{MacroOperation, NUM_MACRO};
use crate::keycode::{KeyCode, ModifierCombination};
use crate::keymap::KeyMap;
use crate::light::LedIndicator;
use crate::usb::descriptor::{KeyboardReport, ViaReport};

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

impl<const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize> Runnable
    for Keyboard<'_, ROW, COL, NUM_LAYER, NUM_ENCODER>
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

/// led states for the keyboard hid report (its value is received by by the light service in a hid report)
/// LedIndicator type would be nicer, but that does not have const expr constructor
pub(crate) static LOCK_LED_STATES: core::sync::atomic::AtomicU8 = core::sync::atomic::AtomicU8::new(0u8);

pub struct Keyboard<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize = 0> {
    /// Keymap
    pub(crate) keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,

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

    /// One shot layer state
    osl_state: OneShotState<u8>,

    /// One shot modifier state
    osm_state: OneShotState<HidModifiers>,

    /// The modifiers coming from (last) KeyAction::WithModifier  
    with_modifiers: HidModifiers,

    /// Macro text typing state (affects the effective modifiers)
    macro_texting: bool,
    macro_caps: bool,

    /// The real state before fork activations is stored here
    fork_states: [Option<ActiveFork>; FORK_MAX_NUM], // chosen replacement key of the currently triggered forks and the related modifier suppression
    fork_keep_mask: HidModifiers, // aggregate here the explicit modifiers pressed since the last fork activations

    /// the held keys for the keyboard hid report, except the modifiers
    held_modifiers: HidModifiers,

    /// the held keys for the keyboard hid report, except the modifiers
    held_keycodes: [KeyCode; 6],

    /// Registered key position
    registered_keys: [Option<(u8, u8)>; 6],

    /// Internal mouse report buf
    mouse_report: MouseReport,

    /// Internal media report buf
    media_report: MediaKeyboardReport,

    /// Internal system control report buf
    system_control_report: SystemControlReport,

    /// Via report
    via_report: ViaReport,

    /// stores the last KeyCode executed, to be repeated if the repeat key os pressed
    last_key_code: KeyCode,

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

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    Keyboard<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    pub fn new(keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>, behavior: BehaviorConfig) -> Self {
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
            osl_state: OneShotState::default(),
            osm_state: OneShotState::default(),
            with_modifiers: HidModifiers::default(),
            macro_texting: false,
            macro_caps: false,
            fork_states: [None; FORK_MAX_NUM],
            fork_keep_mask: HidModifiers::default(),
            unprocessed_events: Vec::new(),
            registered_keys: [None; 6],
            held_modifiers: HidModifiers::default(),
            held_keycodes: [KeyCode::No; 6],
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
            last_key_code: KeyCode::No,
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
        let key_action = self.keymap.borrow_mut().get_action_with_layer_cache(key_event);

        if self.combo_on {
            if let Some(key_action) = self.process_combo(key_action, key_event).await {
                debug!("Process key action after combo: {:?}, {:?}", key_action, key_event);
                self.process_key_action(key_action, key_event).await;
            }
        } else {
            self.process_key_action(key_action, key_event).await;
        }
    }

    pub(crate) async fn send_keyboard_report_with_resolved_modifiers(&mut self, pressed: bool) {
        // all modifier related effects are combined here to be sent with the hid report:
        let modifiers = self.resolve_modifiers(pressed).into_bits();

        self.send_report(Report::KeyboardReport(KeyboardReport {
            modifier: modifiers,
            reserved: 0,
            leds: LOCK_LED_STATES.load(core::sync::atomic::Ordering::Relaxed),
            keycodes: self.held_keycodes.map(|k| k as u8),
        }))
        .await;

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
        self.send_report(Report::MediaKeyboardReport(self.media_report)).await;
        self.media_report.usage_id = 0;
        yield_now().await;
    }

    /// Send mouse report if needed
    pub(crate) async fn send_mouse_report(&mut self) {
        // Prevent mouse report flooding, set maximum mouse report rate to 50 HZ
        self.send_report(Report::MouseReport(self.mouse_report)).await;
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

    async fn process_key_action(&mut self, original_key_action: KeyAction, key_event: KeyEvent) {
        let key_action = self.try_start_forks(original_key_action, key_event);

        match key_action {
            KeyAction::No | KeyAction::Transparent => (),
            KeyAction::Single(a) => self.process_key_action_normal(a, key_event).await,
            KeyAction::WithModifier(a, m) => self.process_key_action_with_modifier(a, m, key_event).await,
            KeyAction::Tap(a) => self.process_key_action_tap(a, key_event).await,
            KeyAction::TapHold(tap_action, hold_action) => {
                self.process_key_action_tap_hold(tap_action, hold_action, key_event)
                    .await;
            }
            KeyAction::OneShot(oneshot_action) => self.process_key_action_oneshot(oneshot_action, key_event).await,
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

        if !key_event.pressed {
            // Record release of current key, which will be used in tap/hold processing
            let mut is_mod = false;
            if let KeyAction::Single(Action::Key(k)) = key_action {
                if k.is_modifier() {
                    is_mod = true;
                }
            }
            // Record the last release event
            self.last_release = (key_event, is_mod, Some(Instant::now()));
        }

        self.try_finish_forks(original_key_action, key_event);
    }

    /// Replaces the incoming key_action if a fork is configured for that key.
    /// The replacement decision is made at key_press time, and the decision
    /// is kept until the key is released.
    fn try_start_forks(&mut self, key_action: KeyAction, key_event: KeyEvent) -> KeyAction {
        if self.keymap.borrow().behavior.fork.forks.is_empty() {
            return key_action;
        }

        if !key_event.pressed {
            for (i, fork) in (&self.keymap.borrow().behavior.fork.forks).into_iter().enumerate() {
                if fork.trigger == key_action {
                    if let Some(active) = self.fork_states[i] {
                        // If the originating key of a fork is released, simply release the replacement key
                        // (The fork deactivation is delayed, will happen after the release hid report is sent)
                        return active.replacement;
                    }
                }
            }
            return key_action;
        }

        let mut decision_state = StateBits {
            // "explicit modifiers" includes the effect of one-shot modifiers, held modifiers keys only
            modifiers: self.resolve_explicit_modifiers(key_event.pressed),
            leds: LedIndicator::from_bits(LOCK_LED_STATES.load(core::sync::atomic::Ordering::Relaxed)),
            mouse: HidMouseButtons::from_bits(self.mouse_report.buttons),
        };

        let mut triggered_forks = [false; FORK_MAX_NUM]; // used to avoid loops
        let mut chain_starter: Option<usize> = None;
        let mut combined_suppress = HidModifiers::default();
        let mut replacement = key_action;

        'bind: loop {
            for (i, fork) in (&self.keymap.borrow().behavior.fork.forks).into_iter().enumerate() {
                if !triggered_forks[i] && self.fork_states[i].is_none() && fork.trigger == replacement {
                    let decision = (fork.match_any & decision_state) != StateBits::default()
                        && (fork.match_none & decision_state) == StateBits::default();

                    replacement = if decision {
                        fork.positive_output
                    } else {
                        fork.negative_output
                    };

                    let suppress = fork.match_any.modifiers & !fork.kept_modifiers;

                    combined_suppress |= suppress;

                    // Suppress the previously activated KeyAction::WithModifiers
                    // (even if they held for a long time, a new keypress arrived
                    // since then, which breaks the key repeat, so losing their
                    // effect likely will not cause problem...)
                    self.with_modifiers &= !suppress;

                    // Reduce the previously aggregated keeps with the match_any mask
                    // (since this is the expected behavior in most cases)
                    self.fork_keep_mask &= !fork.match_any.modifiers;

                    // Then add the user defined keeps (if any)
                    self.fork_keep_mask |= fork.kept_modifiers;

                    if chain_starter.is_none() {
                        chain_starter = Some(i);
                    }

                    if fork.bindable {
                        // If this fork is bindable look for other not yet activated forks,
                        // which can be triggered by that the current replacement key
                        triggered_forks[i] = true; // Avoid triggering the same fork again -> no infinite loops either

                        // For the next fork evaluations, update the decision state
                        // with the suppressed modifiers
                        decision_state.modifiers &= !suppress;
                        continue 'bind;
                    }

                    //return final decision is ready
                    break 'bind;
                }
            }

            // No (more) forks were triggered, so we are done
            break 'bind;
        }

        if let Some(initial) = chain_starter {
            // After the initial fork triggered, we have switched to "bind mode".
            // The later triggered forks will not really activate, only update
            // the replacement decision and modifier suppressions of the initially
            // triggered fork, which is here marked as active:
            self.fork_states[initial] = Some(ActiveFork {
                replacement,
                suppress: combined_suppress,
            });
        }

        // No (or no more) forks were triggered, so we are done
        replacement
    }

    // Release of forked key must deactivate the fork
    // (explicit modifier suppressing effect will be stopped only AFTER the release hid report is sent)
    fn try_finish_forks(&mut self, original_key_action: KeyAction, key_event: KeyEvent) {
        if !key_event.pressed {
            for (i, fork) in (&self.keymap.borrow().behavior.fork.forks).into_iter().enumerate() {
                if self.fork_states[i].is_some() && fork.trigger == original_key_action {
                    // if the originating key of a fork is released the replacement decision is not valid anymore
                    self.fork_states[i] = None;
                }
            }
        }
    }

    async fn process_combo(&mut self, key_action: KeyAction, key_event: KeyEvent) -> Option<KeyAction> {
        let mut is_combo_action = false;
        let current_layer = self.keymap.borrow().get_activated_layer();
        for combo in self.keymap.borrow_mut().behavior.combo.combos.iter_mut() {
            is_combo_action |= combo.update(key_action, key_event, current_layer);
        }

        if key_event.pressed && is_combo_action {
            if self.combo_actions_buffer.push_back((key_action, key_event)).is_err() {
                error!("Combo actions buffer overflowed! This is a bug and should not happen!");
            }

            //FIXME last combo is not checked
            let next_action = self
                .keymap
                .borrow_mut()
                .behavior
                .combo
                .combos
                .iter_mut()
                .find_map(|combo| (combo.is_all_pressed() && !combo.is_triggered()).then_some(combo.trigger()));

            if next_action.is_some() {
                self.combo_actions_buffer.clear();
                debug!("Combo action {:?} matched:: clearing combo buffer", next_action);
            } else {
                let timeout = embassy_time::Timer::after(self.keymap.borrow().behavior.combo.timeout);
                match select(timeout, KEY_EVENT_CHANNEL.receive()).await {
                    Either::First(_) => self.dispatch_combos().await,
                    Either::Second(event) => self.unprocessed_events.push(event).unwrap(),
                }
            }
            next_action
        } else {
            if !key_event.pressed {
                for combo in self.keymap.borrow_mut().behavior.combo.combos.iter_mut() {
                    if combo.is_triggered() && combo.actions.contains(&key_action) {
                        combo.reset();
                        return Some(combo.output);
                    }
                }
            }

            self.dispatch_combos().await;
            Some(key_action)
        }
    }

    // Dispatch combo into key action
    async fn dispatch_combos(&mut self) {
        while let Some((action, event)) = self.combo_actions_buffer.pop_front() {
            debug!("Dispatching combo action: {:?}", action);
            self.process_key_action(action, event).await;
        }

        self.keymap
            .borrow_mut()
            .behavior
            .combo
            .combos
            .iter_mut()
            .filter(|combo| !combo.is_triggered())
            .for_each(Combo::reset);
    }

    async fn process_key_action_normal(&mut self, action: Action, key_event: KeyEvent) {
        match action {
            Action::Key(key) => self.process_action_key(key, key_event).await,
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
                self.send_keyboard_report_with_resolved_modifiers(key_event.pressed)
                    .await;
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
            // These modifiers will be combined into the hid report, so
            // they will be "pressed" the same time as the key (in same hid report)
            self.with_modifiers |= modifiers.to_hid_modifiers();
        } else {
            // The modifiers will not be part of the hid report, so
            // they will be "released" the same time as the key (in same hid report)
            self.with_modifiers &= !(modifiers.to_hid_modifiers());
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
    async fn process_key_action_tap_hold(&mut self, tap_action: Action, hold_action: Action, key_event: KeyEvent) {
        if self.keymap.borrow().behavior.tap_hold.enable_hrm {
            // If HRM is enabled, check whether it's a different key is in key streak
            if let Some(last_release_time) = self.last_release.2 {
                if key_event.pressed {
                    if last_release_time.elapsed() < self.keymap.borrow().behavior.tap_hold.prior_idle_time
                        && !(key_event.row == self.last_release.0.row && key_event.col == self.last_release.0.col)
                    {
                        // The previous key is a different key and released within `prior_idle_time`, it's in key streak
                        debug!("Key streak detected, trigger tap action");
                        self.process_key_action_tap(tap_action, key_event).await;
                        return;
                    } else if last_release_time.elapsed() < self.keymap.borrow().behavior.tap_hold.hold_timeout
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

            let hold_timeout =
                embassy_time::Timer::after_millis(self.keymap.borrow().behavior.tap_hold.hold_timeout.as_millis());
            match select(hold_timeout, KEY_EVENT_CHANNEL.receive()).await {
                Either::First(_) => {
                    // Timeout, trigger hold
                    debug!("Hold timeout, got HOLD: {:?}, {:?}", hold_action, key_event);
                    self.process_key_action_normal(hold_action, key_event).await;
                }
                Either::Second(e) => {
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
                    self.keymap.borrow().behavior.tap_hold.post_wait_time.as_millis(),
                );
                match select(wait_timeout, wait_release).await {
                    Either::First(_) => {
                        // Wait timeout, release the hold key finally
                        self.process_key_action_normal(hold_action, key_event).await;
                    }
                    Either::Second(next_press) => {
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
            Action::Modifier(m) => self.process_action_osm(m.to_hid_modifiers(), key_event).await,
            Action::LayerOn(l) => self.process_action_osl(l, key_event).await,
            _ => self.process_key_action_normal(oneshot_action, key_event).await,
        }
    }

    async fn process_action_osm(&mut self, modifiers: HidModifiers, key_event: KeyEvent) {
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

                    let timeout = embassy_time::Timer::after(self.keymap.borrow().behavior.one_shot.timeout);
                    match select(timeout, KEY_EVENT_CHANNEL.receive()).await {
                        Either::First(_) => {
                            // Timeout, release modifiers
                            self.update_osl(key_event);
                            self.osm_state = OneShotState::None;
                        }
                        Either::Second(e) => {
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
                    // one shoot modifiers -> this way "releasing" them.
                    self.send_keyboard_report_with_resolved_modifiers(key_event.pressed)
                        .await;
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

                    let timeout = embassy_time::Timer::after(self.keymap.borrow().behavior.one_shot.timeout);
                    match select(timeout, KEY_EVENT_CHANNEL.receive()).await {
                        Either::First(_) => {
                            // Timeout, deactivate layer
                            self.keymap.borrow_mut().deactivate_layer(layer_num);
                            self.osl_state = OneShotState::None;
                        }
                        Either::Second(e) => {
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
    async fn process_action_keycode(&mut self, mut key: KeyCode, key_event: KeyEvent) {
        if key == KeyCode::Again {
            key = self.last_key_code;
        } else {
            self.last_key_code = key;
        }

        if key.is_consumer() {
            self.process_action_consumer_control(key, key_event).await;
        } else if key.is_system() {
            self.process_action_system_control(key, key_event).await;
        } else if key.is_mouse_key() {
            self.process_action_mouse(key, key_event).await;
        } else if key.is_user() {
            self.process_user(key, key_event).await;
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

    /// calculates the combined effect of "explicit modifiers":
    /// - registered modifiers
    /// - one-shot modifiers
    pub fn resolve_explicit_modifiers(&self, pressed: bool) -> HidModifiers {
        // if a one-shot modifier is active, decorate the hid report of keypress with those modifiers
        let mut result = self.held_modifiers;

        // OneShotState::Held keeps the temporary modifiers active until the key is released
        if pressed {
            if let Some(osm) = self.osm_state.value() {
                result |= *osm;
            }
        } else if let OneShotState::Held(osm) = self.osm_state {
            // One shot modifiers usually "released" together with the key release,
            // except when one-shoot is in "held mode" (to allow Alt+Tab like use cases)
            // In this later case Held -> None state change will report
            // the "modifier released" change in a separate hid report
            result |= osm;
        };

        result
    }

    /// Calculates the combined effect of all modifiers:
    /// - text macro related modifier suppressions + capitalization
    /// - registered (held) modifiers keys
    /// - one-shot modifiers
    /// - effect of KeyAction::WithModifiers (while they are pressed)
    /// - possible fork related modifier suppressions    
    pub fn resolve_modifiers(&self, pressed: bool) -> HidModifiers {
        // text typing macro should not be affected by any modifiers,
        // only its own capitalization
        if self.macro_texting {
            if self.macro_caps {
                return HidModifiers::new().with_left_shift(true);
            } else {
                return HidModifiers::new();
            }
        }

        // "explicit" modifiers: one-shot modifier, registered held modifiers:
        let mut result = self.resolve_explicit_modifiers(pressed);

        // The triggered forks suppress the 'match_any' modifiers automatically
        // unless they are configured as the 'kept_modifiers'
        let mut fork_suppress = HidModifiers::default();
        for fork_state in &self.fork_states {
            if let Some(active) = fork_state {
                fork_suppress |= active.suppress;
            }
        }

        // Some of these suppressions could have been canceled after the fork activation
        // by "explicit" modifier key presses - fork_keep_mask collects these:
        fork_suppress &= !self.fork_keep_mask;

        // Execute the remaining suppressions
        result &= !fork_suppress;

        // Apply the modifiers from KeyAction::WithModifiers
        // the suppression effect of forks should not apply on these
        if pressed {
            result |= self.with_modifiers;
        }

        result
    }

    // process a basic keypress/release and also take care of applying one shot modifiers
    async fn process_basic(&mut self, key: KeyCode, key_event: KeyEvent) {
        if key_event.pressed {
            self.register_key(key, key_event);
        } else {
            self.unregister_key(key, key_event);
        }

        self.send_keyboard_report_with_resolved_modifiers(key_event.pressed)
            .await;
    }

    // process action key
    async fn process_action_key(&mut self, key: KeyCode, key_event: KeyEvent) {
        let key = match key {
            KeyCode::GraveEscape => {
                if self.held_modifiers.into_bits() == 0 {
                    KeyCode::Escape
                } else {
                    KeyCode::Grave
                }
            }
            _ => key,
        };

        self.process_action_keycode(key, key_event).await;
        self.update_osm(key_event);
        self.update_osl(key_event);
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
                    // The key is just released, ignore the key event, use a slightly longer time interval
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

    async fn process_user(&mut self, key: KeyCode, key_event: KeyEvent) {
        debug!("Processing user key: {:?}, event: {:?}", key, key_event);
        #[cfg(feature = "_ble")]
        {
            use crate::ble::trouble::profile::BleProfileAction;
            use crate::ble::trouble::NUM_BLE_PROFILE;
            use crate::channel::BLE_PROFILE_CHANNEL;
            // Get user key id
            let id = key as u8 - KeyCode::User0 as u8;
            if key_event.pressed {
                // Clear Peer is processed when pressed
                if id == NUM_BLE_PROFILE as u8 + 4 {
                    #[cfg(feature = "split")]
                    if key_event.pressed {
                        // Wait for 5s, if the key is still pressed, clear split peer info
                        // If there's any other key event received during this period, skip
                        match select(embassy_time::Timer::after_millis(5000), KEY_EVENT_CHANNEL.receive()).await {
                            Either::First(_) => {
                                // Timeout reached, send clear peer message
                                info!("Clear peer");
                                if let Ok(publisher) = crate::channel::SPLIT_MESSAGE_PUBLISHER.publisher() {
                                    publisher.publish_immediate(crate::split::SplitMessage::ClearPeer);
                                }
                            }
                            Either::Second(e) => {
                                // Received a new key event before timeout, add to unprocessed list
                                if self.unprocessed_events.push(e).is_err() {
                                    warn!("unprocessed event queue is full, dropping event");
                                }
                            }
                        }
                    }
                }
            } else {
                // Other user keys are processed when released
                if id < NUM_BLE_PROFILE as u8 {
                    info!("Switch to profile: {}", id);
                    // User0~7: Swtich to the specific profile
                    BLE_PROFILE_CHANNEL.send(BleProfileAction::SwitchProfile(id)).await;
                } else if id == NUM_BLE_PROFILE as u8 {
                    // User8: Next profile
                    BLE_PROFILE_CHANNEL.send(BleProfileAction::NextProfile).await;
                } else if id == NUM_BLE_PROFILE as u8 + 1 {
                    // User9: Previous profile
                    BLE_PROFILE_CHANNEL.send(BleProfileAction::PreviousProfile).await;
                } else if id == NUM_BLE_PROFILE as u8 + 2 {
                    // User10: Clear profile
                    BLE_PROFILE_CHANNEL.send(BleProfileAction::ClearProfile).await;
                } else if id == NUM_BLE_PROFILE as u8 + 3 {
                    // User11:
                    BLE_PROFILE_CHANNEL.send(BleProfileAction::ToggleConnection).await;
                }
            }
        }
    }

    fn process_boot(&mut self, key: KeyCode, key_event: KeyEvent) {
        // When releasing the key, process the boot action
        if !key_event.pressed {
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
                    let (operation, new_offset) =
                        self.keymap.borrow().get_next_macro_operation(macro_start_idx, offset);
                    // Execute the operation
                    match operation {
                        MacroOperation::Press(k) => {
                            self.macro_texting = false;
                            self.register_key(k, key_event);
                            self.send_keyboard_report_with_resolved_modifiers(true).await;
                        }
                        MacroOperation::Release(k) => {
                            self.macro_texting = false;
                            self.unregister_key(k, key_event);
                            self.send_keyboard_report_with_resolved_modifiers(false).await;
                        }
                        MacroOperation::Tap(k) => {
                            self.macro_texting = false;
                            self.register_key(k, key_event);
                            self.send_keyboard_report_with_resolved_modifiers(true).await;
                            embassy_time::Timer::after_millis(2).await;
                            self.unregister_key(k, key_event);
                            self.send_keyboard_report_with_resolved_modifiers(false).await;
                        }
                        MacroOperation::Text(k, is_cap) => {
                            self.macro_texting = true;
                            self.macro_caps = is_cap;
                            self.register_keycode(k, key_event);
                            self.send_keyboard_report_with_resolved_modifiers(true).await;
                            embassy_time::Timer::after_millis(2).await;
                            self.unregister_keycode(k, key_event);
                            self.send_keyboard_report_with_resolved_modifiers(false).await;
                        }
                        MacroOperation::Delay(t) => {
                            embassy_time::Timer::after_millis(t as u64).await;
                        }
                        MacroOperation::End => {
                            if self.macro_texting {
                                //restore the state of the keyboard (held modifiers, etc.) after text typing
                                self.send_keyboard_report_with_resolved_modifiers(false).await;
                                self.macro_texting = false;
                            }
                            break;
                        }
                    };

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
            self.held_keycodes[index] = key;
            self.registered_keys[index] = Some((key_event.row, key_event.col));
        } else {
            // Otherwise, find the first free slot
            if let Some(index) = self.held_keycodes.iter().position(|&k| k == KeyCode::No) {
                self.held_keycodes[index] = key;
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
            self.held_keycodes[index] = KeyCode::No;
            self.registered_keys[index] = None;
        } else {
            // Otherwise, release the first same key
            if let Some(index) = self.held_keycodes.iter().position(|&k| k == key) {
                self.held_keycodes[index] = KeyCode::No;
                self.registered_keys[index] = None;
            }
        }
    }

    /// Register a modifier to be sent in hid report.
    fn register_modifier_key(&mut self, key: KeyCode) {
        self.held_modifiers |= key.to_hid_modifiers();

        // if a modifier key arrives after fork activation, it should be kept
        self.fork_keep_mask |= key.to_hid_modifiers();
    }

    /// Unregister a modifier from hid report.
    fn unregister_modifier_key(&mut self, key: KeyCode) {
        self.held_modifiers &= !key.to_hid_modifiers();
    }

    /// Register a modifier combination to be sent in hid report.
    fn register_modifiers(&mut self, modifiers: ModifierCombination) {
        self.held_modifiers |= modifiers.to_hid_modifiers();

        // if a modifier key arrives after fork activation, it should be kept
        self.fork_keep_mask |= modifiers.to_hid_modifiers();
    }

    /// Unregister a modifier combination from hid report.
    fn unregister_modifiers(&mut self, modifiers: ModifierCombination) {
        self.held_modifiers &= !modifiers.to_hid_modifiers();
    }
}

#[cfg(test)]
mod test {
    use embassy_futures::block_on;
    use embassy_futures::select::select;
    use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
    use embassy_sync::mutex::Mutex;
    use embassy_time::{Duration, Timer};
    use futures::{join, FutureExt};
    use rusty_fork::rusty_fork_test;

    use super::*;
    use crate::action::KeyAction;
    use crate::config::{BehaviorConfig, CombosConfig, ForksConfig};
    use crate::fork::Fork;
    use crate::hid_state::HidModifiers;
    use crate::{a, k, layer, mo};

    // mod key values
    const KC_LSHIFT: u8 = 1 << 1;

    #[derive(Debug, Clone)]
    struct TestKeyPress {
        row: u8,
        col: u8,
        pressed: bool,
        delay: u64, // Delay before this key event in milliseconds
    }

    async fn run_key_sequence_test<const N: usize>(
        keyboard: &mut Keyboard<'_, 5, 14, 2>,
        key_sequence: &[TestKeyPress],
        expected_reports: Vec<KeyboardReport, N>,
    ) {
        static REPORTS_DONE: Mutex<CriticalSectionRawMutex, bool> = Mutex::new(false);

        KEY_EVENT_CHANNEL.clear();
        KEYBOARD_REPORT_CHANNEL.clear();

        join!(
            // Run keyboard until all reports are received
            async {
                select(keyboard.run(), async {
                    select(
                        Timer::after(Duration::from_secs(5)).then(|_| async {
                            panic!("Test timed out");
                        }),
                        async {
                            while !*REPORTS_DONE.lock().await {
                                Timer::after(Duration::from_millis(10)).await;
                            }
                        },
                    )
                    .await;
                })
                .await;
            },
            // Send all key events with delays
            async {
                for key in key_sequence {
                    Timer::after(Duration::from_millis(key.delay)).await;
                    KEY_EVENT_CHANNEL
                        .send(KeyEvent {
                            row: key.row,
                            col: key.col,
                            pressed: key.pressed,
                        })
                        .await;
                }
            },
            // Verify reports
            async {
                for expected in expected_reports {
                    match KEYBOARD_REPORT_CHANNEL.receive().await {
                        Report::KeyboardReport(report) => {
                            assert_eq!(report, expected, "Expected {:?} but actually {:?}", expected, report);

                            println!("Received expected key report: {:?}", report);
                        }
                        _ => panic!("Expected a KeyboardReport"),
                    }
                }
                // Set done flag after all reports are verified
                *REPORTS_DONE.lock().await = true;
            }
        );

        // Reset the done flag for next test
        *REPORTS_DONE.lock().await = false;
    }

    macro_rules! key_sequence {
    ($([$row:expr, $col:expr, $pressed:expr, $delay:expr]),* $(,)?) => {
        vec![
            $(
                TestKeyPress {
                    row: $row,
                    col: $col,
                    pressed: $pressed,
                    delay: $delay,
                },
            )*
        ]
    };
    }

    macro_rules! key_report {
    ( $([$modifier:expr, $keys:expr]),* $(,)? ) => {{
        // Count the number of elements at compile time

        const N: usize = {
            let arr = [$((($modifier, $keys)),)*];
            arr.len()
        };


        let mut reports: Vec<KeyboardReport, N> = Vec::new();
        $(
            reports.push(KeyboardReport {
                modifier: $modifier,
                keycodes: $keys,
                leds: 0,
                reserved: 0,
            }).unwrap();
        )*
        reports
    }};
    }

    // Init logger for tests
    #[ctor::ctor]
    fn init_log() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();
    }

    #[rustfmt::skip]
    pub const fn get_keymap() -> [[[KeyAction; 14]; 5]; 2] {
        [
            layer!([
                [k!(Grave), k!(Kc1), k!(Kc2), k!(Kc3), k!(Kc4), k!(Kc5), k!(Kc6), k!(Kc7), k!(Kc8), k!(Kc9), k!(Kc0), k!(Minus), k!(Equal), k!(Backspace)],
                [k!(Tab), k!(Q), k!(W), k!(E), k!(R), k!(T), k!(Y), k!(U), k!(I), k!(O), k!(P), k!(LeftBracket), k!(RightBracket), k!(Backslash)],
                [k!(Escape), k!(A), k!(S), k!(D), k!(F), k!(G), k!(H), k!(J), k!(K), k!(L), k!(Semicolon), k!(Quote), a!(No), k!(Enter)],
                [k!(LShift), k!(Z), k!(X), k!(C), k!(V), k!(B), k!(N), k!(M), k!(Comma), k!(Dot), k!(Slash), a!(No), a!(No), k!(RShift)],
                [k!(LCtrl), k!(LGui), k!(LAlt), a!(No), a!(No), k!(Space), a!(No), a!(No), a!(No), mo!(1), k!(RAlt), a!(No), k!(RGui), k!(RCtrl)]
            ]),
            layer!([
                [k!(Grave), k!(F1), k!(F2), k!(F3), k!(F4), k!(F5), k!(F6), k!(F7), k!(F8), k!(F9), k!(F10), k!(F11), k!(F12), k!(Delete)],
                [a!(No), a!(Transparent), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No)],
                [k!(CapsLock), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No)],
                [a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), k!(Up)],
                [a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), a!(No), k!(Left), a!(No), k!(Down), k!(Right)]
            ]),
        ]
    }

    fn get_combos_config() -> CombosConfig {
        // Define the function to return the appropriate combo configuration
        CombosConfig {
            combos: Vec::from_iter([
                Combo::new(
                    [
                        k!(V), //3,4
                        k!(B), //3,5
                    ]
                    .to_vec(),
                    k!(LShift),
                    Some(0),
                ),
                Combo::new(
                    [
                        k!(R), //1,4
                        k!(T), //1,5
                    ]
                    .to_vec(),
                    k!(LAlt),
                    Some(0),
                ),
            ]),
            timeout: Duration::from_millis(100),
        }
    }

    fn create_test_keyboard_with_config(config: BehaviorConfig) -> Keyboard<'static, 5, 14, 2> {
        // Box::leak is acceptable in tests
        let keymap = Box::new(get_keymap());
        let leaked_keymap = Box::leak(keymap);

        let keymap = block_on(KeyMap::new(leaked_keymap, None, config.clone()));
        let keymap_cell = RefCell::new(keymap);
        let keymap_ref = Box::leak(Box::new(keymap_cell));

        Keyboard::new(keymap_ref, config)
    }

    fn create_test_keyboard() -> Keyboard<'static, 5, 14, 2> {
        create_test_keyboard_with_config(BehaviorConfig::default())
    }

    fn key_event(row: u8, col: u8, pressed: bool) -> KeyEvent {
        KeyEvent { row, col, pressed }
    }

    rusty_fork_test! {
    #[test]
    fn test_register_key() {
        let main = async {
            let mut keyboard = create_test_keyboard();
            keyboard.register_key(KeyCode::A, key_event(2, 1, true));
            assert_eq!(keyboard.held_keycodes[0], KeyCode::A);
        };
        block_on(main);
    }
    }
    rusty_fork_test! {
    #[test]
    fn test_basic_key_press_release() {
        let main = async {
            let mut keyboard = create_test_keyboard();

            // Press A key
            keyboard.process_inner(key_event(2, 1, true)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::A); // A key's HID code is 0x04

            // Release A key
            keyboard.process_inner(key_event(2, 1, false)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::No);
        };
        block_on(main);
    }
    }
    rusty_fork_test! {
    #[test]
    fn test_modifier_key() {
        let main = async {
            let mut keyboard = create_test_keyboard();

            // Press Shift key
            keyboard.register_key(KeyCode::LShift, key_event(3, 0, true));
            assert_eq!(keyboard.held_modifiers, HidModifiers::new().with_left_shift(true)); // Left Shift's modifier bit is 0x02

            // Release Shift key
            keyboard.unregister_key(KeyCode::LShift, key_event(3, 0, false));
            assert_eq!(keyboard.held_modifiers, HidModifiers::new());
        };
        block_on(main);
    }
    }
    rusty_fork_test! {
    #[test]
    fn test_tap_hold_key() {
        let main = async {
            let mut keyboard = create_test_keyboard();
            let tap_hold_action = KeyAction::TapHold(Action::Key(KeyCode::A), Action::Key(KeyCode::LShift));

            // Tap
            keyboard
                .process_key_action(tap_hold_action.clone(), key_event(2, 1, true))
                .await;
            Timer::after(Duration::from_millis(10)).await;
            keyboard
                .process_key_action(tap_hold_action.clone(), key_event(2, 1, false))
                .await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::No); // A should be released
            assert_eq!(keyboard.held_modifiers, HidModifiers::new()); // Shift should not held

            // Hold
            keyboard
                .process_key_action(tap_hold_action.clone(), key_event(2, 1, true))
                .await;
            Timer::after(Duration::from_millis(200)).await; // wait for hold timeout
            assert_eq!(keyboard.held_modifiers, HidModifiers::new().with_left_shift(true)); // should activate Shift modifier
            assert_eq!(keyboard.held_keycodes[0], KeyCode::No); // A should not be pressed

            keyboard
                .process_key_action(tap_hold_action, key_event(2, 1, false))
                .await;
            assert_eq!(keyboard.held_modifiers, HidModifiers::new()); // Shift should be released
        };
        block_on(main);
    }
    }
    rusty_fork_test! {
    #[test]
    fn test_combo_timeout_and_ignore() {
        let main = async {
            let mut keyboard = create_test_keyboard_with_config(BehaviorConfig {
                combo: get_combos_config(),
                ..Default::default()
            });

            let sequence = key_sequence![
                [3, 4, true, 10],   // Press V
                [3, 4, false, 100], // Release V
            ];

            let expected_reports = key_report![
                [0, [KeyCode::V as u8, 0, 0, 0, 0, 0]],
            ];

            run_key_sequence_test(&mut keyboard, &sequence, expected_reports).await;
        };

        block_on(main);
    }
    }
    rusty_fork_test! {
    #[test]
    fn test_combo_with_mod_then_mod_timeout() {
        let main = async {
            let mut keyboard = create_test_keyboard_with_config(BehaviorConfig {
                combo: get_combos_config(),
                ..Default::default()
            });
            let sequence = key_sequence![
                [3, 4, true, 10], // Press V
                [3, 5, true, 10], // Press B
                [1, 4, true, 50], // Press R
                [1, 4, false, 90], // Release R
                [3, 4, false, 150], // Release V
                [3, 5, false, 170], // Release B
            ];

            let expected_reports = key_report![
                [KC_LSHIFT, [0; 6]],
                [KC_LSHIFT, [KeyCode::R as u8, 0, 0, 0, 0, 0]],
                [KC_LSHIFT, [0; 6]],
                [0, [0; 6]],
            ];

            run_key_sequence_test(&mut keyboard, &sequence, expected_reports).await;
        };

        block_on(main);
    }
    }

    rusty_fork_test! {
    #[test]
    fn test_combo_with_mod() {
        let main = async {
            let mut keyboard = create_test_keyboard_with_config(BehaviorConfig {
                combo: get_combos_config(),
                ..Default::default()
            });

            let sequence = key_sequence![
                [3, 4, true, 10],   // Press V
                [3, 5, true, 10],   // Press B
                [3, 6, true, 50],   // Press N
                [3, 6, false, 70],  // Release N
                [3, 4, false, 100], // Release V
                [3, 5, false, 110], // Release B
            ];

            let expected_reports = key_report![
                [KC_LSHIFT, [0; 6]],
                [KC_LSHIFT, [KeyCode::N as u8, 0, 0, 0, 0, 0]],
                [KC_LSHIFT, [0; 6]],
                [0, [0; 6]],
            ];

            run_key_sequence_test(&mut keyboard, &sequence, expected_reports).await;
        };

        block_on(main);
    }
    }

    rusty_fork_test! {
    #[test]
    fn test_multiple_keys() {
        let main = async {
            let mut keyboard = create_test_keyboard();

            keyboard.process_inner(key_event(2, 1, true)).await;
            assert!(keyboard.held_keycodes.contains(&KeyCode::A));

            keyboard.process_inner(key_event(3, 5, true)).await;
            assert!(keyboard.held_keycodes.contains(&KeyCode::A) && keyboard.held_keycodes.contains(&KeyCode::B));

            keyboard.process_inner(key_event(3, 5, false)).await;
            assert!(keyboard.held_keycodes.contains(&KeyCode::A) && !keyboard.held_keycodes.contains(&KeyCode::B));

            keyboard.process_inner(key_event(2, 1, false)).await;
            assert!(!keyboard.held_keycodes.contains(&KeyCode::A));
            assert!(keyboard.held_keycodes.iter().all(|&k| k == KeyCode::No));
        };

        block_on(main);
    }
    }
    rusty_fork_test! {
    #[test]
    fn test_repeat_key_single() {
        let main = async {
            let mut keyboard = create_test_keyboard();
            keyboard.keymap.borrow_mut().set_action_at(
                0,
                0,
                0,
                KeyAction::Single(Action::Key(KeyCode::Again)),
            );

            // first press ever of the Again issues KeyCode:No
            keyboard.process_inner(key_event(0, 0, true)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::No); // A key's HID code is 0x04

            // Press A key
            keyboard.process_inner(key_event(2, 1, true)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::A); // A key's HID code is 0x04

            // Release A key
            keyboard.process_inner(key_event(2, 1, false)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::No);

            // after another key is pressed, that key is repeated
            keyboard.process_inner(key_event(0, 0, true)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::A); // A key's HID code is 0x04

            // releasing the repeat key
            keyboard.process_inner(key_event(0, 0, false)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::No); // A key's HID code is 0x04

            // Press S key
            keyboard.process_inner(key_event(2, 2, true)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::S); // A key's HID code is 0x04

            // after another key is pressed, that key is repeated
            keyboard.process_inner(key_event(0, 0, true)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::S); // A key's HID code is 0x04
        };
        block_on(main);
    }
    }
    rusty_fork_test! {
    #[test]
    fn test_repeat_key_th() {
        let main = async {
            let mut keyboard = create_test_keyboard();
            keyboard.keymap.borrow_mut().set_action_at(
                0,
                0,
                0,
                KeyAction::TapHold(Action::Key(KeyCode::F), Action::Key(KeyCode::Again)),
            );

            // first press ever of the Again issues KeyCode:No
            keyboard.process_inner(key_event(0, 0, true)).await;
            keyboard
                .send_keyboard_report_with_resolved_modifiers(true)
                .await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::No); // A key's HID code is 0x04

            // Press A key
            keyboard.process_inner(key_event(2, 1, true)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::A); // A key's HID code is 0x04

            // Release A key
            keyboard.process_inner(key_event(2, 1, false)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::No);

            // after another key is pressed, that key is repeated
            keyboard.process_inner(key_event(0, 0, true)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::A); // A key's HID code is 0x04

            // releasing the repeat key
            keyboard.process_inner(key_event(0, 0, false)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::No); // A key's HID code is 0x04

            // Press S key
            keyboard.process_inner(key_event(2, 2, true)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::S); // A key's HID code is 0x04

            // after another key is pressed, that key is repeated
            keyboard.process_inner(key_event(0, 0, true)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::S); // A key's HID code is 0x04
        };
        block_on(main);
    }
    }
    rusty_fork_test! {
    #[test]
    fn test_key_action_transparent() {
        let main = async {
            let mut keyboard = create_test_keyboard();

            // Activate layer 1
            keyboard.process_action_layer_switch(1, key_event(0, 0, true));

            // Press Transparent key (Q on lower layer)
            keyboard.process_inner(key_event(1, 1, true)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::Q); // Q key's HID code is 0x14

            // Release Transparent key
            keyboard.process_inner(key_event(1, 1, false)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::No);
        };
        block_on(main);
    }
    }
    rusty_fork_test! {
    #[test]
    fn test_key_action_no() {
        let main = async {
            let mut keyboard = create_test_keyboard();

            // Press No key
            keyboard.process_inner(key_event(4, 3, true)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::No);

            // Release No key
            keyboard.process_inner(key_event(4, 3, false)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::No);
        };
        block_on(main);
    }
    }

    fn create_test_keyboard_with_forks(fork1: Fork, fork2: Fork) -> Keyboard<'static, 5, 14, 2> {
        let mut cfg = ForksConfig::default();
        let _ = cfg.forks.push(fork1);
        let _ = cfg.forks.push(fork2);
        create_test_keyboard_with_config(BehaviorConfig {
            fork: cfg,
            ..BehaviorConfig::default()
        })
    }

    rusty_fork_test! {
    #[test]
    fn test_fork_with_held_modifier() {
        let main = async {
            //{ trigger = "Dot", negative_output = "Dot", positive_output = "WM(Semicolon, LShift)", match_any = "LShift|RShift" },
            let fork1 = Fork {
                trigger: KeyAction::Single(Action::Key(KeyCode::Dot)),
                negative_output: KeyAction::Single(Action::Key(KeyCode::Dot)),
                positive_output: KeyAction::WithModifier(
                    Action::Key(KeyCode::Semicolon),
                    ModifierCombination::default().with_shift(true),
                ),
                match_any: StateBits {
                    modifiers: HidModifiers::default().with_left_shift(true).with_right_shift(true),
                    leds: LedIndicator::default(),
                    mouse: HidMouseButtons::default(),
                },
                match_none: StateBits::default(),
                kept_modifiers: HidModifiers::default(),
                bindable: false,
            };

            //{ trigger = "Comma", negative_output = "Comma", positive_output = "Semicolon", match_any = "LShift|RShift" },
            let fork2 = Fork {
                trigger: KeyAction::Single(Action::Key(KeyCode::Comma)),
                negative_output: KeyAction::Single(Action::Key(KeyCode::Comma)),
                positive_output: KeyAction::Single(Action::Key(KeyCode::Semicolon)),
                match_any: StateBits {
                    modifiers: HidModifiers::default().with_left_shift(true).with_right_shift(true),
                    leds: LedIndicator::default(),
                    mouse: HidMouseButtons::default(),
                },
                match_none: StateBits::default(),
                kept_modifiers: HidModifiers::default(),
                bindable: false,
            };

            let mut keyboard = create_test_keyboard_with_forks(fork1, fork2);

            // Press Dot key, by itself it should emit '.'
            keyboard.process_inner(key_event(3, 9, true)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::Dot);

            // Release Dot key
            keyboard.process_inner(key_event(3, 9, false)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::No);

            // Press LShift key
            keyboard.process_inner(key_event(3, 0, true)).await;

            // Press Dot key, with shift it should emit ':'
            keyboard.process_inner(key_event(3, 9, true)).await;
            assert_eq!(
                keyboard.resolve_modifiers(true),
                HidModifiers::new().with_left_shift(true)
            );
            assert_eq!(keyboard.held_keycodes[0], KeyCode::Semicolon);

            //Release Dot key
            keyboard.process_inner(key_event(3, 9, false)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::No);
            assert_eq!(
                keyboard.resolve_modifiers(false),
                HidModifiers::new().with_left_shift(true)
            );

            // Release LShift key
            keyboard.process_inner(key_event(3, 0, false)).await;
            assert_eq!(keyboard.held_modifiers, HidModifiers::new());
            assert_eq!(keyboard.resolve_modifiers(false), HidModifiers::new());

            // Press Comma key, by itself it should emit ','
            keyboard.process_inner(key_event(3, 8, true)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::Comma);

            // Release Dot key
            keyboard.process_inner(key_event(3, 8, false)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::No);

            // Press LShift key
            keyboard.process_inner(key_event(3, 0, true)).await;

            // Press Comma key, with shift it should emit ';' (shift is suppressed)
            keyboard.process_inner(key_event(3, 8, true)).await;
            assert_eq!(keyboard.resolve_modifiers(true), HidModifiers::new());
            assert_eq!(keyboard.held_keycodes[0], KeyCode::Semicolon);

            // Release Comma key, shift is still held
            keyboard.process_inner(key_event(3, 8, false)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::No);
            assert_eq!(
                keyboard.resolve_modifiers(false),
                HidModifiers::new().with_left_shift(true)
            );

            // Release LShift key
            keyboard.process_inner(key_event(3, 0, false)).await;
            assert_eq!(keyboard.held_modifiers, HidModifiers::new());
            assert_eq!(keyboard.resolve_modifiers(false), HidModifiers::new());
        };

        block_on(main);
    }
    }
    rusty_fork_test! {
    #[test]
    fn test_fork_with_held_mouse_button() {
        let main = async {
            //{ trigger = "Z", negative_output = "MouseBtn5", positive_output = "C", match_any = "LCtrl|RCtrl|LShift|RShift", kept_modifiers="LShift|RShift" },
            let fork1 = Fork {
                trigger: KeyAction::Single(Action::Key(KeyCode::Z)),
                negative_output: KeyAction::Single(Action::Key(KeyCode::MouseBtn5)),
                positive_output: KeyAction::Single(Action::Key(KeyCode::C)),
                match_any: StateBits {
                    modifiers: HidModifiers::default()
                        .with_left_ctrl(true)
                        .with_right_ctrl(true)
                        .with_left_shift(true)
                        .with_right_shift(true),
                    leds: LedIndicator::default(),
                    mouse: HidMouseButtons::default(),
                },
                match_none: StateBits::default(),
                kept_modifiers: HidModifiers::default().with_left_shift(true).with_right_shift(true),
                bindable: false,
            };

            //{ trigger = "A", negative_output = "S", positive_output = "D", match_any = "MouseBtn5" },
            let fork2 = Fork {
                trigger: KeyAction::Single(Action::Key(KeyCode::A)),
                negative_output: KeyAction::Single(Action::Key(KeyCode::S)),
                positive_output: KeyAction::Single(Action::Key(KeyCode::D)),
                match_any: StateBits {
                    modifiers: HidModifiers::default(),
                    leds: LedIndicator::default(),
                    mouse: HidMouseButtons::default().with_button5(true),
                },
                match_none: StateBits::default(),
                kept_modifiers: HidModifiers::default(),
                bindable: false,
            };

            let mut keyboard = create_test_keyboard_with_forks(fork1, fork2);

            // Press Z key, by itself it should emit 'MouseBtn5'
            keyboard.process_inner(key_event(3, 1, true)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::No);
            assert_eq!(keyboard.mouse_report.buttons, 1u8 << 4); // MouseBtn5

            // Release Z key
            keyboard.process_inner(key_event(3, 1, false)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::No);
            assert_eq!(keyboard.mouse_report.buttons, 0);

            // Press LCtrl key
            keyboard.process_inner(key_event(4, 0, true)).await;
            // Press LShift key
            keyboard.process_inner(key_event(3, 0, true)).await;
            assert_eq!(
                keyboard.resolve_modifiers(true),
                HidModifiers::new()
                    .with_left_ctrl(true)
                    .with_left_shift(true)
            );

            // Press 'Z' key, with Ctrl it should emit 'C', with suppressed ctrl, but kept shift
            keyboard.process_inner(key_event(3, 1, true)).await;
            assert_eq!(
                keyboard.resolve_modifiers(true),
                HidModifiers::new().with_left_shift(true)
            );
            assert_eq!(keyboard.held_keycodes[0], KeyCode::C);
            assert_eq!(keyboard.mouse_report.buttons, 0);

            // Release 'Z' key, suppression of ctrl is removed
            keyboard.process_inner(key_event(3, 1, false)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::No);
            assert_eq!(
                keyboard.resolve_modifiers(false),
                HidModifiers::new()
                    .with_left_ctrl(true)
                    .with_left_shift(true)
            );

            // Release LCtrl key
            keyboard.process_inner(key_event(4, 0, false)).await;
            assert_eq!(
                keyboard.resolve_modifiers(false),
                HidModifiers::new().with_left_shift(true)
            );

            // Release LShift key
            keyboard.process_inner(key_event(3, 0, false)).await;
            assert_eq!(keyboard.resolve_modifiers(false), HidModifiers::new());

            // Press 'A' key, by itself it should emit 'S'
            keyboard.process_inner(key_event(2, 1, true)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::S);

            // Release 'A' key
            keyboard.process_inner(key_event(2, 1, false)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::No);
            assert_eq!(keyboard.resolve_modifiers(false), HidModifiers::new());
            assert_eq!(keyboard.mouse_report.buttons, 0);

            Timer::after(Duration::from_millis(200)).await; // wait a bit

            // Press Z key, by itself it should emit 'MouseBtn5'
            keyboard.process_inner(key_event(3, 1, true)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::No);
            assert_eq!(keyboard.mouse_report.buttons, 1u8 << 4); // MouseBtn5 //this fails, but ok in debug - why?

            // Press 'A' key, with 'MouseBtn5' it should emit 'D'
            keyboard.process_inner(key_event(2, 1, true)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::D);

            // Release Z (MouseBtn1) key, 'D' is still held
            keyboard.process_inner(key_event(3, 8, false)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::D);

            // Release 'A' key -> releases 'D'
            keyboard.process_inner(key_event(2, 1, false)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::No);
        };

        block_on(main);
    }
    }
}
