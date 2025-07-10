use core::cell::RefCell;
use core::cmp::Ordering;
use core::fmt::Debug;

use embassy_futures::select::{select, Either};
use embassy_futures::yield_now;
use embassy_time::{Duration, Instant, Timer};
use heapless::Vec;
use usbd_hid::descriptor::{MediaKeyboardReport, MouseReport, SystemControlReport};
use TapHoldDecision::{Buffering, Ignore};
use TapHoldState::Initial;
#[cfg(feature = "controller")]
use {
    crate::channel::{send_controller_event, ControllerPub, CONTROLLER_CHANNEL},
    crate::event::ControllerEvent,
};

use crate::action::{Action, KeyAction};
use crate::channel::{KEYBOARD_REPORT_CHANNEL, KEY_EVENT_CHANNEL};
use crate::combo::Combo;
use crate::descriptor::{KeyboardReport, ViaReport};
use crate::event::KeyEvent;
use crate::fork::{ActiveFork, StateBits};
use crate::hid::Report;
use crate::hid_state::{HidModifiers, HidMouseButtons};
use crate::input_device::Runnable;
use crate::keyboard_macros::MacroOperation;
use crate::keycode::{KeyCode, ModifierCombination};
use crate::keymap::KeyMap;
use crate::light::LedIndicator;
#[cfg(all(feature = "split", feature = "_ble"))]
use crate::split::ble::central::update_activity_time;
use crate::tap_hold::TapHoldDecision::{ChordHold, CleanBuffer, Hold};
use crate::tap_hold::{ChordHoldState, HoldingKey, TapHoldDecision, TapHoldState};
use crate::{boot, FORK_MAX_NUM};

const HOLD_BUFFER_SIZE: usize = 16;

/// Led states for the keyboard hid report (its value is received by by the light service in a hid report)
/// LedIndicator type would be nicer, but that does not have const expr constructor
pub(crate) static LOCK_LED_STATES: core::sync::atomic::AtomicU8 = core::sync::atomic::AtomicU8::new(0u8);

#[derive(Debug)]
enum LoopState {
    /// Default state, fire and forget current key event
    OK,
    /// Save current event into buffer
    Queue,
    /// Flush event buffer
    Flush,
    /// Stop keyboard running
    Stop,
}

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
            let result = match self.next_buffered_key() {
                Some(key) => self.process_buffered_key(key).await,
                None => {
                    // No buffered tap-hold event, wait for new key
                    let key_event = KEY_EVENT_CHANNEL.receive().await;
                    // Process the key event
                    self.process_inner(key_event).await
                }
            };

            match result {
                LoopState::Queue => {
                    // keep unprocessed key events
                    // every key should be buffered into event list, check in every turn in future
                    continue;
                }
                LoopState::Stop => {
                    return;
                }
                _ => {
                    // Stop buffering, clean all buffered events
                    self.clean_buffered_tap_keys();

                    // After processing the key event, check if there are unprocessed events
                    // This will happen if there's recursion in key processing
                    if self.holding_buffer.is_empty() && !self.unprocessed_events.is_empty() {
                        while !self.unprocessed_events.is_empty() {
                            // Process unprocessed events
                            let e = self.unprocessed_events.remove(0);
                            debug!("Unprocessed event: {:?}", e);
                            self.process_inner(e).await;
                        }
                    }
                }
            }

            embassy_time::Timer::after_micros(500).await;
        }
    }
}

pub struct Keyboard<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize = 0> {
    /// Keymap
    pub(crate) keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,

    /// Unprocessed events
    unprocessed_events: Vec<KeyEvent, 16>,

    /// Buffered holding keys
    pub holding_buffer: Vec<HoldingKey, HOLD_BUFFER_SIZE>,

    chord_state: Option<ChordHoldState<COL>>,

    /// Timer which records the timestamp of key changes
    pub(crate) timer: [[Option<Instant>; ROW]; COL],

    /// Record the timestamp of last release, (event, is_modifier, timestamp)
    last_release: (KeyEvent, bool, Option<Instant>),

    /// Record whether the keyboard is in hold-after-tap state
    hold_after_tap: [Option<KeyEvent>; 6],

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

    /// The held modifiers for the keyboard hid report
    held_modifiers: HidModifiers,

    /// The held keys for the keyboard hid report, except the modifiers
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

    /// Mouse acceleration state
    mouse_accel: u8,
    mouse_repeat: u8,
    mouse_wheel_repeat: u8,

    /// Used for temporarily disabling combos
    combo_on: bool,

    /// Publisher for controller channel
    #[cfg(feature = "controller")]
    controller_pub: ControllerPub,
}

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    Keyboard<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    pub fn new(keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>) -> Self {
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
            osl_state: OneShotState::default(),
            osm_state: OneShotState::default(),
            with_modifiers: HidModifiers::default(),
            macro_texting: false,
            macro_caps: false,
            fork_states: [None; FORK_MAX_NUM],
            fork_keep_mask: HidModifiers::default(),
            unprocessed_events: Vec::new(),
            holding_buffer: Vec::new(),
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
            mouse_accel: 0,
            mouse_repeat: 0,
            mouse_wheel_repeat: 0,
            combo_on: true,
            #[cfg(feature = "controller")]
            controller_pub: unwrap!(CONTROLLER_CHANNEL.publisher()),
            chord_state: None,
        }
    }

    async fn send_report(&self, report: Report) {
        KEYBOARD_REPORT_CHANNEL.sender().send(report).await
    }

    // A tap hold key reaches timeout, turning into hold press event
    async fn process_tap_hold_timeout(&mut self, key: HoldingKey) -> LoopState {
        info!(
            "[TAP-HOLD] TIMEOUT: tap hold event now reach timeout and should become hold: {:?}",
            key
        );
        self.fire_holding_keys(TapHoldDecision::Timeout, key.key_event).await;
        LoopState::OK
    }

    // Clean up for leak keys, remove non-tap-hold keys in PostTap state from the buffer
    pub(crate) fn clean_buffered_tap_keys(&mut self) {
        self.holding_buffer.retain(|e| match e.action {
            KeyAction::TapHold(_, _) => true,
            _ => match e.state {
                TapHoldState::PostTap => {
                    debug!("Processing buffering TAP keys with post tap: {:?}", e.key_event);
                    false
                }
                _ => true,
            },
        });
    }

    /// Process the latest buffered key.
    ///
    /// The given holding key is a copy of the buffered key. Only tap-hold keys are considered now.
    /// TODO: process other type of buffered holding keys
    async fn process_buffered_key(&mut self, key: HoldingKey) -> LoopState {
        debug!("Processing buffered key: {:?}", key);
        match key.state {
            TapHoldState::WaitingCombo => {
                let time_left = if self.keymap.borrow().behavior.combo.timeout > key.pressed_time.elapsed() {
                    self.keymap.borrow().behavior.combo.timeout - key.pressed_time.elapsed()
                } else {
                    Duration::from_ticks(0)
                };
                debug!("[COMBO] Waiting combo, timeout in: {:?}ms", time_left.as_millis());
                match select(Timer::after(time_left), KEY_EVENT_CHANNEL.receive()).await {
                    Either::First(_) => {
                        // Timeout, dispatch combo
                        self.dispatch_combos().await;
                        LoopState::OK
                    }
                    Either::Second(key_event) => {
                        // Process new key event
                        debug!("[TAP-HOLD] Interrupted into new key event: {:?}", key_event);
                        self.process_inner(key_event).await;
                        LoopState::OK
                    }
                }
            }
            _ => {
                let time_left = if self.keymap.borrow().behavior.tap_hold.hold_timeout > key.pressed_time.elapsed() {
                    self.keymap.borrow().behavior.tap_hold.hold_timeout - key.pressed_time.elapsed()
                } else {
                    Duration::from_ticks(0)
                };

                debug!(
                    "[TAP-HOLD] Processing buffered tap-hold key: {:?}, timeout in {} ms",
                    key.key_event,
                    time_left.as_millis()
                );

                // Wait for hold timeout or new key event
                match select(Timer::after(time_left), KEY_EVENT_CHANNEL.receive()).await {
                    Either::First(_) => self.process_tap_hold_timeout(key).await,
                    Either::Second(key_event) => {
                        // Process new key event
                        debug!("[TAP-HOLD] Interrupted into new key event: {:?}", key_event);
                        self.process_inner(key_event).await;
                        LoopState::OK
                    }
                }
            }
        }
    }

    /// Process key changes at (row, col)
    async fn process_inner(&mut self, key_event: KeyEvent) -> LoopState {
        // Matrix should process key pressed event first, record the timestamp of key changes
        if key_event.pressed {
            self.timer[key_event.col as usize][key_event.row as usize] = Some(Instant::now());
        }

        // Update activity time for BLE split central sleep management
        #[cfg(all(feature = "split", feature = "_ble"))]
        update_activity_time();

        // Process key
        let key_action = self.keymap.borrow_mut().get_action_with_layer_cache(key_event);

        if self.combo_on {
            if let Some(key_action) = self.process_combo(key_action, key_event).await {
                self.process_key_action(key_action, key_event).await
            } else {
                LoopState::OK
            }
        } else {
            self.process_key_action(key_action, key_event).await
        }
    }

    pub(crate) async fn send_keyboard_report_with_resolved_modifiers(&mut self, pressed: bool) {
        // all modifier related effects are combined here to be sent with the hid report:
        let modifiers = self.resolve_modifiers(pressed);

        self.send_report(Report::KeyboardReport(KeyboardReport {
            modifier: modifiers.into_bits(),
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

    // Calculate next state of tap hold
    // 1. turn into buffering state
    // 2. clean buffer
    // 3. ignore
    fn make_tap_hold_decision(&mut self, key_action: KeyAction, key_event: KeyEvent) -> TapHoldDecision {
        let permissive = self.keymap.borrow().behavior.tap_hold.permissive_hold;

        // Check if there's buffered tap-hold key
        let is_buffered = self.holding_buffer.iter().any(|i| match i.action {
            KeyAction::TapHold(_, _) => i.state == TapHoldState::Initial,
            _ => false,
        });

        debug!(
            "\x1b[34m[TAP-HOLD] tap_hold_decision\x1b[0m: permissive={}, is_tap_hold_buffered={}, is_pressed={}, action={:?}",
            permissive, is_buffered, key_event.pressed, key_action
        );

        if is_buffered {
            if key_event.pressed {
                // New key pressed after a tap-hold key.

                // 1. Check chordal hold
                if let Some(hand) = &self.chord_state {
                    // TODO: add more chordal configuration and behaviors here
                    if !hand.is_same(key_event) {
                        debug!("Is chordal hold hand: {:?}, raise", hand);
                        return ChordHold;
                    }
                }

                // 2. Permissive hold
                //
                // Permissive hold checks the key release, so the pressed key should be buffered when pressed.
                if permissive {
                    // Buffer pressed keys if permissive hold is enabled.
                    return match key_action {
                        KeyAction::TapHold(_, _) => {
                            // Ignore following tap-hold keys, they will be always checked
                            Ignore
                        }
                        _ => {
                            // Buffer keys and wait for key release
                            debug!("key {:?} press down while BUFFERING, save it into buffer", key_action);
                            Buffering
                        }
                    };
                }
            } else {
                // Key releasing while tap-holding
                if permissive {
                    // PERMISSIVE HOLDING, which means any key press-and-release after a tap-hold key will raise hold decision
                    // Key release while permissive hold is enabled, hold will be triggered
                    return CleanBuffer;
                };
            }
        }

        // Default decision
        Ignore
    }

    async fn process_key_action(&mut self, mut original_key_action: KeyAction, key_event: KeyEvent) -> LoopState {
        let decision = self.make_tap_hold_decision(original_key_action, key_event);

        debug!("\x1b[34m[TAP-HOLD] --> decision is \x1b[0m: {:?}", decision);
        match decision {
            Ignore => {}
            Buffering => {
                // Save into buffer, will be process in the future
                self.add_holding_key_to_buffer(key_event, original_key_action, Initial);
                return LoopState::Queue;
            }
            CleanBuffer | Hold | ChordHold => {
                // CleanBuffer: permissive hold is triggered by a key release
                // ChordHold: chordal hold is triggered by a key press
                // Hold: impossible for now
                self.fire_holding_keys(decision, key_event).await;
                // Because the layer/keymap state might be changed after `fire_holding_keys`, so we need to get the key action again
                original_key_action = self.keymap.borrow_mut().get_action_with_layer_cache(key_event);
            }
            _ => {
                error!("Unexpected tap hold decision {:?}", decision);
                return LoopState::OK;
            }
        }

        debug!("Processing key action: {:?}", original_key_action);
        // Process current key action after tap-hold decision and (optional) all holding keys are resolved
        self.process_key_action_inner(original_key_action, key_event).await
    }

    async fn process_key_action_inner(&mut self, original_key_action: KeyAction, key_event: KeyEvent) -> LoopState {
        // Start forks
        let key_action = self.try_start_forks(original_key_action, key_event);

        #[cfg(feature = "controller")]
        send_controller_event(&mut self.controller_pub, ControllerEvent::Key(key_event, key_action));

        match key_action {
            KeyAction::No | KeyAction::Transparent => (),
            KeyAction::Single(a) => {
                debug!("Process Single key action: {:?}, {:?}", a, key_event);
                self.process_key_action_normal(a, key_event).await;
            }
            KeyAction::WithModifier(a, m) => self.process_key_action_with_modifier(a, m, key_event).await,
            KeyAction::Tap(a) => self.process_key_action_tap(a, key_event).await,
            KeyAction::TapHold(tap_action, hold_action) => {
                self.process_key_action_tap_hold(tap_action, hold_action, key_event)
                    .await;
            }
            KeyAction::OneShot(oneshot_action) => self.process_key_action_oneshot(oneshot_action, key_event).await,
            KeyAction::TapDance(index) => {
                self.process_key_action_tap_dance(index, key_event).await;
            }
        }

        // Release to early
        if !key_event.pressed {
            // Record release of current key, which will be used in tap/hold processing
            debug!("Record released key event: {:?}", key_event);
            let mut is_mod = false;
            if let KeyAction::Single(Action::Key(k)) = key_action {
                // TODO: Use if-let chain
                if k.is_modifier() {
                    is_mod = true;
                }
            }
            // Record the last release event
            // TODO: check key action, should be a-z/space/enter
            self.last_release = (key_event, is_mod, Some(Instant::now()));
        }

        self.try_finish_forks(original_key_action, key_event);

        LoopState::OK
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
                        debug!("replace input with fork action {:?}", active);
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
            if self
                .holding_buffer
                .push(HoldingKey {
                    state: TapHoldState::WaitingCombo,
                    key_event,
                    pressed_time: Instant::now(),
                    action: key_action,
                })
                .is_err()
            {
                error!("Holding buffer overflowed when saving combo action");
            }

            // FIXME: last combo is not checked
            let next_action = self
                .keymap
                .borrow_mut()
                .behavior
                .combo
                .combos
                .iter_mut()
                .find_map(|combo| (combo.is_all_pressed() && !combo.is_triggered()).then_some(combo.trigger()));

            if next_action.is_some() {
                debug!("[COMBO] {:?} triggered", next_action);
                self.holding_buffer
                    .retain(|item| item.state != TapHoldState::WaitingCombo);
            }
            next_action
        } else {
            if !key_event.pressed {
                let mut combo_output = None;
                let mut releasing_triggered_combo = false;

                for combo in self.keymap.borrow_mut().behavior.combo.combos.iter_mut() {
                    if combo.actions.contains(&key_action) {
                        // Releasing a combo key in triggered combo
                        releasing_triggered_combo |= combo.is_triggered();

                        // Release the combo key, check whether the combo is fully released
                        if combo.update_released(key_action) {
                            // If the combo is fully released, update the combo output
                            debug!("[COMBO] {:?} is released", combo.output);
                            combo_output = combo_output.or(Some(combo.output));
                        }
                    }
                }

                // Releasing a triggered combo
                // - Return the output of the triggered combo when the combo is fully released
                // - Return None when the combo is not fully released yet
                if releasing_triggered_combo {
                    return combo_output;
                }
            }

            self.dispatch_combos().await;
            Some(key_action)
        }
    }

    // Dispatch combo into key action
    async fn dispatch_combos(&mut self) {
        // For each WaitingCombo in the holding buffer, dispatch it
        // Note that the process_key_action_inner is an async function, so the retain doesn't work
        let mut i = 0;
        while i < self.holding_buffer.len() {
            if self.holding_buffer[i].state == TapHoldState::WaitingCombo {
                let key = self.holding_buffer.swap_remove(i);
                debug!("[COMBO] Dispatching combo: {:?}", key);
                self.process_key_action_inner(key.action, key.key_event).await;
            } else {
                i += 1;
            }
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

    // sort buffer order by start_time ASC
    fn push_and_sort_buffers(&mut self, item: HoldingKey) {
        if let Err(e) = self.holding_buffer.push(item) {
            error!("Holding buffer overflowed, cannot save: {:?}", e);
        }
        self.sort_buffers();
    }

    fn sort_buffers(&mut self) {
        self.holding_buffer.sort_unstable_by(|l, r| {
            if l.pressed_time.as_millis() >= r.pressed_time.as_millis() {
                Ordering::Greater
            } else {
                Ordering::Less
            }
        });
    }

    // Release hold key
    async fn release_tap_hold_key(&mut self, key_event: KeyEvent) {
        debug!("[TAP-HOLD] On Releasing: tap-hold key event {:?}", key_event);

        let col = key_event.col as usize;
        let row = key_event.row as usize;

        trace!("[TAP-HOLD] current buffer queue to process {:?}", self.holding_buffer);

        // While tap hold key is releasing, pressed key event should be updating into PostTap or PostHold state
        if let Some(hold_key) = self.remove_holding_key_from_buffer(key_event) {
            if let KeyAction::TapHold(tap_action, hold_action) = hold_key.action {
                match hold_key.state {
                    TapHoldState::BeforeHold | TapHoldState::PostHold => {
                        debug!(
                            "[TAP-HOLD] {:?} releasing with key event {:?}",
                            hold_key.state, key_event
                        );
                        self.process_key_action_normal(hold_action, key_event).await;
                    }
                    TapHoldState::PostTap => {
                        debug!(
                            "TapHold {:?}] post Tapping, releasing {:?}",
                            hold_key.key_event, tap_action
                        );
                        // The tap-hold key is already "pressed" as tap, release it here.
                        // This is a special case, because the "tap_action" isn't tapped, it's triggered by "pressing" the tap-action
                        self.process_key_action_normal(tap_action, key_event).await;
                    }
                    TapHoldState::Initial => {
                        // Release tap-hold key as tap action
                        debug!(
                            "[TAP-HOLD] quick release should be tapping, send tap action, {:?}",
                            tap_action
                        );
                        // Use hold_key.key_event(whose pressed value should be true) to process tap action
                        self.process_key_action_tap(tap_action, hold_key.key_event).await;
                    }
                    _ => {
                        error!(
                            "[TAP-HOLD] Unexpected TapHoldState {:?}, while releasing {:?}",
                            hold_key.state, hold_key.key_event
                        );
                    }
                }
            }
        }

        // Clear timer
        self.timer[col][row] = None;
        debug!("[TAP-HOLD] tap-hold key event {:?}, cleanup done", key_event);
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
            Action::TriggerMacro(macro_idx) => self.execute_macro(macro_idx, key_event).await,
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
        debug!("TAP action: {:?}, {:?}", action, key_event);

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
                // Ignore hold within pre idle time for quick typing
                if key_event.pressed {
                    if last_release_time.elapsed() < self.keymap.borrow().behavior.tap_hold.prior_idle_time
                        && !(key_event.row == self.last_release.0.row && key_event.col == self.last_release.0.col)
                    {
                        // The previous key is a different key and released within `prior_idle_time`, it's in key streak
                        debug!("Key streak detected, trigger tap action");
                        self.process_key_action_normal(tap_action, key_event).await;

                        // Push into buffer, process by order in loop
                        self.add_holding_key_to_buffer(
                            key_event,
                            KeyAction::TapHold(tap_action, hold_action),
                            TapHoldState::PostTap,
                        );
                        return;
                    } else if last_release_time.elapsed() < self.keymap.borrow().behavior.tap_hold.hold_timeout
                        && key_event.row == self.last_release.0.row
                        && key_event.col == self.last_release.0.col
                    {
                        // Quick tapping to repeat
                        debug!("Pressed a same tap-hold key after tapped it within `hold_timeout`");

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

        // New hold key pressed, should push into unreleased events
        if key_event.pressed {
            // Save unprocessed key
            self.add_holding_key_to_buffer(key_event, KeyAction::TapHold(tap_action, hold_action), Initial);
        } else {
            // Release a tap-hold key, should check timeout for tap

            // Find holding_after_tap key_event
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
            } else {
                // Check unreleased event and remove key with same rol and col
                self.release_tap_hold_key(key_event).await;
            }
        }
    }

    /// Process one shot action.
    async fn process_key_action_oneshot(&mut self, oneshot_action: Action, key_event: KeyEvent) {
        match oneshot_action {
            Action::Modifier(m) => {
                self.process_action_osm(m.to_hid_modifiers(), key_event).await;
                // Process OSL to avoid the OSM state stuck when an OSM is followed by an OSL
                self.update_osl(key_event);
            }
            Action::LayerOn(l) => {
                self.process_action_osl(l, key_event).await;
                // Process OSM to avoid the OSL state stuck when an OSL is followed by an OSM
                self.update_osm(key_event);
            }
            _ => self.process_key_action_normal(oneshot_action, key_event).await,
        }
    }

    /// Process tap dance action.
    async fn process_key_action_tap_dance(&mut self, index: u8, _key_event: KeyEvent) {
        let tap_dances = &self.keymap.borrow().behavior.tap_dance.tap_dances;

        if let Some(tap_dance) = tap_dances.get(index as usize) {
            // TODO: Implement full tap dance functionality with timing, double tap, hold, etc.
            warn!("Tap dance index {}: {:?} is triggered", index, tap_dance);
        } else {
            warn!("Tap dance index {} not found", index);
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

                    let timeout = Timer::after(self.keymap.borrow().behavior.one_shot.timeout);
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
            debug!("Repeat last key code: {:?} , {:?}", key, key_event);
        } else if key_event.pressed {
            //TODO should save releasing key only
            debug!(
                "Last key code changed from {:?} to {:?}(pressed: {:?})",
                self.last_key_code, key, key_event.pressed
            );
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

    /// Calculates the combined effect of "explicit modifiers":
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
        // Text typing macro should not be affected by any modifiers,
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
        for fork_state in self.fork_states.iter().flatten() {
            fork_suppress |= fork_state.suppress;
        }

        // Some of these suppressions could have been canceled after the fork activation
        // by "explicit" modifier key presses - fork_keep_mask collects these:
        fork_suppress &= !self.fork_keep_mask;

        // Execute the remaining suppressions
        result &= !fork_suppress;

        // Apply the modifiers from KeyAction::WithModifiers
        // the suppression effect of forks should not apply on these
        result |= self.with_modifiers;

        result
    }

    // Process a basic keypress/release and also take care of applying one shot modifiers
    async fn process_basic(&mut self, key: KeyCode, key_event: KeyEvent) {
        if key_event.pressed {
            self.register_key(key, key_event);
        } else {
            self.unregister_key(key, key_event);
        }

        self.send_keyboard_report_with_resolved_modifiers(key_event.pressed)
            .await;
    }

    // Process action key
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

    /// Process mouse key action with acceleration support.
    async fn process_action_mouse(&mut self, key: KeyCode, key_event: KeyEvent) {
        if key.is_mouse_key() {
            if key_event.pressed {
                match key {
                    KeyCode::MouseUp => {
                        // Reset repeat counter for direction change
                        if self.mouse_report.y > 0 {
                            self.mouse_repeat = 0;
                        }
                        let unit = self.calculate_mouse_move_unit();
                        self.mouse_report.y = -unit;
                    }
                    KeyCode::MouseDown => {
                        if self.mouse_report.y < 0 {
                            self.mouse_repeat = 0;
                        }
                        let unit = self.calculate_mouse_move_unit();
                        self.mouse_report.y = unit;
                    }
                    KeyCode::MouseLeft => {
                        if self.mouse_report.x > 0 {
                            self.mouse_repeat = 0;
                        }
                        let unit = self.calculate_mouse_move_unit();
                        self.mouse_report.x = -unit;
                    }
                    KeyCode::MouseRight => {
                        if self.mouse_report.x < 0 {
                            self.mouse_repeat = 0;
                        }
                        let unit = self.calculate_mouse_move_unit();
                        self.mouse_report.x = unit;
                    }
                    KeyCode::MouseWheelUp => {
                        if self.mouse_report.wheel < 0 {
                            self.mouse_wheel_repeat = 0;
                        }
                        let unit = self.calculate_mouse_wheel_unit();
                        self.mouse_report.wheel = unit;
                    }
                    KeyCode::MouseWheelDown => {
                        if self.mouse_report.wheel > 0 {
                            self.mouse_wheel_repeat = 0;
                        }
                        let unit = self.calculate_mouse_wheel_unit();
                        self.mouse_report.wheel = -unit;
                    }
                    KeyCode::MouseWheelLeft => {
                        if self.mouse_report.pan > 0 {
                            self.mouse_wheel_repeat = 0;
                        }
                        let unit = self.calculate_mouse_wheel_unit();
                        self.mouse_report.pan = -unit;
                    }
                    KeyCode::MouseWheelRight => {
                        if self.mouse_report.pan < 0 {
                            self.mouse_wheel_repeat = 0;
                        }
                        let unit = self.calculate_mouse_wheel_unit();
                        self.mouse_report.pan = unit;
                    }
                    KeyCode::MouseBtn1 => self.mouse_report.buttons |= 1 << 0,
                    KeyCode::MouseBtn2 => self.mouse_report.buttons |= 1 << 1,
                    KeyCode::MouseBtn3 => self.mouse_report.buttons |= 1 << 2,
                    KeyCode::MouseBtn4 => self.mouse_report.buttons |= 1 << 3,
                    KeyCode::MouseBtn5 => self.mouse_report.buttons |= 1 << 4,
                    KeyCode::MouseBtn6 => self.mouse_report.buttons |= 1 << 5,
                    KeyCode::MouseBtn7 => self.mouse_report.buttons |= 1 << 6,
                    KeyCode::MouseBtn8 => self.mouse_report.buttons |= 1 << 7,
                    KeyCode::MouseAccel0 => {
                        self.mouse_accel |= 1 << 0;
                    }
                    KeyCode::MouseAccel1 => {
                        self.mouse_accel |= 1 << 1;
                    }
                    KeyCode::MouseAccel2 => {
                        self.mouse_accel |= 1 << 2;
                    }
                    _ => {}
                }
            } else {
                match key {
                    KeyCode::MouseUp => {
                        if self.mouse_report.y < 0 {
                            self.mouse_report.y = 0;
                        }
                    }
                    KeyCode::MouseDown => {
                        if self.mouse_report.y > 0 {
                            self.mouse_report.y = 0;
                        }
                    }
                    KeyCode::MouseLeft => {
                        if self.mouse_report.x < 0 {
                            self.mouse_report.x = 0;
                        }
                    }
                    KeyCode::MouseRight => {
                        if self.mouse_report.x > 0 {
                            self.mouse_report.x = 0;
                        }
                    }
                    KeyCode::MouseWheelUp => {
                        if self.mouse_report.wheel > 0 {
                            self.mouse_report.wheel = 0;
                        }
                    }
                    KeyCode::MouseWheelDown => {
                        if self.mouse_report.wheel < 0 {
                            self.mouse_report.wheel = 0;
                        }
                    }
                    KeyCode::MouseWheelLeft => {
                        if self.mouse_report.pan < 0 {
                            self.mouse_report.pan = 0;
                        }
                    }
                    KeyCode::MouseWheelRight => {
                        if self.mouse_report.pan > 0 {
                            self.mouse_report.pan = 0;
                        }
                    }
                    KeyCode::MouseBtn1 => self.mouse_report.buttons &= !(1 << 0),
                    KeyCode::MouseBtn2 => self.mouse_report.buttons &= !(1 << 1),
                    KeyCode::MouseBtn3 => self.mouse_report.buttons &= !(1 << 2),
                    KeyCode::MouseBtn4 => self.mouse_report.buttons &= !(1 << 3),
                    KeyCode::MouseBtn5 => self.mouse_report.buttons &= !(1 << 4),
                    KeyCode::MouseBtn6 => self.mouse_report.buttons &= !(1 << 5),
                    KeyCode::MouseBtn7 => self.mouse_report.buttons &= !(1 << 6),
                    KeyCode::MouseBtn8 => self.mouse_report.buttons &= !(1 << 7),
                    KeyCode::MouseAccel0 => {
                        self.mouse_accel &= !(1 << 0);
                    }
                    KeyCode::MouseAccel1 => {
                        self.mouse_accel &= !(1 << 1);
                    }
                    KeyCode::MouseAccel2 => {
                        self.mouse_accel &= !(1 << 2);
                    }
                    _ => {}
                }

                // Reset repeat counters when movement stops
                if self.mouse_report.x == 0 && self.mouse_report.y == 0 {
                    self.mouse_repeat = 0;
                }
                if self.mouse_report.wheel == 0 && self.mouse_report.pan == 0 {
                    self.mouse_wheel_repeat = 0;
                }

                // Clear all mouse keys in the KEY_EVENT_CHANNEL
                let len = KEY_EVENT_CHANNEL.len();
                for _ in 0..len {
                    let queued_event = KEY_EVENT_CHANNEL.receive().await;
                    if queued_event.col != key_event.col || queued_event.row != key_event.row {
                        KEY_EVENT_CHANNEL.send(queued_event).await;
                    }
                }
            }

            // Apply diagonal compensation for movement
            if self.mouse_report.x != 0 && self.mouse_report.y != 0 {
                let (x, y) = self.apply_diagonal_compensation(self.mouse_report.x, self.mouse_report.y);
                self.mouse_report.x = x;
                self.mouse_report.y = y;
            }

            // Apply diagonal compensation for wheel
            if self.mouse_report.wheel != 0 && self.mouse_report.pan != 0 {
                let (wheel, pan) = self.apply_diagonal_compensation(self.mouse_report.wheel, self.mouse_report.pan);
                self.mouse_report.wheel = wheel;
                self.mouse_report.pan = pan;
            }

            self.send_mouse_report().await;

            // Continue processing ONLY for movement and wheel keys
            if key_event.pressed {
                let is_movement_key = matches!(
                    key,
                    KeyCode::MouseUp | KeyCode::MouseDown | KeyCode::MouseLeft | KeyCode::MouseRight
                );
                let is_wheel_key = matches!(
                    key,
                    KeyCode::MouseWheelUp
                        | KeyCode::MouseWheelDown
                        | KeyCode::MouseWheelLeft
                        | KeyCode::MouseWheelRight
                );

                // Only continue processing for movement and wheel keys
                if is_movement_key || is_wheel_key {
                    // Determine the delay for the next repeat using convenience methods
                    let delay = {
                        let config = &self.keymap.borrow().behavior.mouse_key;
                        if is_movement_key {
                            config.get_movement_delay(self.mouse_repeat)
                        } else {
                            config.get_wheel_delay(self.mouse_wheel_repeat)
                        }
                    };

                    // Increment the appropriate repeat counter
                    if is_movement_key && self.mouse_repeat < u8::MAX {
                        self.mouse_repeat += 1;
                    }
                    if is_wheel_key && self.mouse_wheel_repeat < u8::MAX {
                        self.mouse_wheel_repeat += 1;
                    }

                    // Schedule next movement after the delay
                    embassy_time::Timer::after_millis(delay as u64).await;

                    KEY_EVENT_CHANNEL.send(key_event).await;
                }
            }
        }
    }

    async fn process_user(&mut self, key: KeyCode, key_event: KeyEvent) {
        debug!("Processing user key: {:?}, event: {:?}", key, key_event);
        #[cfg(feature = "_ble")]
        {
            use crate::ble::trouble::profile::BleProfileAction;
            use crate::channel::BLE_PROFILE_CHANNEL;
            use crate::NUM_BLE_PROFILE;
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
        // Get macro index
        if let Some(macro_idx) = key.as_macro_index() {
            self.execute_macro(macro_idx, key_event).await;
        }
    }

    async fn execute_macro(&mut self, macro_idx: u8, key_event: KeyEvent) {
        // Execute the macro only when releasing the key
        if key_event.pressed {
            return;
        }

        // Read macro operations until the end of the macro
        let macro_idx = self.keymap.borrow().get_macro_sequence_start(macro_idx);
        if let Some(macro_start_idx) = macro_idx {
            let mut offset = 0;
            loop {
                // First, get the next macro operation
                let (operation, new_offset) = self.keymap.borrow().get_next_macro_operation(macro_start_idx, offset);
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
                if offset > self.keymap.borrow().behavior.keyboard_macros.macro_sequences.len() {
                    break;
                }
            }
        } else {
            error!("Macro not found");
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

    /// ## On timeout decision
    /// When a tap-hold key_event is reach timeout
    /// - Current key press down as hold and become PostHold
    /// - Ignore all tap-hold keys, keep they state and timer
    /// - For all non tap-hold keys pressed, trigger their tap action.
    ///
    /// ## Other decision
    /// When a non-tap-hold key_event is released while the permissive-hold feature is enabled:
    /// - If the corresponding key_action is a tap-hold, check that it is not already in a TapHold* state.
    /// - For all tap-hold keys pressed before this event, trigger their hold action.
    /// - For all tap-hold keys pressed after this event, trigger their tap action.
    /// - Other keys should send as normal
    ///
    /// This function forces all buffered keys to resolve immediately, ignoring their timeouts.
    async fn fire_holding_keys(&mut self, reason: TapHoldDecision, key_event: KeyEvent) {
        // Press time of current key
        let pressed_time: Instant = if let Some(inst) = self.timer[key_event.col as usize][key_event.row as usize] {
            inst
        } else {
            // Fire all
            Instant::now()
        };

        let hold_keys_to_flush: Vec<_, HOLD_BUFFER_SIZE> = self
            .holding_buffer
            .iter()
            .enumerate()
            .filter_map(|(pos, e)| match reason {
                TapHoldDecision::Timeout => {
                    // A tap-hold key is timeout, flush current timeout keys and all other tapped normal keys
                    if e.state() == Initial {
                        if (e.key_event.col == key_event.col && e.key_event.row == key_event.row)
                            || (!matches!(e.action, KeyAction::TapHold(..)))
                        {
                            Some(pos)
                        } else {
                            // Exclude other tap-hold keys
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => {
                    // CleanBuffer/Hold/ChordHold: fire all keys in Initial state in the buffer
                    if e.state() == Initial {
                        Some(pos)
                    } else {
                        None
                    }
                }
            })
            .collect();

        if hold_keys_to_flush.is_empty() {
            debug!("non tap-hold-key hold before current release key, ignore and skip");
            return;
        } else {
            debug!(
                "[TAP-HOLD] Flush keys {:?} in {:?}",
                hold_keys_to_flush, self.holding_buffer,
            );
        }

        // Iterate buffer twice, since i just can borrow self twice
        for pos in hold_keys_to_flush {
            // First, trigger keys in holding buffer
            if let Some(hold_key) = self.holding_buffer.get(pos) {
                match hold_key.action {
                    KeyAction::TapHold(tap_action, hold_action) => {
                        if hold_key.key_event.col == key_event.col && hold_key.key_event.row == key_event.row {
                            // The current tap-hold key updating
                            let action = if reason == TapHoldDecision::Timeout {
                                hold_action
                            } else {
                                tap_action
                            };
                            debug!("Current Key {:?} become {:?}", hold_key.key_event, action);
                            self.process_key_action_normal(action, hold_key.key_event).await;
                        } else if hold_key.state == Initial && hold_key.pressed_time < pressed_time {
                            debug!("Key {:?} become {:?}", hold_key.key_event, hold_action);
                            self.process_key_action_normal(hold_action, hold_key.key_event).await;
                        } else if hold_key.state == Initial && hold_key.pressed_time >= pressed_time {
                            debug!("Key {:?} become {:?}", hold_key.key_event, tap_action);
                            self.process_key_action_normal(tap_action, hold_key.key_event).await;
                        } else {
                            debug!(
                                "ignore : {:?}, pressed_time {}",
                                hold_key,
                                hold_key.pressed_time.as_millis()
                            );
                        }
                    }
                    _ => {
                        let action = self.keymap.borrow_mut().get_action_with_layer_cache(hold_key.key_event);
                        debug!("Tap Key {:?} now press down, action: {:?}", hold_key.key_event, action);
                        // TODO: ignored return value
                        self.process_key_action_inner(action, hold_key.key_event).await;
                    }
                }
            }

            // Second, update state of the key in the buffer
            // This ensures that the buffer accurately reflects which keys have been resolved as tap or hold,
            // so that subsequent processing (e.g., releases or further key events) can handle them correctly.
            if let Some(hold_key) = self.holding_buffer.get_mut(pos) {
                match hold_key.action {
                    KeyAction::TapHold(tap_action, hold_action) => {
                        if hold_key.key_event.col == key_event.col && hold_key.key_event.row == key_event.row {
                            // This is the key that triggered the flush; mark as PostTap (tap resolved).
                            if reason == TapHoldDecision::Timeout {
                                debug!("Current Key {:?} mark {:?}", hold_key.key_event, hold_action);
                                hold_key.state = TapHoldState::PostHold;
                            } else {
                                debug!("Current Key {:?} mark {:?}", hold_key.key_event, tap_action);
                                hold_key.state = TapHoldState::PostTap;
                            };
                        } else if hold_key.state == Initial && hold_key.pressed_time < pressed_time {
                            // This key was pressed before the triggering key; mark as PostHold (hold resolved).
                            debug!("Key {:?} become {:?}", hold_key.key_event, hold_action);
                            hold_key.state = TapHoldState::PostHold;
                        } else if hold_key.state == Initial && hold_key.pressed_time >= pressed_time {
                            // This key was pressed after or at the same time as the triggering key; mark as PostTap.
                            debug!("Key {:?} become {:?}", hold_key.key_event, tap_action);
                            hold_key.state = TapHoldState::PostTap;
                        } else {
                            // No state change needed; already resolved.
                            debug!(
                                "ignore : {:?}, pressed_time {}",
                                hold_key,
                                hold_key.pressed_time.as_millis()
                            );
                        }
                    }
                    _ => {
                        // For non-tap-hold keys, mark as PostTap to indicate they've been processed.
                        debug!("Tap Key {:?} now marked as PostTap", hold_key.key_event);
                        hold_key.state = TapHoldState::PostTap;
                    }
                }
            }
        }

        debug!(
            "[TAP-HOLD] After flush keys, current hold buffer: {:?}",
            self.holding_buffer
        );

        // Reset chord state
        self.chord_state = None;
    }

    /// When a key is pressed, add it to the holding buffer,
    /// if the current key action is a tap-hold, or the evaluation of current key action should be deferred by tap-hold.
    fn add_holding_key_to_buffer(&mut self, key_event: KeyEvent, action: KeyAction, state: TapHoldState) {
        let pressed_time = self.timer[key_event.col as usize][key_event.row as usize].unwrap_or(Instant::now());
        let new_item = HoldingKey {
            state,
            key_event,
            pressed_time,
            action,
        };
        debug!("Saving action: {:?} to holding buffer", new_item);
        self.push_and_sort_buffers(new_item);
        match action {
            KeyAction::TapHold(_, _) => {
                // If this is the first tap-hold key, initialize the chord state for possible chordal hold detection.
                if self.chord_state.is_none() {
                    self.chord_state = Some(ChordHoldState::create(key_event, ROW, COL));
                }
            }
            _ => {}
        }
    }

    /// Finds the holding key in the buffer that matches the given key_event.
    ///
    /// This function searches for both TapHold and Others key kinds, but is primarily
    /// intended for use with tap-hold keys. Returns the holding key if a matching key is found,
    /// otherwise returns None.
    fn remove_holding_key_from_buffer(&mut self, key_event: KeyEvent) -> Option<HoldingKey> {
        if let Some(i) = self
            .holding_buffer
            .iter()
            .position(|e| e.key_event.row == key_event.row && e.key_event.col == key_event.col)
        {
            Some(self.holding_buffer.swap_remove(i))
        } else {
            None
        }
    }

    /// Get a copy of the next tap-hold key in the buffer.
    // TODO: improve performance
    fn next_buffered_key(&mut self) -> Option<HoldingKey> {
        // Release an unprocessed key
        self.holding_buffer
            .iter()
            .filter_map(|key| {
                if key.state == Initial || key.state == TapHoldState::WaitingCombo {
                    Some(key.clone())
                } else {
                    None
                }
            }) // Now only tap-hold keys are considered actually
            .min_by_key(|e| e.pressed_time) // TODO: If per-key timeout is added, sort by the timeout time here
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

        #[cfg(feature = "controller")]
        send_controller_event(
            &mut self.controller_pub,
            ControllerEvent::Modifier(ModifierCombination::from_hid_modifiers(self.held_modifiers)),
        );

        // if a modifier key arrives after fork activation, it should be kept
        self.fork_keep_mask |= key.to_hid_modifiers();
    }

    /// Unregister a modifier from hid report.
    fn unregister_modifier_key(&mut self, key: KeyCode) {
        self.held_modifiers &= !key.to_hid_modifiers();

        #[cfg(feature = "controller")]
        send_controller_event(
            &mut self.controller_pub,
            ControllerEvent::Modifier(ModifierCombination::from_hid_modifiers(self.held_modifiers)),
        );
    }

    /// Register a modifier combination to be sent in hid report.
    fn register_modifiers(&mut self, modifiers: ModifierCombination) {
        self.held_modifiers |= modifiers.to_hid_modifiers();

        #[cfg(feature = "controller")]
        send_controller_event(
            &mut self.controller_pub,
            ControllerEvent::Modifier(ModifierCombination::from_hid_modifiers(self.held_modifiers)),
        );

        // if a modifier key arrives after fork activation, it should be kept
        self.fork_keep_mask |= modifiers.to_hid_modifiers();
    }

    /// Unregister a modifier combination from hid report.
    fn unregister_modifiers(&mut self, modifiers: ModifierCombination) {
        self.held_modifiers &= !modifiers.to_hid_modifiers();

        #[cfg(feature = "controller")]
        send_controller_event(
            &mut self.controller_pub,
            ControllerEvent::Modifier(ModifierCombination::from_hid_modifiers(self.held_modifiers)),
        );
    }

    /// Calculate mouse movement distance based on current repeat count and acceleration settings
    fn calculate_mouse_move_unit(&self) -> i8 {
        let config = &self.keymap.borrow().behavior.mouse_key;

        let unit = if self.mouse_accel & (1 << 2) != 0 {
            20
        } else if self.mouse_accel & (1 << 1) != 0 {
            12
        } else if self.mouse_accel & (1 << 0) != 0 {
            4
        } else if self.mouse_repeat == 0 {
            config.move_delta as u16
        } else if self.mouse_repeat >= config.time_to_max {
            (config.move_delta as u16).saturating_mul(config.max_speed as u16)
        } else {
            // Natural acceleration with smooth unit progression.
            // Calculate smooth progress using asymptotic curve: f(x) = 2x - x².
            // Where x = repeat_count / time_to_max, giving smooth progression from 0 to 1
            let repeat_count = self.mouse_repeat as u16;
            let time_to_max = config.time_to_max as u16;
            let min_unit = config.move_delta as u16;
            let max_unit = (config.move_delta as u16).saturating_mul(config.max_speed as u16);
            let unit_range = max_unit - min_unit;

            // Use saturating operations to handle overflow cases
            let linear_term = 2u16.saturating_mul(repeat_count).saturating_mul(time_to_max);
            let quadratic_term = repeat_count.saturating_mul(repeat_count);
            let progress_numerator = linear_term.saturating_sub(quadratic_term);
            let progress_denominator = time_to_max.saturating_mul(time_to_max);
            min_unit + (unit_range.saturating_mul(progress_numerator) / progress_denominator.max(1))
        };

        let final_unit = if unit > config.move_max as u16 {
            config.move_max as u16
        } else if unit == 0 {
            1
        } else {
            unit
        };

        final_unit.min(i8::MAX as u16) as i8
    }

    /// Calculate mouse wheel movement distance based on current repeat count and acceleration settings
    fn calculate_mouse_wheel_unit(&self) -> i8 {
        let config = &self.keymap.borrow().behavior.mouse_key;

        let unit = if self.mouse_accel & (1 << 2) != 0 {
            4
        } else if self.mouse_accel & (1 << 1) != 0 {
            2
        } else if self.mouse_accel & (1 << 0) != 0 {
            1
        } else if self.mouse_wheel_repeat == 0 {
            config.wheel_delta as u16
        } else if self.mouse_wheel_repeat >= config.wheel_time_to_max {
            (config.wheel_delta as u16).saturating_mul(config.wheel_max_speed_multiplier as u16)
        } else {
            // Natural acceleration with smooth unit progression.
            let repeat_count = self.mouse_wheel_repeat as u16;
            let time_to_max = config.wheel_time_to_max as u16;
            let min_unit = config.wheel_delta as u16;
            let max_unit = (config.wheel_delta as u16).saturating_mul(config.wheel_max_speed_multiplier as u16);
            let unit_range = max_unit - min_unit;

            // Calculate smooth progress using asymptotic curve: f(x) = 2x - x².
            // Use saturating operations to handle overflow cases.
            let linear_term = 2u16.saturating_mul(repeat_count).saturating_mul(time_to_max);
            let quadratic_term = repeat_count.saturating_mul(repeat_count);
            let progress_numerator = linear_term.saturating_sub(quadratic_term);
            let progress_denominator = time_to_max.saturating_mul(time_to_max);

            min_unit + (unit_range.saturating_mul(progress_numerator) / progress_denominator.max(1))
        };

        let final_unit = if unit > config.wheel_max as u16 {
            config.wheel_max as u16
        } else if unit == 0 {
            1
        } else {
            unit
        };

        final_unit.min(i8::MAX as u16) as i8
    }

    /// Apply diagonal movement compensation (approximation of 1/sqrt(2))
    fn apply_diagonal_compensation(&self, mut x: i8, mut y: i8) -> (i8, i8) {
        if x != 0 && y != 0 {
            // Apply 1/sqrt(2) approximation using 181/256 (0.70703125)
            let x_compensated = (x as i16 * 181 + 128) / 256;
            let y_compensated = (y as i16 * 181 + 128) / 256;

            x = if x_compensated == 0 && x != 0 {
                if x > 0 {
                    1
                } else {
                    -1
                }
            } else {
                x_compensated as i8
            };

            y = if y_compensated == 0 && y != 0 {
                if y > 0 {
                    1
                } else {
                    -1
                }
            } else {
                y_compensated as i8
            };
        }
        (x, y)
    }
}

#[cfg(test)]
mod test {

    use embassy_futures::block_on;
    use embassy_time::{Duration, Timer};
    use rusty_fork::rusty_fork_test;

    use super::*;
    use crate::action::KeyAction;
    use crate::config::{BehaviorConfig, CombosConfig, ForksConfig};
    use crate::fork::Fork;
    use crate::hid_state::HidModifiers;
    use crate::{a, k, layer, mo, th};

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
                [k!(Escape), th!(A, LShift), th!(S, LGui), k!(D), k!(F), k!(G), k!(H), k!(J), k!(K), k!(L), k!(Semicolon), k!(Quote), a!(No), k!(Enter)],
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

    #[rustfmt::skip]
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

        let keymap = block_on(KeyMap::new(leaked_keymap, None, config));
        let keymap_cell = RefCell::new(keymap);
        let keymap_ref = Box::leak(Box::new(keymap_cell));

        Keyboard::new(keymap_ref)
    }

    fn create_test_keyboard() -> Keyboard<'static, 5, 14, 2> {
        create_test_keyboard_with_config(BehaviorConfig::default())
    }

    async fn force_timeout_first_hold(keyboard: &mut Keyboard<'static, 5, 14, 2>) {
        let key = keyboard.next_buffered_key().unwrap();
        keyboard.process_buffered_key(key).await;
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

        #[test]
        fn test_basic_key_press_release() {
            let main = async {
                let mut keyboard = create_test_keyboard();

                // Press A key
                keyboard.process_inner(key_event(0, 0, true)).await;
                assert_eq!(keyboard.held_keycodes[0], KeyCode::Grave); // A key's HID code is 0x04

                // Release A key
                keyboard.process_inner(key_event(0, 0, false)).await;
                assert_eq!(keyboard.held_keycodes[0], KeyCode::No);
            };
            block_on(main);
        }

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

        #[test]
        fn test_multiple_keys() {
            let main = async {
                let mut keyboard = create_test_keyboard();

                keyboard.process_inner(key_event(0, 0, true)).await;
                assert!(keyboard.held_keycodes.contains(&KeyCode::Grave));

                keyboard.process_inner(key_event(1, 0, true)).await;
                assert!(keyboard.held_keycodes.contains(&KeyCode::Grave) && keyboard.held_keycodes.contains(&KeyCode::Tab));

                keyboard.process_inner(key_event(1, 0, false)).await;
                assert!(keyboard.held_keycodes.contains(&KeyCode::Grave) && !keyboard.held_keycodes.contains(&KeyCode::Tab));

                keyboard.process_inner(key_event(0, 0, false)).await;
                assert!(!keyboard.held_keycodes.contains(&KeyCode::Grave));
                assert!(keyboard.held_keycodes.iter().all(|&k| k == KeyCode::No));
            };

            block_on(main);
        }

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
                keyboard.process_inner(key_event(2, 0, true)).await;
                assert_eq!(keyboard.held_keycodes[0], KeyCode::Escape); // A key's HID code is 0x04

                // Release A key
                keyboard.process_inner(key_event(2, 0, false)).await;
                assert_eq!(keyboard.held_keycodes[0], KeyCode::No);

                // after another key is pressed, that key is repeated
                keyboard.process_inner(key_event(0, 0, true)).await;
                assert_eq!(keyboard.held_keycodes[0], KeyCode::Escape); // A key's HID code is 0x04

                // releasing the repeat key
                keyboard.process_inner(key_event(0, 0, false)).await;
                assert_eq!(keyboard.held_keycodes[0], KeyCode::No); // A key's HID code is 0x04

                // Press S key
                keyboard.process_inner(key_event(1, 2, true)).await;
                assert_eq!(keyboard.held_keycodes[0], KeyCode::W); // A key's HID code is 0x04

                // after another key is pressed, that key is repeated
                keyboard.process_inner(key_event(0, 0, true)).await;
                assert_eq!(keyboard.held_keycodes[0], KeyCode::W); // A key's HID code is 0x04
            };
            block_on(main);
        }


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
                keyboard.keymap.borrow_mut().set_action_at(
                    2,
                    1,
                    0,
                    KeyAction::Single(Action::Key(KeyCode::A)),
                );
                keyboard.keymap.borrow_mut().set_action_at(
                    2,
                    2,
                    0,
                    KeyAction::Single(Action::Key(KeyCode::S)),
                );

                // Press down F
                // first press ever of the Again issues KeyCode:No
                keyboard.process_inner(key_event(0, 0, true)).await;
                keyboard
                    .send_keyboard_report_with_resolved_modifiers(true)
                    .await;
                assert_eq!(keyboard.held_keycodes[0], KeyCode::No); // A key's HID code is 0x04
                // Release F
                keyboard.process_inner(key_event(0, 0, false)).await;

                // Press A key
                keyboard.process_inner(key_event(2, 1, true)).await;
                assert_eq!(keyboard.held_keycodes[0], KeyCode::A); // A key's HID code is 0x04

                // Release A key
                keyboard.process_inner(key_event(2, 1, false)).await;
                assert_eq!(keyboard.held_keycodes[0], KeyCode::No);

                // Release F
                keyboard.process_inner(key_event(0, 0, false)).await;

                // Here release event should make again into hold


                embassy_time::Timer::after_millis(200 as u64).await;
                // after another key is pressed, that key is repeated
                keyboard.process_inner(key_event(0, 0, true)).await;
                force_timeout_first_hold(&mut keyboard).await;

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

                // disable th on a
                keyboard.keymap.borrow_mut().set_action_at(
                    2,
                    1,
                    0,
                    KeyAction::Single(Action::Key(KeyCode::A)),
                );


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
                    HidModifiers::new().with_left_ctrl(true).with_left_shift(true)
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
                    HidModifiers::new().with_left_ctrl(true).with_left_shift(true)
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
