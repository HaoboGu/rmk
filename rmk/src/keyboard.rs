use crate::action::{Action, KeyAction};
use crate::input_device::Runnable;
use crate::keyboard_macros::MacroOperation;

use crate::combo::Combo;
use crate::descriptor::{KeyboardReport, ViaReport};
use crate::event::KeyEvent;
use crate::fork::{ActiveFork, StateBits};
use crate::hid::Report;
use crate::hid_state::{HidModifiers, HidMouseButtons};
use crate::keyboard::LoopState::{Queue, Stop, OK};
use crate::keycode::{KeyCode, ModifierCombination};
use crate::keymap::KeyMap;
use crate::light::LedIndicator;
use crate::tap_hold::ChordHoldState;
use crate::tap_hold::HoldDecision::{ChordHold, CleanBuffer, Hold};
use crate::tap_hold::{
    HoldDecision, HoldingKey, HoldingKeyTrait, KeyKind, TapHoldState, TapHoldState::PostHold, TapHoldTimer,
};
use core::cell::RefCell;
use core::cmp::Ordering;
use core::fmt::Debug;

use embassy_futures::select::{select, Either};
use embassy_futures::{join, yield_now};
use embassy_time::{Duration, Instant, Timer};
use heapless::{Deque, FnvIndexMap, Vec};
use usbd_hid::descriptor::{MediaKeyboardReport, MouseReport, SystemControlReport};
#[cfg(feature = "controller")]
use {
    crate::channel::{ControllerPub, CONTROLLER_CHANNEL},
    crate::event::ControllerEvent,
};

use HoldDecision::{Buffering, Ignore};
use TapHoldState::Initial;
#[cfg(feature = "controller")]
use {
    crate::channel::{send_controller_event, ControllerPub, CONTROLLER_CHANNEL},
    crate::event::ControllerEvent,
};

use crate::channel::{KEYBOARD_REPORT_CHANNEL, KEY_EVENT_CHANNEL};
use crate::{boot, COMBO_MAX_LENGTH, FORK_MAX_NUM};

#[derive(Debug)]
enum LoopState {
    OK,    // default state, fire and forgot current key event
    Queue, // save current event into buffer
    Flush, // flush event buffer
    Stop,  // stop keyboard running
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
        let mut buffering = false;
        loop {
            debug!("[RUN] new turn with buffer: {:?}", self.holding_buffer);
            //TODO add post time wait timer here
            let hold_timeout_event = self.find_next_timeout_event();
            let result: LoopState = match hold_timeout_event {
                Some(event) => self.process_buffered_taphold_event(event, buffering).await,
                _ => {
                    // Process new key event
                    let key_event = KEY_EVENT_CHANNEL.receive().await;
                    // Process the key change
                    self.process_inner(key_event).await
                }
            };

            buffering = false;
            match result {
                Queue => {
                    // keep unprocessed key events
                    // every key should be buffered into event list, check in every turn in future
                    buffering = true;
                    continue;
                }
                Stop => {
                    return;
                }
                _ => {
                    // stop buffering, clean all buffered events

                    self.release_buffering_tap_keys_in_loop().await;
                    //fallback
                    // After processing the key change, check if there are unprocessed events
                    // This will happen if there's recursion in key processing

                    if self.holding_buffer.is_empty() && !self.unprocessed_events.is_empty() {
                        self.cleanup_events().await;
                    }
                }
            }
        }
    }
}

/// led states for the keyboard hid report (its value is received by by the light service in a hid report)
/// LedIndicator type would be nicer, but that does not have const expr constructor
pub(crate) static LOCK_LED_STATES: core::sync::atomic::AtomicU8 = core::sync::atomic::AtomicU8::new(0u8);

const HOLD_BUFFER_SIZE: usize = 16;

pub struct Keyboard<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize = 0> {
    /// Keymap
    pub(crate) keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,

    /// Unprocessed events
    unprocessed_events: Vec<KeyEvent, 16>,

    /// buffering holding keys
    pub(crate) holding_buffer: Vec<HoldingKey, HOLD_BUFFER_SIZE>,

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
            last_mouse_tick: FnvIndexMap::new(),
            mouse_key_move_delta: 8,
            mouse_wheel_move_delta: 1,
            combo_actions_buffer: Deque::new(),
            combo_on: true,
            #[cfg(feature = "controller")]
            controller_pub: unwrap!(CONTROLLER_CHANNEL.publisher()),
            chord_state: None,
        }
    }

    async fn send_report(&self, report: Report) {
        KEYBOARD_REPORT_CHANNEL.sender().send(report).await
    }

    // deprecated
    async fn cleanup_events(&mut self) {
        loop {
            if self.unprocessed_events.is_empty() {
                break;
            }
            // Process unprocessed events
            let e = self.unprocessed_events.remove(0);
            debug!("Unprocessed event: {:?}", e);
            self.process_inner(e).await;
        }
    }

    // a data accessor for test purpose
    #[cfg(any(test, feature = "std"))]
    pub fn copy_buffer(&mut self) -> Vec<HoldingKey, HOLD_BUFFER_SIZE> {
        return self.holding_buffer.clone();
    }

    // a tap hold key reach hold timeout, turning into hold press event
    async fn handle_tap_hold_timeout(&mut self, timer: TapHoldTimer, _buffering: bool) -> LoopState {
        if let Some(pos) = &self.find_tap_hold_key_index(timer.key_event) {
            trace!("{:?} hold timeout", timer.key_event);
            let mut hold_key: HoldingKey = self.holding_buffer.swap_remove(*pos);
            match hold_key.kind {
                KeyKind::TapHold { hold_action, .. } => {
                    match hold_key.state {
                        Initial => {
                            hold_key.update_state(TapHoldState::BeforeHold);
                            self.process_key_action_normal(hold_action, hold_key.key_event).await;
                            //marked as post hold
                            debug!("{:?} timeout and send HOLD action", timer.key_event);
                            hold_key.update_state(PostHold);

                            self.push_and_sort_buffers(hold_key).ok();
                        }
                        _ => {
                            //marked as post hold
                            warn!(
                                "!fallback: {:?} key in post state {:?}",
                                timer.key_event, hold_key.state
                            );
                            // post release maybe, there should not be other state right now
                            //release
                            self.process_key_action_normal(hold_action, timer.key_event).await;
                        }
                    }
                }
                _ => {
                    //never happen
                }
            }
        } else {
            panic!("Hold event not exists: {:?}", timer);
        }
        OK
    }

    // do clean up for leak keys
    pub(crate) async fn release_buffering_tap_keys_in_loop(&mut self) {
        // Remove any HoldingKey::Others in PostTap state from the buffer

        self.holding_buffer.retain(|e| match e.kind {
            KeyKind::Others(_) => match e.state {
                TapHoldState::PostTap => {
                    debug!("processing buffering TAP keys with pos: {:?}", e.key_event);
                    return false;
                }
                _ => {
                    return true;
                }
            },
            _ => true,
        });
    }

    //from here , we wait first tap hold into hold or process next key
    async fn process_buffered_taphold_event(&mut self, timer: TapHoldTimer, buffering: bool) -> LoopState {
        let now = Instant::now();
        let time_left: Duration = if timer.deadline > now {
            timer.deadline - now
        } else {
            Duration::from_ticks(0)
        };

        debug!(
            "[TAP-HOLD] Processing with TAP hold events: {:?}, timeout in {} ms",
            timer.key_event,
            time_left.as_millis()
        );

        //wait hold timeout of new common key
        match select(Timer::after(time_left), KEY_EVENT_CHANNEL.receive()).await {
            Either::First(_) => {
                // Process hold timeout
                self.handle_tap_hold_timeout(timer, buffering).await
            }
            Either::Second(key_event) => {
                // Process key event
                debug!("[TAP-HOLD] Interrupted into new key event: {:?}", key_event);
                if buffering {
                    //TODO add comment for this
                    self.unprocessed_events.push(key_event).ok();
                } else {
                    self.process_inner(key_event).await;
                }
                OK
            }
        }
    }
    /// Process key changes at (row, col)
    async fn process_inner(&mut self, key_event: KeyEvent) -> LoopState {
        // Matrix should process key pressed event first, record the timestamp of key changes
        if key_event.pressed {
            self.timer[key_event.col as usize][key_event.row as usize] = Some(Instant::now());
        }

        // Process key
        let key_action = self.keymap.borrow_mut().get_action_with_layer_cache(key_event);

        if self.combo_on {
            if let Some(key_action) = self.process_combo(key_action, key_event).await {
                // debug!("Process key action after combo processing: {:?}, {:?}", key_action, key_event);
                self.process_key_action_with_buffer(key_action, key_event).await
            } else {
                OK
            }
        } else {
            self.process_key_action_with_buffer(key_action, key_event).await
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

    // check if current key event is pressed before all keys
    fn is_releasing_as_hold(&mut self, key_event: KeyEvent) -> bool {
        if let Some(timer) = self.timer[key_event.col as usize][key_event.row as usize] {
            if let Some(timeout) = self.find_next_timeout_event() {
                if timeout.pressed_time < timer {
                    true
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        }
    }

    // calculate next state of tap hold
    // 1. turn into buffering state
    // 2. clean buffer
    // 3. ignore
    fn make_tap_hold_decision(&mut self, key_action: KeyAction, key_event: KeyEvent) -> HoldDecision {
        let permissive = self.keymap.borrow().behavior.tap_hold.permissive_hold;

        //fixme count buffer key types should contain more tap hold key
        let is_buffering = self
            .holding_buffer
            .iter()
            .find(|i| match i.kind {
                KeyKind::TapHold { .. } => {
                    return i.state == TapHoldState::Initial;
                }
                _ => {
                    return false;
                }
            })
            .is_some();

        debug!(
            "\x1b[34m[TAP-HOLD] tap_hold_decision\x1b[0m: permissive={}, is_buffering={}, is_pressed={}, action={:?}",
            permissive, is_buffering, key_event.pressed, key_action
        );

        if is_buffering {
            if key_event.pressed {
                // new key pressed after a tap-hold key
                // on chordal holding, let's check if opposite hand pressing
                if let Some(hand) = &self.chord_state {
                    //TODO add more chordal configuration and behaviors here
                    if !hand.is_same(key_event) {
                        debug!("Is chordal hold hand: {:?}, raise", hand);
                        // quick path, set to hold since here comes chord hold conflict
                        // this is a press-on-hold chordal-hold decision
                        return ChordHold;
                    }
                }

                return if permissive {
                    //permissive hold
                    match key_action {
                        KeyAction::TapHold(_, _) => {
                            // a release on tap hold key ignore and continue
                            // clean buffered keys before
                            Ignore
                        }
                        KeyAction::LayerTapHold(_, _) => Ignore,
                        KeyAction::ModifierTapHold(_, _) => Ignore,
                        _ => {
                            // buffer keys and wait for key release to trigger
                            debug!("key {:?} press down while BUFFERING, save it into buffer", key_action);
                            Buffering
                        }
                    }
                } else {
                    //TODO there should be a hold-on-press implement
                    Ignore
                };
            } else {
                //key releasing while tap-holding
                if permissive {
                    // PERMISSIVE HOLDING, which means any key press-and-release after a tap-hold key will raise hold decision
                    return match key_action {
                        _ => CleanBuffer,
                    };
                } else {
                    // not permissive hold, should be Ignore
                    return Ignore;
                }
            }
        }
        //keep going
        Ignore
    }

    async fn process_key_action_with_buffer(
        &mut self,
        original_key_action: KeyAction,
        key_event: KeyEvent,
    ) -> LoopState {
        let decision = self.make_tap_hold_decision(original_key_action, key_event);

        debug!("\x1b[34m[TAP-HOLD] --> decision is \x1b[0m: {:?}", decision);
        match decision {
            Ignore => {}
            Buffering => {
                //save into buffer, will be process in the future
                self.tap_hold_buffering_new_key_action(key_event, original_key_action);
                return Queue;
            }
            CleanBuffer | Hold | ChordHold => {
                self.fire_all_holding_keys_into_press_action(decision, key_event).await;
            }
            _ => {
                panic!("not expected tap hold decision {:?}", decision);
            }
        }
        self.process_key_action(original_key_action, key_event).await
    }

    async fn process_key_action(&mut self, original_key_action: KeyAction, key_event: KeyEvent) -> LoopState {
        //start forks
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

        //release to early
        if !key_event.pressed {
            // Record release of current key, which will be used in tap/hold processing
            debug!("Record released key event: {:?}", key_event);
            let mut is_mod = false;
            if let KeyAction::Single(Action::Key(k)) = key_action {
                if k.is_modifier() {
                    is_mod = true;
                }
            }
            // Record the last release event
            // TODO check key action, should be a-z/space/enter
            self.last_release = (key_event, is_mod, Some(Instant::now()));
        }

        self.try_finish_forks(original_key_action, key_event);

        OK
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
            Timer::after_millis(1).await;
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
    fn push_and_sort_buffers(&mut self, item: HoldingKey) -> Result<(), ()> {
        self.holding_buffer.push(item).ok();
        self.sort_buffers();
        Ok(())
    }

    fn sort_buffers(&mut self) {
        self.holding_buffer.sort_unstable_by(|l, r| {
            if l.start_time().as_millis() >= r.start_time().as_millis() {
                Ordering::Greater
            } else {
                Ordering::Less
            }
        });
    }

    // release hold key
    async fn tap_hold_process_key_release(
        &mut self,
        _hold_action: Action, //default hold action
        tap_action: Action,   //default tap action
        key_event_released: KeyEvent,
    ) {
        debug!("[TAP-HOLD] On Releasing: tap-hold key event {:?}", key_event_released);

        // handling hold key release
        if key_event_released.pressed {
            error!("[TAP-HOLD] release key action with pressed key, should never happen");
            //never call this while pressing key
            return;
        }

        let col = key_event_released.col as usize;
        let row = key_event_released.row as usize;

        trace!("[TAP-HOLD] current buffer queue to process {:?}", self.holding_buffer);

        // while tap hold key is releasing, pressed key event should be updating into PostTap or PostHold state
        if let Some(e) = self.find_tap_hold_key_index(key_event_released) {
            let hold_key = self.holding_buffer.swap_remove(e);
            match hold_key.kind {
                KeyKind::TapHold {
                    tap_action,
                    hold_action,
                    ..
                } => {
                    //FIXME tap key press should happen before
                    match hold_key.state {
                        // Initial => {
                        //     self.process_key_action_normal(e.tap_action(), e.key_event).await;
                        //     e.update_state(TapHoldState::PostTap);
                        //     debug!("tapped {:?}", e.tap_action());
                        //     self.process_key_action_normal(e.tap_action(), key_event_released).await;
                        // },
                        TapHoldState::BeforeHold | PostHold => {
                            debug!(
                                "[TAP-HOLD] {:?} releasing with key event {:?}",
                                hold_key.state, key_event_released
                            );

                            self.process_key_action_normal(hold_action, key_event_released).await;

                            self.tap_hold_release_key_from_buffer(key_event_released);
                        }
                        TapHoldState::PostTap => {
                            debug!(
                                "TapHold {:?}] post Tapping, releasing {:?}",
                                hold_key.key_event, tap_action
                            );
                            self.process_key_action_normal(tap_action, key_event_released).await;
                            self.tap_hold_release_key_from_buffer(key_event_released);
                        }
                        TapHoldState::Initial => {
                            //should be processed in tap hold decission stage
                            debug!(
                                "[TAP-HOLD] quick release should be tapping, send tap action, {:?}",
                                tap_action
                            );

                            // Timer::after_millis(10).await;
                            self.process_key_action_tap(tap_action, hold_key.key_event).await;

                            // //release
                            // self.process_key_action_normal(tap_hold.tap_action(), key_event_released)
                            //     .await;

                            // self.tap_hold_release_key_from_buffer(key_event_released);
                        }
                        _ => {
                            error!(
                                "[TAP-HOLD] Unexpected TapHoldState {:?}, while releasing {:?}",
                                hold_key.state, hold_key.key_event
                            );
                        }
                    }
                }
                _ => {}
            }
        } else {
            //FIXME a release taphold key should in state of PostXX or Hold/Tap , this branch need remove
            //ignore timer, should be fire by main loop
            warn!("[TapHold] not in buffer release as tap, should be tap action");
            self.process_key_action_normal(tap_action, key_event_released).await;
        }
        // Clear timer
        self.timer[col][row] = None;
        debug!("[TAP-HOLD] tap-hold key event {:?}, cleanup done", key_event_released);
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
                //ignore hold within pre idle time for quick typing
                if key_event.pressed {
                    if last_release_time.elapsed() < self.keymap.borrow().behavior.tap_hold.prior_idle_time
                        && !(key_event.row == self.last_release.0.row && key_event.col == self.last_release.0.col)
                    {
                        // The previous key is a different key and released within `prior_idle_time`, it's in key streak
                        debug!("Key streak detected, trigger tap action");
                        self.process_key_action_normal(tap_action, key_event).await;

                        //push into buffer, process by order in loop
                        self.tap_hold_buffering_new_tap_hold_key(
                            key_event,
                            tap_action,
                            hold_action,
                            Instant::now(),
                            TapHoldState::PostTap,
                        );
                        return;
                    } else if last_release_time.elapsed() < self.keymap.borrow().behavior.tap_hold.hold_timeout
                        && key_event.row == self.last_release.0.row
                        && key_event.col == self.last_release.0.col
                    {
                        //quick tapping to repeat

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

        //new hold key pressed , should push into unreleased events
        if key_event.pressed {
            // Press
            let holdTimeOutValue = self.keymap.borrow().behavior.tap_hold.hold_timeout;
            let now = Instant::now();
            let deadline = now + holdTimeOutValue;

            //save unprocessed key
            self.tap_hold_buffering_new_tap_hold_key(key_event, tap_action, hold_action, deadline, Initial);
        } else {
            // Release a th key, should check timeout for tap

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

            //check unreleased event and remove key with same rol and col
            self.tap_hold_process_key_release(hold_action, tap_action, key_event)
                .await;
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
                "Last key code changed from  {:?} to {:?}(pressed: {:?})",
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
                embassy_time::Timer::after_millis(crate::MOUSE_KEY_INTERVAL as u64).await;
                KEY_EVENT_CHANNEL.try_send(key_event).ok();
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

    //# holding keys
    // When a non-tap-hold key_event is released while the permissive-hold feature is enabled:
    // - If the corresponding key_action is a tap-hold, check that it is not already in a TapHold* state.
    // - For all tap-hold keys pressed before this event, trigger their hold action.
    // - For all tap-hold keys pressed after this event, trigger their tap action.
    // This function forces all buffered tap-hold keys to resolve immediately, ignoring their timeouts.
    async fn fire_all_holding_keys_into_press_action(&mut self, _reason: HoldDecision, key_event: KeyEvent) {
        // press time of current key
        let pressed_time: Instant = if let Some(inst) = self.timer[key_event.col as usize][key_event.row as usize] {
            inst
        } else {
            //fire all
            Instant::now()
        };

        let hold_keys_to_flush: Vec<_, HOLD_BUFFER_SIZE> = self
            .holding_buffer
            .iter()
            .enumerate()
            .filter_map(|(pos, e)| if e.state() == Initial { Some(pos) } else { None })
            .collect();

        // If the slot is found, update the key in the slot

        if hold_keys_to_flush.is_empty() {
            debug!("non tap-hold-key hold before current release key, ignore and skip");
            return;
        } else {
            debug!(
                "[TAP-HOLD] Flush keys {:?} in {:?}",
                hold_keys_to_flush, self.holding_buffer,
            );
        }
        // here iter buffer twice, since i just can borrow self twice
        for pos in hold_keys_to_flush {
            if let Some(hold_key) = self.holding_buffer.get(pos) {
                match hold_key.kind {
                    KeyKind::TapHold {
                        tap_action,
                        hold_action,
                        ..
                    } => {
                        if hold_key.key_event.col == key_event.col && hold_key.key_event.row == key_event.row {
                            //self should be tap
                            debug!("Current Key {:?} become {:?}", hold_key.key_event, tap_action);
                            // self.timer[e.key_event.col as usize][e.key_event.row as usize] = None;
                            self.process_key_action_normal(tap_action, hold_key.key_event).await;
                        } else if hold_key.state == Initial && hold_key.pressed_time < pressed_time {
                            debug!("Key {:?} become {:?}", hold_key.key_event, hold_action);
                            // self.timer[e.key_event.col as usize][e.key_event.row as usize] = None;
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
                    KeyKind::Others(key_action) => {
                        debug!("Tap Key {:?} now press down", hold_key.key_event);
                        //TODO ignored return value
                        self.process_key_action(key_action, hold_key.key_event).await;
                        //wait for hid send
                    }
                }
                Timer::after_millis(1).await;
            }
            // Update the state of the key in the buffer after firing its action.
            // This ensures that the buffer accurately reflects which keys have been resolved as tap or hold,
            // so that subsequent processing (e.g., releases or further key events) can handle them correctly.
            if let Some(hold_key) = self.holding_buffer.get_mut(pos) {
                match hold_key.kind {
                    KeyKind::TapHold {
                        tap_action,
                        hold_action,
                        ..
                    } => {
                        if hold_key.key_event.col == key_event.col && hold_key.key_event.row == key_event.row {
                            // This is the key that triggered the flush; mark as PostTap (tap resolved).
                            debug!("Current Key {:?} mark {:?}", hold_key.key_event, tap_action);
                            hold_key.state = TapHoldState::PostTap;
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
                    KeyKind::Others(_) => {
                        // For non-tap-hold keys, mark as PostTap to indicate they've been processed.
                        debug!("Tap Key {:?} now press down", hold_key.key_event);
                        hold_key.state = TapHoldState::PostTap;
                    }
                }
            }
        }

        debug!(
            "[TAP-HOLD] After flush keys, current hold buffer: {:?}",
            self.holding_buffer
        );
        // self.holding_buffer.clear();
        // reset chord state
        self.chord_state = None;
    }

    /// Save a pressed tap-hold key into the holding buffer for later resolution.
    /// This is called when a tap-hold key is pressed, to track its state and timing.
    /// The buffer is sorted by press time to ensure correct tap/hold resolution order.
    fn tap_hold_buffering_new_tap_hold_key(
        &mut self,
        key_event: KeyEvent,
        tap_action: Action,
        hold_action: Action,
        deadline: Instant,
        state: TapHoldState,
    ) {
        let pressed_time = self.timer[key_event.col as usize][key_event.row as usize].unwrap();
        let new_item = HoldingKey {
            state,
            key_event,
            pressed_time,
            kind: KeyKind::TapHold {
                tap_action,
                hold_action,
                deadline,
            },
        };
        debug!("[TAP-HOLD] --> Save TapHold : {:?}", new_item);
        // Add the new tap-hold key to the buffer and keep the buffer sorted by press time.
        let _ = self.push_and_sort_buffers(new_item);

        // If this is the first tap-hold key, initialize the chord state for possible chordal hold detection.
        if self.chord_state.is_none() {
            self.chord_state = Some(ChordHoldState::create(key_event, ROW, COL));
        }
    }

    /// Buffer a single non-tap-hold key press for later evaluation.
    /// Used when a key event needs to be deferred due to tap-hold logic.
    fn tap_hold_buffering_new_key_action(&mut self, key_event: KeyEvent, key_action: KeyAction) {
        let pressed_time = self.timer[key_event.col as usize][key_event.row as usize].unwrap();

        let item = HoldingKey {
            state: Initial,
            key_event,
            pressed_time,
            kind: KeyKind::Others(key_action),
        };
        debug!(
            "Save new Tapping key into buffer: {:?}, size({})",
            item,
            self.holding_buffer.len()
        );
        let _ = self.push_and_sort_buffers(item);
    }

    /// Finds the index of a key in the holding buffer that matches the given key_event.
    ///
    /// This function searches for both TapHold and Others key kinds, but is primarily
    /// intended for use with tap-hold keys. Returns the index if a matching key is found,
    /// otherwise returns None.
    fn find_tap_hold_key_index(&self, key_event: KeyEvent) -> Option<usize> {
        self.holding_buffer
            .iter()
            .enumerate()
            .find_map(|(pos, meta)| match meta.kind {
                KeyKind::TapHold { .. } => {
                    if meta.key_event.row == key_event.row && meta.key_event.col == key_event.col {
                        Some(pos)
                    } else {
                        None
                    }
                }
                KeyKind::Others(_) => {
                    if meta.key_event.row == key_event.row && meta.key_event.col == key_event.col {
                        Some(pos)
                    } else {
                        None
                    }
                }
            })
    }

    //find same key position in unreleased events, marked as released
    fn tap_hold_release_key_from_buffer(&mut self, key_event: KeyEvent) -> bool {
        //release an unprocessed key
        if let Some(pos) = self
            .holding_buffer
            .iter()
            .position(|e| e.is_tap_hold() && e.key_event().row == key_event.row && e.key_event().col == key_event.col)
        {
            return match self.holding_buffer.remove(pos).kind {
                KeyKind::TapHold { .. } => true,
                _ => false,
            };
        }
        false
    }

    async fn drop_buffered_tapping(&mut self, release_time: Instant) {
        //find pre released key and remove from buffer
        if self.holding_buffer.len() > 0 {
            self.holding_buffer
                .iter()
                .enumerate()
                .for_each(|(_, key)| match key.kind {
                    KeyKind::Others(_) => if key.state == Initial && key.pressed_time <= release_time {},
                    KeyKind::TapHold { .. } => {}
                })
        }
    }

    //TODO improve performance
    fn find_next_timeout_event(&mut self) -> Option<TapHoldTimer> {
        //release an unprocessed key
        if self.holding_buffer.len() > 0 {
            self.holding_buffer
                .iter()
                .filter_map(|key| match key.kind {
                    KeyKind::TapHold { deadline, .. } => {
                        //
                        if key.state == Initial {
                            Some(TapHoldTimer {
                                key_event: key.key_event,
                                pressed_time: key.pressed_time,
                                deadline: deadline.clone(),
                            })
                        } else {
                            None
                        }
                    }
                    _ => None,
                })
                .min_by_key(|e| e.deadline)
        } else {
            None?
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
}

#[cfg(test)]
mod test {

    use embassy_futures::block_on;
    use embassy_time::{Duration, Timer};
    use futures::{join, FutureExt};
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
        let event = keyboard.find_next_timeout_event().unwrap();
        keyboard.process_buffered_taphold_event(event, false).await;
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
    fn test_tap_hold_key_tap_and_single_hold() {

        let _main = async {
            let mut keyboard = create_test_keyboard();
            let tap_hold_action = KeyAction::TapHold(Action::Key(KeyCode::A), Action::Key(KeyCode::LShift));
            // Tap
            join!(
                keyboard.process_key_action(tap_hold_action.clone(), key_event(2, 1, true)),
                Timer::after(Duration::from_millis(0)).then( |_| async {
                //send release event
                  KEY_EVENT_CHANNEL.send(key_event(2, 1, false)).await;
                })
            );


            match KEYBOARD_REPORT_CHANNEL.receive().await {
                Report::KeyboardReport(report) =>
                    assert_eq!(report.keycodes[0], 0x4),  // A should be released
                _ => panic!("Expected a Tap on A, but received a different report type"),
            };

            // a released
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

            //press down F
            // first press ever of the Again issues KeyCode:No
            keyboard.process_inner(key_event(0, 0, true)).await;
            keyboard
                .send_keyboard_report_with_resolved_modifiers(true)
                .await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::No); // A key's HID code is 0x04
            //release F
            keyboard.process_inner(key_event(0, 0, false)).await;

            // Press A key
            keyboard.process_inner(key_event(2, 1, true)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::A); // A key's HID code is 0x04

            // Release A key
            keyboard.process_inner(key_event(2, 1, false)).await;
            assert_eq!(keyboard.held_keycodes[0], KeyCode::No);



            keyboard.process_inner(key_event(0, 0, false)).await;

            // here release event should make again into hold


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
