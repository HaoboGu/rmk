use core::cell::RefCell;
use core::fmt::Debug;

use embassy_futures::select::{Either, select};
use embassy_futures::yield_now;
#[cfg(feature = "_ble")]
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Instant, Timer, with_deadline};
use heapless::Vec;
use rmk_types::action::{Action, KeyAction, KeyboardAction, MorseMode};
use rmk_types::keycode::{ConsumerKey, HidKeyCode, KeyCode, SpecialKey, SystemControlKey};
use rmk_types::led_indicator::LedIndicator;
use rmk_types::modifier::ModifierCombination;
use rmk_types::mouse_button::MouseButtons;
use usbd_hid::descriptor::{MediaKeyboardReport, MouseReport, SystemControlReport};

use crate::channel::KEYBOARD_REPORT_CHANNEL;
use crate::combo::Combo;
use crate::config::Hand;
use crate::descriptor::KeyboardReport;
#[cfg(all(feature = "split", feature = "_ble", feature = "controller"))]
use crate::event::ClearPeerEvent;
#[cfg(feature = "controller")]
use crate::event::{KeyEvent, ModifierEvent, publish_controller_event};
use crate::event::{KeyPos, KeyboardEvent, KeyboardEventPos, SubscribableInputEvent, publish_input_event_async};
use crate::fork::{ActiveFork, StateBits};
use crate::hid::Report;
use crate::input_device::Runnable;
use crate::input_device::rotary_encoder::Direction;
use crate::keyboard::held_buffer::{HeldBuffer, HeldKey, KeyState};
use crate::keyboard_macros::MacroOperation;
use crate::keymap::KeyMap;
use crate::morse::{MorsePattern, TAP};
#[cfg(all(feature = "split", feature = "_ble"))]
use crate::split::ble::central::update_activity_time;
use crate::{FORK_MAX_NUM, boot};

pub(crate) mod combo;
pub(crate) mod held_buffer;
pub(crate) mod morse;
pub(crate) mod mouse;
pub(crate) mod oneshot;

const HOLD_BUFFER_SIZE: usize = 16;

// Timestamp of the last key action, the value is the number of seconds since the boot
#[cfg(feature = "_ble")]
pub(crate) static LAST_KEY_TIMESTAMP: Signal<crate::RawMutex, u32> = Signal::new();

/// Led states for the keyboard hid report (its value is received by by the light service in a hid report)
/// LedIndicator type would be nicer, but that does not have const expr constructor
pub(crate) static LOCK_LED_STATES: core::sync::atomic::AtomicU8 = core::sync::atomic::AtomicU8::new(0u8);

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

/// State machine for Caps Word
#[derive(Debug, Default)]
enum CapsWordState {
    /// Caps Word is activated (but may have timed out thus becoming inactive)
    Activated {
        /// Time since last key press
        timer: Instant,
        /// Whether the current key should be shifted
        shift_current: bool,
    },
    /// Caps Word is deactivated
    #[default]
    Deactivated,
}

impl CapsWordState {
    /// Caps Word timeout duration
    const TIMEOUT: Duration = Duration::from_secs(5);

    /// Activate Caps Word
    fn activate(&mut self) {
        *self = CapsWordState::Activated {
            timer: Instant::now(),
            shift_current: false,
        };
    }

    /// Deactivate Caps Word
    fn deactivate(&mut self) {
        *self = CapsWordState::Deactivated;
    }

    /// Toggle Caps Word
    fn toggle(&mut self) {
        match self {
            CapsWordState::Activated { .. } => self.deactivate(),
            CapsWordState::Deactivated => self.activate(),
        }
    }

    /// Return whether Caps Word is active (and has not timed out)
    fn is_active(&self) -> bool {
        if let CapsWordState::Activated { timer, .. } = self {
            timer.elapsed() < Self::TIMEOUT
        } else {
            false
        }
    }

    /// Return whether the current key pressed is to be shifted
    fn is_shift_current(&self) -> bool {
        if let CapsWordState::Activated { shift_current, .. } = self {
            *shift_current
        } else {
            false
        }
    }

    /// Check whether to shift the given key, and update the state accordingly
    ///
    /// Note that this function does not check the CapsWord key itself.
    fn check(&mut self, key: HidKeyCode) {
        if let CapsWordState::Activated { timer, shift_current } = self {
            if key.is_caps_word_continue_key() && timer.elapsed() < Self::TIMEOUT {
                *timer = Instant::now();
                *shift_current = key.is_caps_word_shifted_key();
            } else {
                self.deactivate();
            }
        }
    }
}

impl<const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize> Runnable
    for Keyboard<'_, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    /// Main keyboard processing task, it receives input devices result, processes keys.
    /// The report is sent using `send_report`.
    async fn run(&mut self) -> ! {
        loop {
            // TODO: Now the unprocessed_events is only used in one-shot keys and clear peer key.
            // Maybe it can be removed in the future?
            if !self.unprocessed_events.is_empty() {
                // Process unprocessed events
                let e = self.unprocessed_events.remove(0);
                debug!("Unprocessed event: {:?}", e);
                self.process_inner(e).await
            } else if let Some(key) = self.next_buffered_key() {
                // Process buffered held key
                self.process_buffered_key(key).await
            } else {
                // No buffered tap-hold event, wait for new key
                let event = self.keyboard_event_subscriber.receive().await;
                // Process the key event
                self.process_inner(event).await
            };
        }
    }
}

pub struct Keyboard<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize = 0> {
    /// Keymap
    pub(crate) keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,

    /// Keyboard event subscriber - single instance to receive all keyboard events
    keyboard_event_subscriber: embassy_sync::channel::Receiver<'static, crate::RawMutex, KeyboardEvent, 16>,

    /// Unprocessed events
    pub unprocessed_events: Vec<KeyboardEvent, 4>,

    /// Buffered held keys
    pub held_buffer: HeldBuffer,

    /// Timer which records the timestamp of key changes
    pub(crate) timer: [[Option<Instant>; ROW]; COL],

    /// Timer which records the timestamp of rotary encoder changes
    pub(crate) rotary_encoder_timer: [[Option<Instant>; 2]; NUM_ENCODER],

    /// Record the timestamp of last **simple key** press.
    /// It's used in tap-hold prior-idle-time check.
    last_press_time: Instant,

    /// stores the last KeyCode executed, to be repeated if the repeat key os pressed
    /// Used in repeat-key
    last_key_code: KeyCode,

    /// One shot layer state
    osl_state: OneShotState<u8>,

    /// One shot modifier state
    osm_state: OneShotState<ModifierCombination>,

    /// Caps Word state machine
    caps_word: CapsWordState,

    /// The modifiers coming from (last) Action::KeyWithModifier
    with_modifiers: ModifierCombination,

    /// Macro text typing state (affects the effective modifiers)
    macro_texting: bool,
    macro_caps: bool,

    /// The real state before fork activations is stored here
    fork_states: [Option<ActiveFork>; FORK_MAX_NUM], // chosen replacement key of the currently triggered forks and the related modifier suppression
    fork_keep_mask: ModifierCombination, // aggregate here the explicit modifiers pressed since the last fork activations

    /// The held modifiers for the keyboard hid report
    held_modifiers: ModifierCombination,

    /// The held keys for the keyboard hid report, except the modifiers
    held_keycodes: [HidKeyCode; 6],

    /// Registered key position.
    /// This is still needed besides `held_keycodes` because multiple keys with same keycode can be registered.
    registered_keys: [Option<KeyboardEvent>; 6],

    /// Internal mouse report buf
    mouse_report: MouseReport,

    /// Internal media report buf
    media_report: MediaKeyboardReport,

    /// Internal system control report buf
    system_control_report: SystemControlReport,

    /// Mouse acceleration state
    mouse_accel: u8,
    mouse_repeat: u8,
    mouse_wheel_repeat: u8,

    /// Used for temporarily disabling combos
    combo_on: bool,
}

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    Keyboard<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    pub fn new(keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>) -> Self {
        Keyboard {
            keymap,
            keyboard_event_subscriber: KeyboardEvent::input_subscriber(),
            timer: [[None; ROW]; COL],
            rotary_encoder_timer: [[None; 2]; NUM_ENCODER],
            last_press_time: Instant::now(),
            osl_state: OneShotState::default(),
            osm_state: OneShotState::default(),
            caps_word: CapsWordState::default(),
            with_modifiers: ModifierCombination::default(),
            macro_texting: false,
            macro_caps: false,
            fork_states: [None; FORK_MAX_NUM],
            fork_keep_mask: ModifierCombination::default(),
            unprocessed_events: Vec::new(),
            held_buffer: HeldBuffer::new(),
            registered_keys: [None; 6],
            held_modifiers: ModifierCombination::default(),
            held_keycodes: [HidKeyCode::No; 6],
            mouse_report: MouseReport {
                buttons: 0,
                x: 0,
                y: 0,
                wheel: 0,
                pan: 0,
            },
            media_report: MediaKeyboardReport { usage_id: 0 },
            system_control_report: SystemControlReport { usage_id: 0 },
            last_key_code: KeyCode::Hid(HidKeyCode::No),
            mouse_accel: 0,
            mouse_repeat: 0,
            mouse_wheel_repeat: 0,
            combo_on: true,
        }
    }

    /// Send a keyboard report to the host
    async fn send_report(&self, report: Report) {
        KEYBOARD_REPORT_CHANNEL.sender().send(report).await
    }

    /// Get a copy of the next timeout key in the buffer,
    /// which is either a combo component that is waiting for other combo keys,
    /// or a morse key that is in the pressed or released state.
    pub fn next_buffered_key(&mut self) -> Option<HeldKey> {
        self.held_buffer.next_timeout(|k| {
            matches!(k.state, KeyState::Released(_) | KeyState::WaitingCombo)
                || (matches!(k.state, KeyState::Pressed(_)) && k.action.is_morse())
        })
    }

    // Clean up for leak keys, remove non morse keys in ProcessedButReleaseNotReportedYet state from the buffer
    pub(crate) fn clean_buffered_processed_keys(&mut self) {
        self.held_buffer.keys.retain(|k| {
            if k.action.is_morse() {
                true
            } else {
                match k.state {
                    KeyState::ProcessedButReleaseNotReportedYet(_) => {
                        warn!("NEED CLEAN: Processing buffering TAP keys with post tap: {:?}", k.event);
                        false
                    }
                    _ => true,
                }
            }
        });
    }

    /// Process the latest buffered key.
    ///
    /// The given holding key is a copy of the buffered key. Only tap-hold keys are considered now.
    pub async fn process_buffered_key(&mut self, key: HeldKey) {
        debug!(
            "Processing buffered key: \nevent: {:?} state: {:?}",
            key.event, key.state
        );
        match key.state {
            KeyState::WaitingCombo => {
                debug!(
                    "[Combo] Waiting combo, timeout in: {:?}ms",
                    (key.timeout_time.saturating_duration_since(Instant::now())).as_millis()
                );
                match with_deadline(key.timeout_time, self.keyboard_event_subscriber.receive()).await {
                    Ok(event) => {
                        // Process new key event
                        debug!("[Combo] Interrupted by a new key event: {:?}", event);
                        self.process_inner(event).await;
                    }
                    Err(_timeout) => {
                        // Timeout, dispatch combo
                        debug!("[Combo] Timeout, dispatch combo");
                        self.dispatch_combos(&key.action, key.event).await;
                    }
                }
            }
            KeyState::Pressed(_) | KeyState::Released(_) => {
                if key.action.is_morse() {
                    // Wait for timeout or new key event
                    info!("Waiting morse key: {:?}", key.action);
                    match with_deadline(key.timeout_time, self.keyboard_event_subscriber.receive()).await {
                        Ok(event) => {
                            debug!("Buffered morse key interrupted by a new key event: {:?}", event);
                            self.process_inner(event).await;
                        }
                        Err(_timeout) => {
                            debug!("Buffered morse key timeout");
                            self.handle_morse_timeout(&key).await;
                        }
                    }
                }
            }
            _ => (),
        }
    }

    /// Process key changes at (row, col)
    pub async fn process_inner(&mut self, event: KeyboardEvent) {
        #[cfg(feature = "vial_lock")]
        self.keymap.borrow_mut().matrix_state.update(&event);

        // Matrix should process key pressed event first, record the timestamp of key changes
        if event.pressed {
            self.set_timer_value(event, Some(Instant::now()));
        }
        // Update activity time for BLE split central sleep management
        #[cfg(all(feature = "split", feature = "_ble"))]
        update_activity_time();

        // Process key
        let key_action = &self.keymap.borrow_mut().get_action_with_layer_cache(event);

        if self.combo_on {
            if let (Some(key_action), is_combo) = self.process_combo(key_action, event).await {
                self.process_key_action(&key_action, event, is_combo).await
            }
        } else {
            self.process_key_action(key_action, event, false).await
        }
    }

    async fn process_key_action(&mut self, key_action: &KeyAction, event: KeyboardEvent, is_combo: bool) {
        // First, make the decision for current key and held keys
        let (decision_for_current_key, decisions) = self.make_decisions_for_keys(key_action, event);

        // Fire held keys if needed
        let (keyboard_state_updated, updated_decision_for_cur_key) =
            self.fire_held_keys(decision_for_current_key, decisions).await;

        // Process current key action after all held keys are resolved
        match updated_decision_for_cur_key {
            KeyBehaviorDecision::CleanBuffer | KeyBehaviorDecision::Release => {
                debug!("Clean buffer, then process current key normally");
                let key_action = if keyboard_state_updated && !is_combo {
                    // The key_action needs to be updated due to the morse key might be triggered
                    &self.keymap.borrow_mut().get_action_with_layer_cache(event)
                } else {
                    key_action
                };
                self.process_key_action_inner(key_action, event).await
            }
            KeyBehaviorDecision::Buffer => {
                debug!("Current key is buffered");
                let press_time = Instant::now();
                let timeout_time = if key_action.is_morse() {
                    press_time + Self::morse_timeout(&self.keymap.borrow(), key_action, true)
                } else {
                    press_time
                };
                self.held_buffer.push(HeldKey::new(
                    event,
                    *key_action,
                    KeyState::Pressed(MorsePattern::default()),
                    press_time,
                    timeout_time,
                ));
            }
            KeyBehaviorDecision::Ignore => {
                debug!("Current key is ignored or not buffered, process normally: {:?}", event);
                // Process current key normally
                let key_action = if keyboard_state_updated && !is_combo {
                    // The key_action needs to be updated due to the morse key might be triggered
                    &self.keymap.borrow_mut().get_action_with_layer_cache(event)
                } else {
                    key_action
                };
                self.process_key_action_inner(key_action, event).await
            }
            KeyBehaviorDecision::FlowTap => {
                let action = Self::action_from_pattern(self.keymap.borrow().behavior, key_action, TAP); //tap action
                self.process_key_action_normal(action, event).await;
                // Push back after triggered press
                let now = Instant::now();
                let time_out = now + Self::morse_timeout(&self.keymap.borrow(), key_action, true);
                self.held_buffer.push(HeldKey::new(
                    event,
                    *key_action,
                    KeyState::ProcessedButReleaseNotReportedYet(action),
                    now,
                    time_out,
                ));
            }
        }
    }

    /// Fire held keys according to their decisions.
    ///
    /// This function fires held keys according to their decisions, and returns
    /// whether the keyboard state is updated after firing those keys and
    /// the updated decision for current key.
    async fn fire_held_keys(
        &mut self,
        mut decision_for_current_key: KeyBehaviorDecision,
        decisions: Vec<(KeyboardEventPos, HeldKeyDecision), 16>,
    ) -> (bool, KeyBehaviorDecision) {
        let mut keyboard_state_updated = false;
        // Fire buffered keys
        for (pos, decision) in decisions {
            // Some decisions of held keys have been made, fire those keys
            // debug!("✅ Decision for held key: {:?}: {:?}", pos, decision)
            match decision {
                HeldKeyDecision::UnilateralTap | HeldKeyDecision::FlowTap => {
                    if let Some(mut held_key) = self.held_buffer.remove_if(|k| k.event.pos == pos)
                        && held_key.action.is_morse()
                    {
                        // Unilateral tap of the held key is triggered
                        debug!("Cleaning buffered morse key due to unilateral tap or flow tap");
                        match held_key.state {
                            KeyState::Pressed(_) | KeyState::Holding(_) => {
                                // In this state pattern is not surely finished,
                                // however an other key is pressed so terminate the sequence
                                // with a tap due to UnilateralTap decision; try to resolve as is
                                let pattern = match held_key.state {
                                    KeyState::Pressed(pattern) => pattern.followed_by_tap(), // The HeldKeyDecision turned this into tap!
                                    KeyState::Holding(pattern) => pattern,
                                    _ => unreachable!(),
                                };
                                debug!("Pattern after unilateral tap or flow tap: {:?}", pattern);
                                let action =
                                    Self::action_from_pattern(self.keymap.borrow().behavior, &held_key.action, pattern);
                                self.process_key_action_normal(action, held_key.event).await;
                                held_key.state = KeyState::ProcessedButReleaseNotReportedYet(action);
                                // Push back after triggered tap
                                self.held_buffer.push_without_sort(held_key);
                            }
                            KeyState::Released(pattern) => {
                                // In this state pattern is not surely finished,
                                // however an other key is pressed so terminate the sequence, try to resolve as is
                                debug!("Pattern after released, unilateral tap or flow tap: {:?}", pattern);
                                let action =
                                    Self::action_from_pattern(self.keymap.borrow().behavior, &held_key.action, pattern);
                                held_key.event.pressed = true;
                                self.process_key_action_tap(action, held_key.event).await;
                                // The tap is fully fired, don't push it back to buffer again
                                // Removing from the held buffer is like setting to an idle state
                            }
                            _ => (),
                        }
                    }
                }
                HeldKeyDecision::PermissiveHold | HeldKeyDecision::HoldOnOtherKeyPress => {
                    if let Some(mut held_key) = self.held_buffer.remove_if(|k| k.event.pos == pos) {
                        let action = self.keymap.borrow_mut().get_action_with_layer_cache(held_key.event);

                        if action.is_morse() {
                            // Permissive hold of held key is triggered
                            debug!("Cleaning buffered morse key due to permissive hold or hold on other key press");
                            match held_key.state {
                                KeyState::Pressed(_) | KeyState::Holding(_) => {
                                    // In this state pattern is not surely finished,
                                    // however an other key is pressed so terminate the sequence
                                    // with a hold due to PermissiveHold/HoldOnOtherKeyPress decision; try to resolve as is
                                    let pattern = match held_key.state {
                                        KeyState::Pressed(pattern) => pattern.followed_by_hold(), // The HeldKeyDecision turned this into hold!
                                        KeyState::Holding(pattern) => pattern,
                                        _ => unreachable!(),
                                    };
                                    keyboard_state_updated = true;
                                    debug!("pattern after permissive hold: {:?}", pattern);
                                    let action =
                                        Self::action_from_pattern(self.keymap.borrow().behavior, &action, pattern);
                                    self.process_key_action_normal(action, held_key.event).await;
                                    held_key.state = KeyState::ProcessedButReleaseNotReportedYet(action);
                                    // Push back after triggered hold
                                    self.held_buffer.push_without_sort(held_key);
                                }
                                KeyState::Released(pattern) => {
                                    debug!("pattern after released, permissive hold: {:?}", pattern);
                                    let action =
                                        Self::action_from_pattern(self.keymap.borrow().behavior, &action, pattern);
                                    held_key.event.pressed = true;
                                    self.process_key_action_tap(action, held_key.event).await;
                                    // The tap is fully fired, don't push it back to buffer again
                                    // Removing from the held buffer is like setting to an idle state
                                }
                                _ => (),
                            }
                        }
                    }
                }
                HeldKeyDecision::Release => {
                    // Releasing the current key, will always be tapping, because timeout isn't here
                    if let Some(mut held_key) = self.held_buffer.remove_if(|k| k.event.pos == pos) {
                        let key_action = if keyboard_state_updated {
                            self.keymap.borrow_mut().get_action_with_layer_cache(held_key.event)
                        } else {
                            held_key.action
                        };
                        debug!("Processing current key before releasing: {:?}", held_key.event);
                        if !key_action.is_morse() {
                            match key_action {
                                KeyAction::Single(action) => {
                                    self.process_key_action_normal(action, held_key.event).await;
                                }
                                KeyAction::Tap(action) => {
                                    self.process_key_action_tap(action, held_key.event).await;
                                }
                                _ => unreachable!(),
                            }
                        } else {
                            match held_key.state {
                                KeyState::Pressed(_) | KeyState::Holding(_) => {
                                    debug!("Cleaning buffered Release key");

                                    let pattern = match held_key.state {
                                        KeyState::Pressed(pattern) => pattern.followed_by_tap(), // TODO? should we double check the timeout with Instant::now() >= held_key.timeout_time?
                                        KeyState::Holding(pattern) => pattern,
                                        _ => unreachable!(),
                                    };

                                    debug!("pattern by decided tap release: {:?}", pattern);

                                    let final_action = Self::try_predict_final_action(
                                        self.keymap.borrow().behavior,
                                        &key_action,
                                        pattern,
                                    );
                                    if let Some(action) = final_action {
                                        debug!("tap prediction {:?} -> {:?}", pattern, action);
                                        self.process_key_action_normal(action, held_key.event).await;
                                        held_key.state = KeyState::ProcessedButReleaseNotReportedYet(action);
                                    }
                                }
                                _ => {} // For morse, the releasing will not be processed immediately, so just ignore it
                            }
                            // Push back after triggered hold
                            self.held_buffer.push_without_sort(held_key);
                        }
                    }

                    // After processing current key in `Release` state, mark the `decision_for_current_key` to `CleanBuffer`
                    // That means all normal keys pressed AFTER the press of current releasing key will be fired
                    decision_for_current_key = KeyBehaviorDecision::CleanBuffer;
                }
                HeldKeyDecision::Normal => {
                    // Check if the normal keys in the buffer should be triggered.
                    let trigger_normal = matches!(decision_for_current_key, KeyBehaviorDecision::CleanBuffer);

                    if trigger_normal && let Some(held_key) = self.held_buffer.remove_if(|k| k.event.pos == pos) {
                        debug!("Cleaning buffered normal key");
                        let action = if keyboard_state_updated {
                            self.keymap.borrow_mut().get_action_with_layer_cache(held_key.event)
                        } else {
                            held_key.action
                        };

                        // Note: Morse like actions are not expected here.
                        assert!(!action.is_morse());
                        debug!("Tap Key {:?} now press down, action: {:?}", held_key.event, action);
                        self.process_key_action_inner(&action, held_key.event).await;
                    }
                }
                _ => (),
            }
        }
        (keyboard_state_updated, decision_for_current_key)
    }

    fn get_hand(hand_info: &[[Hand; COL]; ROW], pos: KeyPos) -> Hand {
        let col = pos.col as usize;
        let row = pos.row as usize;
        if col < COL && row < ROW {
            hand_info[row][col]
        } else {
            Hand::Unknown
        }
    }

    /// Make decisions for current key and each held key.
    ///
    /// This function iterates all held keys and makes decision for them if a special mode is triggered, such as permissive hold, etc.
    fn make_decisions_for_keys(
        &mut self,
        key_action: &KeyAction,
        event: KeyboardEvent,
    ) -> (
        KeyBehaviorDecision,
        Vec<(KeyboardEventPos, HeldKeyDecision), HOLD_BUFFER_SIZE>,
    ) {
        // Decision of current key and held keys
        let mut decision_for_current_key = KeyBehaviorDecision::Ignore;
        let mut decisions: Vec<(_, HeldKeyDecision), HOLD_BUFFER_SIZE> = Vec::new();

        // When pressing a morse key, check flow tap first.
        if event.pressed
            && self.keymap.borrow().behavior.morse.enable_flow_tap
            && key_action.is_morse()
            && self.last_press_time.elapsed() < self.keymap.borrow().behavior.morse.prior_idle_time
        {
            // It's in key streak, trigger the first tap action
            debug!("Flow tap detected, trigger tap action for current morse key");

            decision_for_current_key = KeyBehaviorDecision::FlowTap;
        }

        // Whether the held buffer needs to be checked.
        let check_held_buffer = event.pressed
            || self
                .held_buffer
                .find_action(key_action)
                .is_some_and(|k| matches!(k.state, KeyState::Pressed(_) | KeyState::Released(_)));

        if check_held_buffer {
            // First, sort by press time
            self.held_buffer.keys.sort_unstable_by_key(|k| k.press_time);

            // Check all unresolved held keys, calculate their decision one-by-one
            for held_key in self
                .held_buffer
                .keys
                .iter()
                .filter(|k| matches!(k.state, KeyState::Pressed(_) | KeyState::Released(_)))
            {
                // Releasing a key is already buffered
                if !event.pressed && held_key.action == *key_action {
                    debug!("Releasing a held key: {:?}", event);
                    let _ = decisions.push((held_key.event.pos, HeldKeyDecision::Release));
                    decision_for_current_key = KeyBehaviorDecision::Release;
                    continue;
                }

                // Buffered normal keys should be added to the decision list,
                // they will be processed later according to the decision of current key
                if !held_key.action.is_morse() && matches!(held_key.state, KeyState::Pressed(_)) {
                    let _ = decisions.push((held_key.event.pos, HeldKeyDecision::Normal));
                    continue;
                }

                // The remaining keys are not same as the current key, check only morse keys
                if held_key.event.pos != event.pos && held_key.action.is_morse() {
                    let mode = Self::tap_hold_mode(&self.keymap.borrow(), &held_key.action);

                    if event.pressed {
                        // The current key is being pressed

                        if decision_for_current_key == KeyBehaviorDecision::FlowTap
                            && matches!(held_key.state, KeyState::Pressed(_))
                        {
                            debug!("Flow tap triggered, resolve buffered morse key as tapping");
                            // If flow tap of current key is triggered, tapping all held keys
                            let _ = decisions.push((held_key.event.pos, HeldKeyDecision::FlowTap));
                            continue;
                        }

                        // Check morse key mode
                        match mode {
                            MorseMode::PermissiveHold => {
                                // Permissive hold mode checks key releases, so push current key press into buffer.
                                decision_for_current_key = KeyBehaviorDecision::Buffer;
                            }
                            MorseMode::HoldOnOtherPress => {
                                debug!(
                                    "Trigger morse key due to hold on other key press: {:?}",
                                    held_key.action
                                );
                                let _ = decisions.push((held_key.event.pos, HeldKeyDecision::HoldOnOtherKeyPress));
                                decision_for_current_key = KeyBehaviorDecision::CleanBuffer;
                            }
                            _ => {}
                        }
                    } else {
                        let unilateral_tap = Self::is_unilateral_tap_enabled(&self.keymap.borrow(), &held_key.action);

                        // 1. Check unilateral tap of held key
                        // Note: `decision for current key == Release` means that current held key is pressed AFTER the current releasing key,
                        // releasing a key should not trigger unilateral tap of keys which are pressed AFTER the released key
                        if unilateral_tap
                            && event.pos != held_key.event.pos
                            && decision_for_current_key != KeyBehaviorDecision::Release
                            && let KeyboardEventPos::Key(pos1) = held_key.event.pos
                            && let KeyboardEventPos::Key(pos2) = event.pos
                        {
                            let hand_info = &self.keymap.borrow().positional_config.hand;

                            let hand1 = Self::get_hand(hand_info, pos1);
                            let hand2 = Self::get_hand(hand_info, pos2);

                            if hand1 == hand2 && hand1 != Hand::Unknown {
                                //if same hand
                                debug!("Unilateral tap triggered, resolve morse key as tapping");
                                let _ = decisions.push((held_key.event.pos, HeldKeyDecision::UnilateralTap));
                                continue;
                            }
                        }

                        // The current key is being released, check only the held key in permissive hold mode
                        if decision_for_current_key != KeyBehaviorDecision::Release && mode == MorseMode::PermissiveHold
                        {
                            debug!("Permissive hold!");
                            // Check first current releasing key is in the buffer, AND after the current key
                            let _ = decisions.push((held_key.event.pos, HeldKeyDecision::PermissiveHold));
                            decision_for_current_key = KeyBehaviorDecision::CleanBuffer;
                        }
                    }
                }
            }
        }
        (decision_for_current_key, decisions)
    }

    async fn process_key_action_inner(&mut self, original_key_action: &KeyAction, event: KeyboardEvent) {
        // Start forks
        let key_action = self.try_start_forks(original_key_action, event);

        // Clear with_modifier if a new key is pressed
        if self.with_modifiers.into_bits() != 0 && event.pressed {
            self.with_modifiers = ModifierCombination::new();
        }

        #[cfg(feature = "_ble")]
        LAST_KEY_TIMESTAMP.signal(Instant::now().as_secs() as u32);

        #[cfg(feature = "controller")]
        publish_controller_event(KeyEvent {
            keyboard_event: event,
            key_action,
        });

        if !key_action.is_morse() {
            match key_action {
                KeyAction::No | KeyAction::Transparent => (),
                KeyAction::Single(action) => {
                    debug!("Process Single key action: {:?}, {:?}", action, event);
                    self.process_key_action_normal(action, event).await;
                }
                KeyAction::Tap(action) => self.process_key_action_tap(action, event).await,
                _ => unreachable!(),
            }
        } else {
            self.process_key_action_morse(&key_action, event).await;
        }
        self.try_finish_forks(original_key_action, event);
    }

    /// Replaces the incoming key_action if a fork is configured for that key.
    /// The replacement decision is made at key_press time, and the decision
    /// is kept until the key is released.
    fn try_start_forks(&mut self, key_action: &KeyAction, event: KeyboardEvent) -> KeyAction {
        if self.keymap.borrow().behavior.fork.forks.is_empty() {
            return *key_action;
        }

        if !event.pressed {
            for (i, fork) in (&self.keymap.borrow().behavior.fork.forks).into_iter().enumerate() {
                if fork.trigger == *key_action
                    && let Some(active) = self.fork_states[i]
                {
                    // If the originating key of a fork is released, simply release the replacement key
                    // (The fork deactivation is delayed, will happen after the release hid report is sent)
                    debug!("replace input with fork action {:?}", active);
                    return active.replacement;
                }
            }
            return *key_action;
        }

        let mut decision_state = StateBits {
            // "explicit modifiers" includes the effect of one-shot modifiers, held modifiers keys only
            modifiers: self.resolve_explicit_modifiers(event.pressed),
            leds: LedIndicator::from_bits(LOCK_LED_STATES.load(core::sync::atomic::Ordering::Relaxed)),
            mouse: MouseButtons::from_bits(self.mouse_report.buttons),
        };

        let mut triggered_forks = [false; FORK_MAX_NUM]; // used to avoid loops
        let mut chain_starter: Option<usize> = None;
        let mut combined_suppress = ModifierCombination::default();
        let mut replacement = *key_action;

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

                    // Suppress the previously activated Action::KeyWithModifiers
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
    fn try_finish_forks(&mut self, original_key_action: &KeyAction, event: KeyboardEvent) {
        if !event.pressed {
            for (i, fork) in (&self.keymap.borrow().behavior.fork.forks).into_iter().enumerate() {
                if self.fork_states[i].is_some() && fork.trigger == *original_key_action {
                    // if the originating key of a fork is released the replacement decision is not valid anymore
                    self.fork_states[i] = None;
                }
            }
        }
    }

    /// Trigger a combo that is delayed(if exists).
    ///
    /// A combo is delayed when it's **triggered** but it's a "subset" of another combo, like combo "asd" and combo "asdf".
    /// When "asd" is pressed, combo "asd" is delayed due to there's "asdf" combo. The delayed combo will be triggered when:
    /// - Timeout
    /// - Any of the key in the delayed combo is released
    /// - Current delayed combos are interrupted
    ///
    /// When multiple combos are delayed, this function will only trigger the longest one, for example,
    /// combo "as", "sd" and "asd" are delayed, this function will only trigger "asd", and clear the combo state of "as"/"sd"
    ///
    /// If the full combo("asdf") is triggered, the delayed combo will be cleared without triggering it.
    ///
    /// Parameters:
    /// - `key_action`: The action of the key that triggered this function
    /// - `event`: The keyboard event. When pressing (interrupting), trigger any delayed combo.
    ///   When releasing, only trigger combos that contain the key_action.
    async fn trigger_delayed_combo(&mut self, key_action: &KeyAction, event: KeyboardEvent) {
        // First, find the delayed combo and trigger it
        let triggered_combo = self
            .keymap
            .borrow_mut()
            .behavior
            .combo
            .combos
            .iter_mut()
            .filter_map(|c| c.as_mut())
            .filter_map(|c| {
                if c.is_all_pressed() && !c.is_triggered() {
                    // When a key is pressed (interrupting a combo wait), trigger any delayed combo.
                    // When releasing a key, only trigger combos that contain the key_action.
                    if event.pressed || c.config.actions.contains(key_action) {
                        // All keys are pressed but the combo is not triggered, trigger it
                        return Some((c.size(), c));
                    }
                }
                None
            }) // Find all delayed combos
            .max_by_key(|x| x.0) // Find only the longest one
            .map(|(_, c)| (c.trigger(), c.config.actions)); // Trigger it and get the actions

        // Clean the held buffer, process the combo output action and clear other combos
        if let Some((action, combo_actions)) = triggered_combo {
            // Only remove keys that are part of the triggered combo from the held buffer
            self.held_buffer.keys.retain(|item| {
                if item.state != KeyState::WaitingCombo {
                    return true;
                }
                // Check if this key is part of the triggered combo
                !combo_actions.contains(&item.action)
            });

            let mut new_event = event;
            new_event.pressed = true;
            self.process_key_action(&action, new_event, true).await;
            debug!("[Combo] {:?} triggered", action);
            embassy_time::Timer::after_millis(20).await;
            // Reset other combos' state
            self.reset_combo(key_action);
        }
    }

    // Reset combos that contain a key_action but not triggered yet
    fn reset_combo(&mut self, key_action: &KeyAction) {
        // Reset other sub-combo states
        self.keymap
            .borrow_mut()
            .behavior
            .combo
            .combos
            .iter_mut()
            .filter_map(|c| c.as_mut())
            .for_each(|c| {
                if c.is_all_pressed() && !c.is_triggered() && c.config.actions.contains(key_action) {
                    info!("Resetting combo: {:?}", c,);
                    c.reset();
                }
            });
    }

    /// Check combo before process keys.
    ///
    /// This function returns key action after processing combo, and a boolean indicates that if current returned key action is a combo output
    async fn process_combo(&mut self, key_action: &KeyAction, event: KeyboardEvent) -> (Option<KeyAction>, bool) {
        let current_layer = self.keymap.borrow().get_activated_layer();

        // First, when releasing a key, check whether there's untriggered combo, if so, triggerer it first
        if !event.pressed {
            self.trigger_delayed_combo(key_action, event).await;
        }

        let max_size_of_updated_combo = self
            .keymap
            .borrow_mut()
            .behavior
            .combo
            .combos
            .iter_mut()
            .filter_map(|c| c.as_mut())
            .map(|c| {
                if c.update(key_action, event, current_layer) {
                    info!("Updated combo: {:?}", c);
                    c.size()
                } else {
                    0
                }
            })
            .max();

        if event.pressed
            && let Some(max_size) = max_size_of_updated_combo
            && max_size > 0
        {
            // If the max_size > 0, there's at least one combo is updated
            let pressed_time = self.get_timer_value(event).unwrap_or(Instant::now());
            self.held_buffer.push(HeldKey::new(
                event,
                *key_action,
                KeyState::WaitingCombo,
                pressed_time,
                pressed_time + self.keymap.borrow().behavior.combo.timeout,
            ));

            // Only one combo is updated, and triggered
            let next_action = self
                .keymap
                .borrow_mut()
                .behavior
                .combo
                .combos
                .iter_mut()
                .filter_map(|c| c.as_mut())
                .find_map(|c| {
                    if c.is_all_pressed() && !c.is_triggered() && c.size() == max_size {
                        Some(c.trigger())
                    } else {
                        None
                    }
                });

            if let Some(next_action) = next_action {
                debug!("[Combo] {:?} triggered", next_action);
                self.held_buffer
                    .keys
                    .retain(|item| item.state != KeyState::WaitingCombo);
                self.reset_combo(key_action);
                return (Some(next_action), true);
            }
            (None, false)
        } else {
            // No combo is updated, dispatch combos
            if !event.pressed {
                info!("Releasing keys in combo: {:?} {:?}", event, key_action);

                let mut combo_output = None;
                let mut releasing_triggered_combo = false;

                for combo in self
                    .keymap
                    .borrow_mut()
                    .behavior
                    .combo
                    .combos
                    .iter_mut()
                    .filter_map(|c| c.as_mut())
                {
                    if combo.config.actions.contains(key_action) {
                        // Releasing a combo key in triggered combo
                        releasing_triggered_combo |= combo.is_triggered();
                        info!("[Combo] releasing: {:?}", combo);

                        // Release the combo key, check whether the combo is fully released
                        if combo.update_released(key_action) {
                            // If the combo is fully released, update the combo output
                            debug!("[Combo] {:?} is released", combo.config.output);
                            combo_output = combo_output.or(Some(combo.config.output));
                        }
                    }
                }

                // Releasing a triggered combo
                // - Return the output of the triggered combo when the combo is fully released
                // - Return None when the combo is not fully released yet
                if releasing_triggered_combo {
                    return (combo_output, true);
                }
            }

            // When no key is updated(the combo is interruptted), or a key is released,
            self.dispatch_combos(key_action, event).await;
            (Some(*key_action), false)
        }
    }

    // Dispatch combo keys buffered in the held buffer when the combo isn't being triggered.
    async fn dispatch_combos(&mut self, key_action: &KeyAction, event: KeyboardEvent) {
        self.trigger_delayed_combo(key_action, event).await;

        // Dispatch all keys with state `WaitingCombo` in the held buffer
        let mut i = 0;
        while i < self.held_buffer.keys.len() {
            if self.held_buffer.keys[i].state == KeyState::WaitingCombo {
                let key = self.held_buffer.keys.swap_remove(i);
                debug!("[Combo] Dispatching combo: {:?}", key);
                self.process_key_action(&key.action, key.event, false).await;
            } else {
                i += 1;
            }
        }

        // Reset triggered combo states
        self.keymap
            .borrow_mut()
            .behavior
            .combo
            .combos
            .iter_mut()
            .filter_map(|combo| combo.as_mut())
            .filter(|combo| !combo.is_triggered())
            .for_each(Combo::reset);
    }

    async fn process_key_action_normal(&mut self, action: Action, event: KeyboardEvent) {
        match action {
            Action::No => {}
            Action::Key(key) => self.process_action_key(key, event).await,
            Action::LayerOn(layer_num) => self.process_action_layer_switch(layer_num, event),
            Action::LayerOff(layer_num) => {
                // Turn off a layer temporarily when the key is pressed
                // Reactivate the layer after the key is released
                if event.pressed {
                    self.keymap.borrow_mut().deactivate_layer(layer_num);
                }
            }
            Action::LayerToggle(layer_num) => {
                // Toggle a layer when the key is release
                if !event.pressed {
                    self.keymap.borrow_mut().toggle_layer(layer_num);
                }
            }
            Action::LayerToggleOnly(layer_num) => {
                // Activate a layer and deactivate all other layers(except default layer)
                if event.pressed {
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
                if event.pressed {
                    self.register_modifiers(modifiers);
                } else {
                    self.unregister_modifiers(modifiers);
                }
                //report the modifier press/release in its own hid report
                self.send_keyboard_report_with_resolved_modifiers(event.pressed).await;
                self.update_osl(event);
            }
            Action::TriggerMacro(macro_idx) => self.execute_macro(macro_idx, event).await,
            Action::KeyWithModifier(key_code, modifiers) => {
                if event.pressed {
                    // These modifiers will be combined into the hid report, so
                    // they will be "pressed" the same time as the key (in same hid report)
                    self.with_modifiers |= modifiers;
                } else {
                    // The modifiers will not be part of the hid report, so
                    // they will be "released" the same time as the key (in same hid report)
                    self.with_modifiers &= !(modifiers);
                }
                self.process_action_key(key_code, event).await
            }
            Action::LayerOnWithModifier(layer_num, modifiers) => {
                if event.pressed {
                    // These modifiers will be combined into the hid report, so
                    // they will be "pressed" the same time as the key (in same hid report)
                    self.held_modifiers |= modifiers;
                } else {
                    // The modifiers will not be part of the hid report, so
                    // they will be "released" the same time as the key (in same hid report)
                    self.held_modifiers &= !(modifiers);
                }
                self.process_action_layer_switch(layer_num, event);
                self.send_keyboard_report_with_resolved_modifiers(event.pressed).await
            }
            Action::OneShotLayer(l) => {
                self.process_action_osl(l, event).await;
                // Process OSM to avoid the OSL state stuck when an OSL is followed by an OSM
                self.update_osm(event);
            }
            Action::OneShotModifier(m) => {
                self.process_action_osm(m, event).await;
                // Process OSL to avoid the OSM state stuck when an OSM is followed by an OSL
                self.update_osl(event);
            }
            Action::OneShotKey(_k) => warn!("One-shot key is not supported: {:?}", action),
            Action::Light(_light_action) => warn!("Light controll is not supported"),
            Action::KeyboardControl(c) => self.process_action_keyboard_control(c, event).await,
            Action::Special(special_key) => self.process_action_special(special_key, event).await,
            Action::User(id) => self.process_user(id, event).await,
            Action::TriLayerLower => {
                // Tri-layer lower, turn layer 1 on and update layer state
                self.process_action_layer_switch(1, event);
                self.keymap.borrow_mut().update_fn_layer_state();
            }
            Action::TriLayerUpper => {
                // Tri-layer upper, turn layer 2 on and update layer state
                self.process_action_layer_switch(2, event);
                self.keymap.borrow_mut().update_fn_layer_state();
            }
        }
    }

    /// Tap action, send a key when the key is pressed, then release the key.
    async fn process_key_action_tap(&mut self, action: Action, mut event: KeyboardEvent) {
        debug!("TAP action: {:?}, {:?}", action, event);

        if event.pressed {
            self.process_key_action_normal(action, event).await;

            // Wait 10ms, then send release
            Timer::after_millis(10).await;

            event.pressed = false;
            self.process_key_action_normal(action, event).await;
        }
    }

    pub fn print_buffer(&self) {
        self.held_buffer
            .keys
            .iter()
            .enumerate()
            .for_each(|(i, k)| info!("\n✅Held buffer {}: {:?}, state: {:?}", i, k.event, k.state));
    }

    async fn process_action_osm(&mut self, modifiers: ModifierCombination, event: KeyboardEvent) {
        // Update one shot state
        if event.pressed {
            // Add new modifier combination to existing one shot or init if none
            self.osm_state = match self.osm_state {
                OneShotState::None => OneShotState::Initial(modifiers),
                OneShotState::Initial(m) => OneShotState::Initial(m | modifiers),
                OneShotState::Single(m) => OneShotState::Single(m | modifiers),
                OneShotState::Held(m) => OneShotState::Held(m | modifiers),
            };

            self.update_osl(event);
        } else {
            match self.osm_state {
                OneShotState::Initial(m) | OneShotState::Single(m) => {
                    self.osm_state = OneShotState::Single(m);
                    let timeout = Timer::after(self.keymap.borrow().behavior.one_shot.timeout);
                    match select(timeout, self.keyboard_event_subscriber.receive()).await {
                        Either::First(_) => {
                            // Timeout, release modifiers
                            self.update_osl(event);
                            self.osm_state = OneShotState::None;
                        }
                        Either::Second(e) => {
                            // New event, send it to queue
                            if self.unprocessed_events.push(e).is_err() {
                                warn!("Unprocessed event queue is full, dropping event");
                            }
                        }
                    }
                }
                OneShotState::Held(_) => {
                    // Release modifier
                    self.update_osl(event);
                    self.osm_state = OneShotState::None;

                    // This sends a separate hid report with the
                    // currently registered modifiers except the
                    // one shoot modifiers -> this way "releasing" them.
                    self.send_keyboard_report_with_resolved_modifiers(event.pressed).await;
                }
                _ => (),
            };
        }
    }

    async fn process_action_osl(&mut self, layer_num: u8, event: KeyboardEvent) {
        // Update one shot state
        if event.pressed {
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
                    match select(timeout, self.keyboard_event_subscriber.receive()).await {
                        Either::First(_) => {
                            // Timeout, deactivate layer
                            self.keymap.borrow_mut().deactivate_layer(layer_num);
                            self.osl_state = OneShotState::None;
                        }
                        Either::Second(e) => {
                            // New event, send it to queue
                            if self.unprocessed_events.push(e).is_err() {
                                warn!("Unprocessed event queue is full, dropping event");
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

    /// Calculates the combined effect of "explicit modifiers":
    /// - registered modifiers
    /// - one-shot modifiers
    pub fn resolve_explicit_modifiers(&self, pressed: bool) -> ModifierCombination {
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
    /// - effect of Action::KeyWithModifiers (while they are pressed)
    /// - possible fork related modifier suppressions
    pub fn resolve_modifiers(&mut self, pressed: bool) -> ModifierCombination {
        // Text typing macro should not be affected by any modifiers,
        // only its own capitalization
        if self.macro_texting {
            if self.macro_caps {
                return ModifierCombination::new().with_left_shift(true);
            } else {
                return ModifierCombination::new();
            }
        }

        // "explicit" modifiers: one-shot modifier, registered held modifiers:
        let mut result = self.resolve_explicit_modifiers(pressed);

        // The triggered forks suppress the 'match_any' modifiers automatically
        // unless they are configured as the 'kept_modifiers'
        let mut fork_suppress = ModifierCombination::default();
        for fork_state in self.fork_states.iter().flatten() {
            fork_suppress |= fork_state.suppress;
        }

        // Some of these suppressions could have been canceled after the fork activation
        // by "explicit" modifier key presses - fork_keep_mask collects these:
        fork_suppress &= !self.fork_keep_mask;

        // Execute the remaining suppressions
        result &= !fork_suppress;

        // Apply the modifiers from Action::KeyWithModifiers
        // the suppression effect of forks should not apply on these
        result |= self.with_modifiers;

        // Apply Caps Word shift
        if self.caps_word.is_active() && pressed && self.caps_word.is_shift_current() {
            result |= ModifierCombination::new().with_left_shift(true);
        }

        result
    }

    // Process a basic keypress/release and also take care of applying one shot modifiers
    async fn process_hid_keycode(&mut self, key: HidKeyCode, event: KeyboardEvent) {
        if event.pressed {
            self.register_key(key, event);
        } else {
            self.unregister_key(key, event);
        }

        self.send_keyboard_report_with_resolved_modifiers(event.pressed).await;
    }

    // Process action special keys
    async fn process_action_special(&mut self, key: SpecialKey, event: KeyboardEvent) {
        match key {
            SpecialKey::GraveEscape => {
                let hid_keycode = if self.held_modifiers.into_bits() == 0 {
                    HidKeyCode::Escape
                } else {
                    HidKeyCode::Grave
                };
                self.process_hid_keycode(hid_keycode, event).await;
            }
            SpecialKey::Repeat => {
                debug!("Repeat last key code: {:?} , {:?}", self.last_key_code, event);
                let key = self.last_key_code;
                self.process_action_key(key, event).await;
            }
        };
    }

    async fn process_action_keyboard_control(&mut self, keyboard_control: KeyboardAction, event: KeyboardEvent) {
        match keyboard_control {
            KeyboardAction::CapsWordToggle => {
                // Handle Caps Word
                if event.pressed {
                    self.caps_word.toggle();
                };
            }
            KeyboardAction::ComboOn => self.combo_on = true,
            KeyboardAction::ComboOff => self.combo_on = false,
            KeyboardAction::ComboToggle => self.combo_on = !self.combo_on,
            KeyboardAction::Bootloader => {
                // When releasing the key, process the boot action
                if !event.pressed {
                    boot::jump_to_bootloader();
                }
            }
            KeyboardAction::Reboot => {
                // When releasing the key, process the boot action
                if !event.pressed {
                    boot::reboot_keyboard();
                }
            }

            _ => warn!("KeyboardAction: {:?} is not supported yet", keyboard_control),
        }
    }

    // Process action key
    async fn process_action_key(&mut self, mut key: KeyCode, event: KeyboardEvent) {
        // Process `Again` key first.
        // Not all platform support `Again` key, so we manually repeat it for users.
        if key == KeyCode::Hid(HidKeyCode::Again) {
            debug!("Repeat(Again) last key code: {:?} , {:?}", self.last_key_code, event);
            key = self.last_key_code;
        }

        // Pre-check
        if event.pressed
            && let KeyCode::Hid(hid_keycode) = key
        {
            // Check hid keycodes
            // Record last press time
            if hid_keycode.is_simple_key() {
                // Records only the simple key
                self.last_press_time = Instant::now();
            }

            // Update last key code
            if hid_keycode != HidKeyCode::Again {
                debug!(
                    "Last key code changed from {:?} to {:?}(pressed: {:?})",
                    self.last_key_code, key, event.pressed
                );
                self.last_key_code = key;
            }

            // Check Caps Word
            self.caps_word.check(hid_keycode);
        }

        match key {
            KeyCode::Hid(hid_keycode) => {
                if let Some(consumer) = hid_keycode.process_as_consumer() {
                    self.process_action_consumer_control(consumer, event).await
                } else if let Some(system_control) = hid_keycode.process_as_system_control() {
                    self.process_action_system_control(system_control, event).await
                } else if hid_keycode.is_mouse_key() {
                    self.process_action_mouse(hid_keycode, event).await;
                } else {
                    // Basic keycodes
                    self.process_hid_keycode(hid_keycode, event).await
                }
            }
            KeyCode::Consumer(consumer) => self.process_action_consumer_control(consumer, event).await,
            KeyCode::SystemControl(system_control) => self.process_action_system_control(system_control, event).await,
        }

        self.update_osm(event);
        self.update_osl(event);
    }

    /// Process layer switch action.
    fn process_action_layer_switch(&mut self, layer_num: u8, event: KeyboardEvent) {
        // Change layer state only when the key's state is changed
        if event.pressed {
            self.keymap.borrow_mut().activate_layer(layer_num);
        } else {
            self.keymap.borrow_mut().deactivate_layer(layer_num);
        }
    }

    /// Process consumer control action. Consumer control keys are keys in hid consumer page, such as media keys.
    async fn process_action_consumer_control(&mut self, key: ConsumerKey, event: KeyboardEvent) {
        self.media_report.usage_id = if event.pressed { key as u16 } else { 0 };

        self.send_media_report().await;
    }

    /// Process system control action. System control keys are keys in system page, such as power key.
    async fn process_action_system_control(&mut self, key: SystemControlKey, event: KeyboardEvent) {
        if event.pressed {
            self.system_control_report.usage_id = key as u8;
            self.send_system_control_report().await;
        } else {
            self.system_control_report.usage_id = 0;
            self.send_system_control_report().await;
        }
    }

    /// Process mouse key action with acceleration support.
    async fn process_action_mouse(&mut self, key: HidKeyCode, event: KeyboardEvent) {
        if event.pressed {
            match key {
                HidKeyCode::MouseUp => {
                    // Reset repeat counter for direction change
                    if self.mouse_report.y > 0 {
                        self.mouse_repeat = 0;
                    }
                    let unit = self.calculate_mouse_move_unit();
                    self.mouse_report.y = -unit;
                }
                HidKeyCode::MouseDown => {
                    if self.mouse_report.y < 0 {
                        self.mouse_repeat = 0;
                    }
                    let unit = self.calculate_mouse_move_unit();
                    self.mouse_report.y = unit;
                }
                HidKeyCode::MouseLeft => {
                    if self.mouse_report.x > 0 {
                        self.mouse_repeat = 0;
                    }
                    let unit = self.calculate_mouse_move_unit();
                    self.mouse_report.x = -unit;
                }
                HidKeyCode::MouseRight => {
                    if self.mouse_report.x < 0 {
                        self.mouse_repeat = 0;
                    }
                    let unit = self.calculate_mouse_move_unit();
                    self.mouse_report.x = unit;
                }
                HidKeyCode::MouseWheelUp => {
                    if self.mouse_report.wheel < 0 {
                        self.mouse_wheel_repeat = 0;
                    }
                    let unit = self.calculate_mouse_wheel_unit();
                    self.mouse_report.wheel = unit;
                }
                HidKeyCode::MouseWheelDown => {
                    if self.mouse_report.wheel > 0 {
                        self.mouse_wheel_repeat = 0;
                    }
                    let unit = self.calculate_mouse_wheel_unit();
                    self.mouse_report.wheel = -unit;
                }
                HidKeyCode::MouseWheelLeft => {
                    if self.mouse_report.pan > 0 {
                        self.mouse_wheel_repeat = 0;
                    }
                    let unit = self.calculate_mouse_wheel_unit();
                    self.mouse_report.pan = -unit;
                }
                HidKeyCode::MouseWheelRight => {
                    if self.mouse_report.pan < 0 {
                        self.mouse_wheel_repeat = 0;
                    }
                    let unit = self.calculate_mouse_wheel_unit();
                    self.mouse_report.pan = unit;
                }
                HidKeyCode::MouseBtn1 => self.mouse_report.buttons |= 1 << 0,
                HidKeyCode::MouseBtn2 => self.mouse_report.buttons |= 1 << 1,
                HidKeyCode::MouseBtn3 => self.mouse_report.buttons |= 1 << 2,
                HidKeyCode::MouseBtn4 => self.mouse_report.buttons |= 1 << 3,
                HidKeyCode::MouseBtn5 => self.mouse_report.buttons |= 1 << 4,
                HidKeyCode::MouseBtn6 => self.mouse_report.buttons |= 1 << 5,
                HidKeyCode::MouseBtn7 => self.mouse_report.buttons |= 1 << 6,
                HidKeyCode::MouseBtn8 => self.mouse_report.buttons |= 1 << 7,
                HidKeyCode::MouseAccel0 => {
                    self.mouse_accel |= 1 << 0;
                }
                HidKeyCode::MouseAccel1 => {
                    self.mouse_accel |= 1 << 1;
                }
                HidKeyCode::MouseAccel2 => {
                    self.mouse_accel |= 1 << 2;
                }
                _ => {}
            }
        } else {
            match key {
                HidKeyCode::MouseUp => {
                    if self.mouse_report.y < 0 {
                        self.mouse_report.y = 0;
                    }
                }
                HidKeyCode::MouseDown => {
                    if self.mouse_report.y > 0 {
                        self.mouse_report.y = 0;
                    }
                }
                HidKeyCode::MouseLeft => {
                    if self.mouse_report.x < 0 {
                        self.mouse_report.x = 0;
                    }
                }
                HidKeyCode::MouseRight => {
                    if self.mouse_report.x > 0 {
                        self.mouse_report.x = 0;
                    }
                }
                HidKeyCode::MouseWheelUp => {
                    if self.mouse_report.wheel > 0 {
                        self.mouse_report.wheel = 0;
                    }
                }
                HidKeyCode::MouseWheelDown => {
                    if self.mouse_report.wheel < 0 {
                        self.mouse_report.wheel = 0;
                    }
                }
                HidKeyCode::MouseWheelLeft => {
                    if self.mouse_report.pan < 0 {
                        self.mouse_report.pan = 0;
                    }
                }
                HidKeyCode::MouseWheelRight => {
                    if self.mouse_report.pan > 0 {
                        self.mouse_report.pan = 0;
                    }
                }
                HidKeyCode::MouseBtn1 => self.mouse_report.buttons &= !(1 << 0),
                HidKeyCode::MouseBtn2 => self.mouse_report.buttons &= !(1 << 1),
                HidKeyCode::MouseBtn3 => self.mouse_report.buttons &= !(1 << 2),
                HidKeyCode::MouseBtn4 => self.mouse_report.buttons &= !(1 << 3),
                HidKeyCode::MouseBtn5 => self.mouse_report.buttons &= !(1 << 4),
                HidKeyCode::MouseBtn6 => self.mouse_report.buttons &= !(1 << 5),
                HidKeyCode::MouseBtn7 => self.mouse_report.buttons &= !(1 << 6),
                HidKeyCode::MouseBtn8 => self.mouse_report.buttons &= !(1 << 7),
                HidKeyCode::MouseAccel0 => {
                    self.mouse_accel &= !(1 << 0);
                }
                HidKeyCode::MouseAccel1 => {
                    self.mouse_accel &= !(1 << 1);
                }
                HidKeyCode::MouseAccel2 => {
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

        // Sync button state to keymap
        self.keymap.borrow_mut().mouse_buttons = self.mouse_report.buttons;

        if !matches!(
            key,
            HidKeyCode::MouseAccel0 | HidKeyCode::MouseAccel1 | HidKeyCode::MouseAccel2
        ) {
            // Send mouse report only for movement and wheel keys
            self.send_mouse_report().await;
        }

        // Continue processing ONLY for movement and wheel keys
        if event.pressed {
            let is_movement_key = matches!(
                key,
                HidKeyCode::MouseUp | HidKeyCode::MouseDown | HidKeyCode::MouseLeft | HidKeyCode::MouseRight
            );
            let is_wheel_key = matches!(
                key,
                HidKeyCode::MouseWheelUp
                    | HidKeyCode::MouseWheelDown
                    | HidKeyCode::MouseWheelLeft
                    | HidKeyCode::MouseWheelRight
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
                // Check if there's a release event in the channel, if there's no release event, re-send the event
                let len = self.keyboard_event_subscriber.len();
                let mut released = false;
                for _ in 0..len {
                    let queued_event = self.keyboard_event_subscriber.receive().await;
                    if queued_event.pos != event.pos || !queued_event.pressed {
                        publish_input_event_async(queued_event).await;
                    }
                    // If there's a release event in the channel
                    if queued_event.pos == event.pos && !queued_event.pressed {
                        released = true;
                    }
                }
                if !released {
                    publish_input_event_async(event).await;
                }
            }
        }
    }

    async fn process_user(&mut self, id: u8, event: KeyboardEvent) {
        debug!("Processing user key id: {:?}, event: {:?}", id, event);
        #[cfg(feature = "_ble")]
        {
            use crate::NUM_BLE_PROFILE;
            use crate::ble::profile::BleProfileAction;
            use crate::channel::BLE_PROFILE_CHANNEL;
            if event.pressed {
                // Clear Peer is processed when pressed
                if id == NUM_BLE_PROFILE as u8 + 4 {
                    #[cfg(all(feature = "split", feature = "_ble"))]
                    if event.pressed {
                        // Wait for 5s, if the key is still pressed, clear split peer info
                        // If there's any other key event received during this period, skip
                        match select(
                            embassy_time::Timer::after_millis(5000),
                            self.keyboard_event_subscriber.receive(),
                        )
                        .await
                        {
                            Either::First(_) => {
                                // Timeout reached, send clear peer message
                                #[cfg(feature = "controller")]
                                publish_controller_event(ClearPeerEvent);
                                info!("Clear peer");
                            }
                            Either::Second(e) => {
                                // Received a new key event before timeout, add to unprocessed list
                                if self.unprocessed_events.push(e).is_err() {
                                    warn!("Unprocessed event queue is full, dropping event");
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

    async fn execute_macro(&mut self, macro_idx: u8, event: KeyboardEvent) {
        // Execute the macro only when releasing the key
        if event.pressed {
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
                        self.register_key(k, event);
                        self.send_keyboard_report_with_resolved_modifiers(true).await;
                    }
                    MacroOperation::Release(k) => {
                        self.macro_texting = false;
                        self.unregister_key(k, event);
                        self.send_keyboard_report_with_resolved_modifiers(false).await;
                    }
                    MacroOperation::Tap(k) => {
                        self.macro_texting = false;
                        self.register_key(k, event);
                        self.send_keyboard_report_with_resolved_modifiers(true).await;
                        embassy_time::Timer::after_millis(2).await;
                        self.unregister_key(k, event);
                        self.send_keyboard_report_with_resolved_modifiers(false).await;
                    }
                    MacroOperation::Text(k, is_cap) => {
                        self.macro_texting = true;
                        self.macro_caps = is_cap;
                        if is_cap {
                            self.send_keyboard_report_with_resolved_modifiers(true).await;
                            embassy_time::Timer::after_millis(12).await;
                        }
                        self.register_keycode(k, event);
                        self.send_keyboard_report_with_resolved_modifiers(true).await;
                        embassy_time::Timer::after_millis(12).await;
                        self.unregister_keycode(k, event);
                        self.send_keyboard_report_with_resolved_modifiers(false).await;
                        if is_cap {
                            self.macro_caps = false;
                            embassy_time::Timer::after_millis(12).await;
                            self.send_keyboard_report_with_resolved_modifiers(false).await;
                        }
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
                embassy_time::Timer::after_millis(1).await;
            }
        } else {
            error!("Macro not found");
        }
    }

    pub(crate) async fn send_keyboard_report_with_resolved_modifiers(&mut self, pressed: bool) {
        // all modifier related effects are combined here to be sent with the hid report:
        let modifiers = self.resolve_modifiers(pressed);
        info!("Sending keyboard report, pressed: {}", pressed);
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

    fn update_osm(&mut self, event: KeyboardEvent) {
        match self.osm_state {
            OneShotState::Initial(m) => self.osm_state = OneShotState::Held(m),
            OneShotState::Single(_) => {
                if !event.pressed {
                    self.osm_state = OneShotState::None;
                }
            }
            _ => (),
        }
    }

    fn update_osl(&mut self, event: KeyboardEvent) {
        match self.osl_state {
            OneShotState::Initial(l) => self.osl_state = OneShotState::Held(l),
            OneShotState::Single(layer_num) => {
                if !event.pressed {
                    self.keymap.borrow_mut().deactivate_layer(layer_num);
                    self.osl_state = OneShotState::None;
                }
            }
            _ => (),
        }
    }

    /// Register a key, the key can be a basic keycode or a modifier.
    fn register_key(&mut self, key: HidKeyCode, event: KeyboardEvent) {
        if key.is_modifier() {
            self.register_modifier_key(key);
        } else {
            self.register_keycode(key, event);
        }
    }

    /// Unregister a key, the key can be a basic keycode or a modifier.
    fn unregister_key(&mut self, key: HidKeyCode, event: KeyboardEvent) {
        if key.is_modifier() {
            self.unregister_modifier_key(key);
        } else {
            self.unregister_keycode(key, event);
        }
    }

    /// Set the timer value for a key event
    fn set_timer_value(&mut self, event: KeyboardEvent, value: Option<Instant>) {
        match event.pos {
            KeyboardEventPos::Key(pos) => {
                self.timer[pos.col as usize][pos.row as usize] = value;
            }
            KeyboardEventPos::RotaryEncoder(encoder_pos) => {
                // Check if the rotary encoder id is valid
                if let Some(encoder) = self.rotary_encoder_timer.get_mut(encoder_pos.id as usize)
                    && encoder_pos.direction != Direction::None
                {
                    encoder[encoder_pos.direction as usize] = value;
                }
            }
        }
    }

    /// Get the timer value for a key event, if the key event is not in the timer, return the current time
    fn get_timer_value(&self, event: KeyboardEvent) -> Option<Instant> {
        match event.pos {
            KeyboardEventPos::Key(pos) => self.timer[pos.col as usize][pos.row as usize],
            KeyboardEventPos::RotaryEncoder(encoder_pos) => {
                // Check if the rotary encoder id is valid
                if let Some(encoder) = self.rotary_encoder_timer.get(encoder_pos.id as usize)
                    && encoder_pos.direction != Direction::None
                {
                    return encoder[encoder_pos.direction as usize];
                }
                None
            }
        }
    }

    /// Register a key to be sent in hid report.
    fn register_keycode(&mut self, key: HidKeyCode, event: KeyboardEvent) {
        // First, find the key event slot according to the position
        let slot = self.registered_keys.iter().enumerate().find_map(|(i, k)| {
            if let Some(e) = k
                && event.pos == e.pos
            {
                return Some(i);
            }
            None
        });

        // If the slot is found, update the key in the slot
        if let Some(index) = slot {
            self.held_keycodes[index] = key;
            self.registered_keys[index] = Some(event);
        } else {
            // Otherwise, find the first free slot
            if let Some(index) = self.held_keycodes.iter().position(|&k| k == HidKeyCode::No) {
                self.held_keycodes[index] = key;
                self.registered_keys[index] = Some(event);
            }
        }
    }

    /// Unregister a key from hid report.
    fn unregister_keycode(&mut self, key: HidKeyCode, event: KeyboardEvent) {
        // First, find the key event slot according to the position
        let slot = self.registered_keys.iter().enumerate().find_map(|(i, k)| {
            if let Some(e) = k
                && event.pos == e.pos
            {
                return Some(i);
            }
            None
        });

        // If the slot is found, update the key in the slot
        if let Some(index) = slot {
            self.held_keycodes[index] = HidKeyCode::No;
            self.registered_keys[index] = None;
        } else {
            // Otherwise, release the first same key
            if let Some(index) = self.held_keycodes.iter().position(|&k| k == key) {
                self.held_keycodes[index] = HidKeyCode::No;
                self.registered_keys[index] = None;
            }
        }
    }

    /// Register a modifier to be sent in hid report.
    fn register_modifier_key(&mut self, key: HidKeyCode) {
        self.held_modifiers |= key.to_hid_modifiers();

        #[cfg(feature = "controller")]
        publish_controller_event(ModifierEvent {
            modifier: self.held_modifiers,
        });

        // if a modifier key arrives after fork activation, it should be kept
        self.fork_keep_mask |= key.to_hid_modifiers();
    }

    /// Unregister a modifier from hid report.
    fn unregister_modifier_key(&mut self, key: HidKeyCode) {
        self.held_modifiers &= !key.to_hid_modifiers();

        #[cfg(feature = "controller")]
        publish_controller_event(ModifierEvent {
            modifier: self.held_modifiers,
        });
    }

    /// Register a modifier combination to be sent in hid report.
    fn register_modifiers(&mut self, modifiers: ModifierCombination) {
        self.held_modifiers |= modifiers;

        #[cfg(feature = "controller")]
        publish_controller_event(ModifierEvent {
            modifier: self.held_modifiers,
        });

        // if a modifier key arrives after fork activation, it should be kept
        self.fork_keep_mask |= modifiers;
    }

    /// Unregister a modifier combination from hid report.
    fn unregister_modifiers(&mut self, modifiers: ModifierCombination) {
        self.held_modifiers &= !modifiers;

        #[cfg(feature = "controller")]
        publish_controller_event(ModifierEvent {
            modifier: self.held_modifiers,
        });
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
                if x > 0 { 1 } else { -1 }
            } else {
                x_compensated as i8
            };

            y = if y_compensated == 0 && y != 0 {
                if y > 0 { 1 } else { -1 }
            } else {
                y_compensated as i8
            };
        }
        (x, y)
    }
}

#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum KeyBehaviorDecision {
    // Clean holding buffer due to permissive hold is triggered
    CleanBuffer,
    // Skip key action processing and buffer key event
    Buffer,
    // Continue processing as normal key event
    Ignore,
    // Release current key
    Release,
    // Flow tap of current key is triggered
    FlowTap,
}

#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum HeldKeyDecision {
    // Ignore it
    Ignore,
    // Unilateral tap triggered
    UnilateralTap,
    // Flow tap triggered, all held morse keys should be triggered as tapping
    FlowTap,
    // Permissive hold triggered
    PermissiveHold,
    // Hold on other key press triggered
    HoldOnOtherKeyPress,
    // Used for the buffered key which is releasing now
    Release,
    // Releasing a key that is pressed before any keys in the buffer
    NotInBuffer,
    // The held key is a normal key,
    // It will always be added to the decision list, and the decision will be made later
    Normal,
}

#[cfg(test)]
mod test {

    use embassy_futures::block_on;
    use embassy_time::{Duration, Timer};
    use rmk_types::action::{KeyAction, MorseMode, MorseProfile};
    use rmk_types::modifier::ModifierCombination;
    use rusty_fork::rusty_fork_test;

    use super::*;
    use crate::combo::{Combo, ComboConfig};
    use crate::config::{BehaviorConfig, CombosConfig, ForksConfig, PositionalConfig};
    use crate::event::{KeyPos, KeyboardEvent, KeyboardEventPos};
    use crate::fork::Fork;
    use crate::{a, k, layer, mo, th, thp};

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
                [k!(Escape), thp!(A, LShift, MorseProfile::new(Some(true), Some(MorseMode::PermissiveHold),None,None)), th!(S, LGui), k!(D), k!(F), k!(G), k!(H), k!(J), k!(K), k!(L), k!(Semicolon), k!(Quote), a!(No), k!(Enter)],
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
            combos: [
                Some(Combo::new(ComboConfig {
                    actions: [
                        k!(V), //3,4
                        k!(B), //3,5
                        k!(No), k!(No),
                    ],
                    output: k!(LShift),
                    layer: Some(0),
                })),
                Some(Combo::new(ComboConfig {
                    actions: [
                        k!(R), //1,4
                        k!(T), //1,5
                        k!(No), k!(No),
                    ],
                    output: k!(LAlt),
                    layer: Some(0),
                })),
                None, None, None, None, None, None
            ],
            timeout: Duration::from_millis(100),
        }
    }

    fn create_test_keyboard_with_config(config: BehaviorConfig) -> Keyboard<'static, 5, 14, 2> {
        static BEHAVIOR_CONFIG: static_cell::StaticCell<BehaviorConfig> = static_cell::StaticCell::new();
        let behavior_config = BEHAVIOR_CONFIG.init(config);

        // Box::leak is acceptable in tests
        let keymap = Box::new(get_keymap());
        let leaked_keymap = Box::leak(keymap);

        static KEY_CONFIG: static_cell::StaticCell<PositionalConfig<5, 14>> = static_cell::StaticCell::new();
        let per_key_config = KEY_CONFIG.init(PositionalConfig::default());
        let keymap = block_on(KeyMap::new(leaked_keymap, None, behavior_config, per_key_config));
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

    fn event(row: u8, col: u8, pressed: bool) -> KeyboardEvent {
        KeyboardEvent::key(row, col, pressed)
    }

    rusty_fork_test! {
        #[test]
        fn test_register_key() {
            let main = async {
                let mut keyboard = create_test_keyboard();
                keyboard.register_key(HidKeyCode::A, KeyboardEvent::key(2, 1, true));
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::A);
            };
            block_on(main);
        }

        #[test]
        fn test_basic_key_press_release() {
            let main = async {
                let mut keyboard = create_test_keyboard();

                // Press A key
                keyboard.process_inner(KeyboardEvent::key(0, 0, true)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::Grave); // A key's HID code is 0x04

                // Release A key
                keyboard.process_inner(KeyboardEvent::key(0, 0, false)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::No);
            };
            block_on(main);
        }

        #[test]
        fn test_modifier_key() {
            let main = async {
                let mut keyboard = create_test_keyboard();

                // Press Shift key
                keyboard.register_key(HidKeyCode::LShift, KeyboardEvent::key(3, 0, true));
                assert_eq!(keyboard.held_modifiers, ModifierCombination::new().with_left_shift(true)); // Left Shift's modifier bit is 0x02

                // Release Shift key
                keyboard.unregister_key(HidKeyCode::LShift, KeyboardEvent::key(3, 0, false));
                assert_eq!(keyboard.held_modifiers, ModifierCombination::new());
            };
            block_on(main);
        }

        #[test]
        fn test_multiple_keys() {
            let main = async {
                let mut keyboard = create_test_keyboard();

                keyboard.process_inner(KeyboardEvent::key(0, 0, true)).await;
                assert!(keyboard.held_keycodes.contains(&HidKeyCode::Grave));

                keyboard.process_inner(KeyboardEvent::key(1, 0, true)).await;
                assert!(keyboard.held_keycodes.contains(&HidKeyCode::Grave) && keyboard.held_keycodes.contains(&HidKeyCode::Tab));

                keyboard.process_inner(KeyboardEvent::key(1, 0, false)).await;
                assert!(keyboard.held_keycodes.contains(&HidKeyCode::Grave) && !keyboard.held_keycodes.contains(&HidKeyCode::Tab));

                keyboard.process_inner(KeyboardEvent::key(0, 0, false)).await;
                assert!(!keyboard.held_keycodes.contains(&HidKeyCode::Grave));
                assert!(keyboard.held_keycodes.iter().all(|&k| k == HidKeyCode::No));
            };

            block_on(main);
        }

        #[test]
        fn test_repeat_key_single() {
            let main = async {
                let mut keyboard = create_test_keyboard();
                keyboard.keymap.borrow_mut().set_action_at(
                    KeyboardEventPos::Key(KeyPos { row: 0, col: 0 }),
                    0,
                    KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::Again))),
                );

                // first press ever of the Again issues KeyCode:No
                keyboard.process_inner(KeyboardEvent::key(0, 0, true)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::No); // A key's HID code is 0x04

                // Press A key
                keyboard.process_inner(KeyboardEvent::key(2, 0, true)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::Escape); // A key's HID code is 0x04

                // Release A key
                keyboard.process_inner(KeyboardEvent::key(2, 0, false)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::No);

                // after another key is pressed, that key is repeated
                keyboard.process_inner(KeyboardEvent::key(0, 0, true)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::Escape); // A key's HID code is 0x04

                // releasing the repeat key
                keyboard.process_inner(KeyboardEvent::key(0, 0, false)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::No); // A key's HID code is 0x04

                // Press S key
                keyboard.process_inner(KeyboardEvent::key(1, 2, true)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::W); // A key's HID code is 0x04

                // after another key is pressed, that key is repeated
                keyboard.process_inner(KeyboardEvent::key(0, 0, true)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::W); // A key's HID code is 0x04
            };
            block_on(main);
        }


        #[test]
        fn test_repeat_key_th() {
            let main = async {
                let mut keyboard = create_test_keyboard();
                keyboard.keymap.borrow_mut().set_action_at(
                    KeyboardEventPos::Key(KeyPos { row: 0, col: 0 }),
                    0,
                    KeyAction::TapHold(Action::Key(KeyCode::Hid(HidKeyCode::F)), Action::Key(KeyCode::Hid(HidKeyCode::Again)), Default::default()),
                );
                keyboard.keymap.borrow_mut().set_action_at(
                    KeyboardEventPos::Key(KeyPos { row: 2, col: 1 }),
                    0,
                    KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::A))),
                );
                keyboard.keymap.borrow_mut().set_action_at(
                    KeyboardEventPos::Key(KeyPos { row: 2, col: 2 }),
                    0,
                    KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::S))),
                );

                // Press down F
                // first press ever of the Again issues KeyCode:No
                keyboard.process_inner(KeyboardEvent::key(0, 0, true)).await;
                keyboard
                    .send_keyboard_report_with_resolved_modifiers(true)
                    .await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::No);
                // Release F
                keyboard.process_inner(KeyboardEvent::key(0, 0, false)).await;

                // Press A key
                keyboard.process_inner(KeyboardEvent::key(2, 1, true)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::A);

                // Release A key
                keyboard.process_inner(KeyboardEvent::key(2, 1, false)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::No);

                // Release F
                keyboard.process_inner(KeyboardEvent::key(0, 0, false)).await;

                // Here release event should make again into hold

                embassy_time::Timer::after_millis(200 as u64).await;
                // after another key is pressed, that key is repeated
                keyboard.process_inner(KeyboardEvent::key(0, 0, true)).await;
                force_timeout_first_hold(&mut keyboard).await;

                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::A);

                // releasing the repeat key
                keyboard.process_inner(KeyboardEvent::key(0, 0, false)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::No);

                // Press S key
                keyboard.process_inner(KeyboardEvent::key(2, 2, true)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::S);

                // after another key is pressed, that key is repeated
                keyboard.process_inner(KeyboardEvent::key(0, 0, true)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::S);
            };
            block_on(main);
        }

        #[test]
        fn test_key_action_transparent() {
            let main = async {
                let mut keyboard = create_test_keyboard();

                // Activate layer 1
                keyboard.process_action_layer_switch(1, KeyboardEvent::key(0, 0, true));

                // Press Transparent key (Q on lower layer)
                keyboard.process_inner(KeyboardEvent::key(1, 1, true)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::Q); // Q key's HID code is 0x14

                // Release Transparent key
                keyboard.process_inner(KeyboardEvent::key(1, 1, false)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::No);
            };
            block_on(main);
        }

        #[test]
        fn test_key_action_no() {
            let main = async {
                let mut keyboard = create_test_keyboard();

                // Press No key
                keyboard.process_inner(KeyboardEvent::key(4, 3, true)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::No);

                // Release No key
                keyboard.process_inner(KeyboardEvent::key(4, 3, false)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::No);
            };
            block_on(main);
        }


        #[test]
        fn test_fork_with_held_modifier() {
            let main = async {
                //{ trigger = "Dot", negative_output = "Dot", positive_output = "WM(Semicolon, LShift)", match_any = "LShift|RShift" },
                let fork1 = Fork {
                    trigger: KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::Dot))),
                    negative_output: KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::Dot))),
                    positive_output: KeyAction::Single(
                        Action::KeyWithModifier(KeyCode::Hid(HidKeyCode::Semicolon),
                        ModifierCombination::default().with_left_shift(true),)
                    ),
                    match_any: StateBits {
                        modifiers: ModifierCombination::default().with_left_shift(true).with_right_shift(true),
                        leds: LedIndicator::default(),
                        mouse: MouseButtons::default(),
                    },
                    match_none: StateBits::default(),
                    kept_modifiers: ModifierCombination::default(),
                    bindable: false,
                };

                //{ trigger = "Comma", negative_output = "Comma", positive_output = "Semicolon", match_any = "LShift|RShift" },
                let fork2 = Fork {
                    trigger: KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::Comma))),
                    negative_output: KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::Comma))),
                    positive_output: KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::Semicolon))),
                    match_any: StateBits {
                        modifiers: ModifierCombination::default().with_left_shift(true).with_right_shift(true),
                        leds: LedIndicator::default(),
                        mouse: MouseButtons::default(),
                    },
                    match_none: StateBits::default(),
                    kept_modifiers: ModifierCombination::default(),
                    bindable: false,
                };

                let mut keyboard = create_test_keyboard_with_forks(fork1, fork2);

                // Press Dot key, by itself it should emit '.'
                keyboard.process_inner(KeyboardEvent::key(3, 9, true)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::Dot);

                // Release Dot key
                keyboard.process_inner(KeyboardEvent::key(3, 9, false)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::No);

                // Press LShift key
                keyboard.process_inner(KeyboardEvent::key(3, 0, true)).await;

                // Press Dot key, with shift it should emit ':'
                keyboard.process_inner(KeyboardEvent::key(3, 9, true)).await;
                assert_eq!(
                    keyboard.resolve_modifiers(true),
                    ModifierCombination::new().with_left_shift(true)
                );
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::Semicolon);

                //Release Dot key
                keyboard.process_inner(KeyboardEvent::key(3, 9, false)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::No);
                assert_eq!(
                    keyboard.resolve_modifiers(false),
                    ModifierCombination::new().with_left_shift(true)
                );

                // Release LShift key
                keyboard.process_inner(KeyboardEvent::key(3, 0, false)).await;
                assert_eq!(keyboard.held_modifiers, ModifierCombination::new());
                assert_eq!(keyboard.resolve_modifiers(false), ModifierCombination::new());

                // Press Comma key, by itself it should emit ','
                keyboard.process_inner(KeyboardEvent::key(3, 8, true)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::Comma);

                // Release Dot key
                keyboard.process_inner(KeyboardEvent::key(3, 8, false)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::No);

                // Press LShift key
                keyboard.process_inner(KeyboardEvent::key(3, 0, true)).await;

                // Press Comma key, with shift it should emit ';' (shift is suppressed)
                keyboard.process_inner(KeyboardEvent::key(3, 8, true)).await;
                assert_eq!(keyboard.resolve_modifiers(true), ModifierCombination::new());
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::Semicolon);

                // Release Comma key, shift is still held
                keyboard.process_inner(KeyboardEvent::key(3, 8, false)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::No);
                assert_eq!(
                    keyboard.resolve_modifiers(false),
                    ModifierCombination::new().with_left_shift(true)
                );

                // Release LShift key
                keyboard.process_inner(KeyboardEvent::key(3, 0, false)).await;
                assert_eq!(keyboard.held_modifiers, ModifierCombination::new());
                assert_eq!(keyboard.resolve_modifiers(false), ModifierCombination::new());
            };

            block_on(main);
        }
        #[test]
        fn test_fork_with_held_mouse_button() {
            let main = async {
                //{ trigger = "Z", negative_output = "MouseBtn5", positive_output = "C", match_any = "LCtrl|RCtrl|LShift|RShift", kept_modifiers="LShift|RShift" },
                let fork1 = Fork {
                    trigger: KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::Z))),
                    negative_output: KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::MouseBtn5))),
                    positive_output: KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::C))),
                    match_any: StateBits {
                        modifiers: ModifierCombination::default()
                            .with_left_ctrl(true)
                            .with_right_ctrl(true)
                            .with_left_shift(true)
                            .with_right_shift(true),
                        leds: LedIndicator::default(),
                        mouse: MouseButtons::default(),
                    },
                    match_none: StateBits::default(),
                    kept_modifiers: ModifierCombination::default().with_left_shift(true).with_right_shift(true),
                    bindable: false,
                };

                //{ trigger = "A", negative_output = "S", positive_output = "D", match_any = "MouseBtn5" },
                let fork2 = Fork {
                    trigger: KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::A))),
                    negative_output: KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::S))),
                    positive_output: KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::D))),
                    match_any: StateBits {
                        modifiers: ModifierCombination::default(),
                        leds: LedIndicator::default(),
                        mouse: MouseButtons::default().with_button5(true),
                    },
                    match_none: StateBits::default(),
                    kept_modifiers: ModifierCombination::default(),
                    bindable: false,
                };

                let mut keyboard = create_test_keyboard_with_forks(fork1, fork2);

                // disable th on a
                keyboard.keymap.borrow_mut().set_action_at(
                    KeyboardEventPos::Key(KeyPos { row: 2, col: 1 }),
                    0,
                    KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::A))),
                );


                // Press Z key, by itself it should emit 'MouseBtn5'
                keyboard.process_inner(KeyboardEvent::key(3, 1, true)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::No);
                assert_eq!(keyboard.mouse_report.buttons, 1u8 << 4); // MouseBtn5

                // Release Z key
                keyboard.process_inner(KeyboardEvent::key(3, 1, false)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::No);
                assert_eq!(keyboard.mouse_report.buttons, 0);

                // Press LCtrl key
                keyboard.process_inner(KeyboardEvent::key(4, 0, true)).await;
                // Press LShift key
                keyboard.process_inner(KeyboardEvent::key(3, 0, true)).await;
                assert_eq!(
                    keyboard.resolve_modifiers(true),
                    ModifierCombination::new().with_left_ctrl(true).with_left_shift(true)
                );

                // Press 'Z' key, with Ctrl it should emit 'C', with suppressed ctrl, but kept shift
                keyboard.process_inner(KeyboardEvent::key(3, 1, true)).await;
                assert_eq!(
                    keyboard.resolve_modifiers(true),
                    ModifierCombination::new().with_left_shift(true)
                );
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::C);
                assert_eq!(keyboard.mouse_report.buttons, 0);

                // Release 'Z' key, suppression of ctrl is removed
                keyboard.process_inner(KeyboardEvent::key(3, 1, false)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::No);
                assert_eq!(
                    keyboard.resolve_modifiers(false),
                    ModifierCombination::new().with_left_ctrl(true).with_left_shift(true)
                );

                // Release LCtrl key
                keyboard.process_inner(KeyboardEvent::key(4, 0, false)).await;
                assert_eq!(
                    keyboard.resolve_modifiers(false),
                    ModifierCombination::new().with_left_shift(true)
                );

                // Release LShift key
                keyboard.process_inner(KeyboardEvent::key(3, 0, false)).await;
                assert_eq!(keyboard.resolve_modifiers(false), ModifierCombination::new());

                // Press 'A' key, by itself it should emit 'S'
                keyboard.process_inner(KeyboardEvent::key(2, 1, true)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::S);

                // Release 'A' key
                keyboard.process_inner(KeyboardEvent::key(2, 1, false)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::No);
                assert_eq!(keyboard.resolve_modifiers(false), ModifierCombination::new());
                assert_eq!(keyboard.mouse_report.buttons, 0);

                Timer::after(Duration::from_millis(200)).await; // wait a bit

                // Press Z key, by itself it should emit 'MouseBtn5'
                keyboard.process_inner(KeyboardEvent::key(3, 1, true)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::No);
                assert_eq!(keyboard.mouse_report.buttons, 1u8 << 4); // MouseBtn5 //this fails, but ok in debug - why?

                // Press 'A' key, with 'MouseBtn5' it should emit 'D'
                keyboard.process_inner(KeyboardEvent::key(2, 1, true)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::D);

                // Release Z (MouseBtn1) key, 'D' is still held
                keyboard.process_inner(KeyboardEvent::key(3, 1, false)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::D);

                // Release 'A' key -> releases 'D'
                keyboard.process_inner(KeyboardEvent::key(2, 1, false)).await;
                assert_eq!(keyboard.held_keycodes[0], HidKeyCode::No);
            };

            block_on(main);
        }
    }
}
