//! Host-side simulation helpers for RMK tests.
//!
//! This module deliberately stops at RMK's transport boundaries. It drives the
//! same keyboard task used by firmware, publishes input events, captures HID
//! reports, and dispatches host requests through the production protocol
//! services. It does not emulate USB enumeration, BLE radio state, or real GPIO
//! electrical behavior.

use std::boxed::Box;
#[cfg(all(feature = "storage", any(not(feature = "_no_usb"), feature = "_ble")))]
use std::future::Future;
#[cfg(all(feature = "storage", any(not(feature = "_no_usb"), feature = "_ble")))]
use std::pin::Pin;
use std::vec::Vec;

#[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
use embassy_futures::select::{Either, select};
#[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
use embassy_futures::yield_now;
#[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
#[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
use embassy_sync::signal::Signal;
use embassy_time::Duration;
#[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
use embassy_time::Timer;
#[cfg(all(feature = "storage", any(not(feature = "_no_usb"), feature = "_ble")))]
use embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash;
#[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
use futures::join;
#[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
use rmk_types::connection::ConnectionStatus;
#[cfg(not(feature = "_no_usb"))]
use rmk_types::connection::UsbState;
#[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
use rmk_types::keycode::HidKeyCode;
use rmk_types::morse::{Morse, MorsePattern, MorseProfile};
#[cfg(feature = "rynk")]
use rmk_types::protocol::rynk::RynkMessage;

#[cfg(not(feature = "_no_usb"))]
use crate::channel::USB_REPORT_CHANNEL;
#[cfg(any(feature = "host", not(feature = "_no_usb"), feature = "_ble"))]
use crate::config::RmkConfig;
#[cfg(all(feature = "storage", any(not(feature = "_no_usb"), feature = "_ble")))]
use crate::config::StorageConfig;
use crate::config::{BehaviorConfig, Hand, PositionalConfig};
#[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
use crate::core_traits::Runnable;
use crate::event::KeyboardEvent;
#[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
use crate::event::{AsyncEventPublisher, AsyncPublishableEvent, KeyboardEventPos};
#[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
use crate::hid::{KeyboardReport, Report};
use crate::input_device::rotary_encoder::Direction;
use crate::keyboard::Keyboard;
use crate::keyboard::combo::{Combo, ComboConfig};
use crate::keymap::{KeyMap, KeymapData};
#[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
use crate::state::CONNECTION_STATUS;
#[cfg(all(feature = "_no_usb", feature = "_ble"))]
use crate::state::set_ble_state;
#[cfg(not(feature = "_no_usb"))]
use crate::state::set_usb_state;
#[cfg(all(feature = "storage", any(not(feature = "_no_usb"), feature = "_ble")))]
use crate::storage::FLASH_OPERATION_FINISHED;
#[cfg(all(feature = "storage", any(not(feature = "_no_usb"), feature = "_ble")))]
use crate::storage::Storage;
use crate::types::action::{Action, EncoderAction, KeyAction};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

#[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
fn reset() {
    KeyboardEvent::publisher_async().clear();

    CONNECTION_STATUS.lock(|c| c.set(ConnectionStatus::default()));
    #[cfg(not(feature = "_no_usb"))]
    set_usb_state(UsbState::Configured);
    #[cfg(all(feature = "_no_usb", feature = "_ble"))]
    set_ble_state(rmk_types::ble::BleState::Connected);

    #[cfg(not(feature = "_no_usb"))]
    USB_REPORT_CHANNEL.clear();
    #[cfg(feature = "_ble")]
    crate::channel::BLE_REPORT_CHANNEL.clear();

    #[cfg(feature = "storage")]
    crate::channel::FLASH_CHANNEL.clear();
}

#[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
async fn receive_sim_report() -> Report {
    #[cfg(not(feature = "_no_usb"))]
    {
        USB_REPORT_CHANNEL.receive().await
    }
    #[cfg(all(feature = "_no_usb", feature = "_ble"))]
    {
        crate::channel::BLE_REPORT_CHANNEL.receive().await
    }
}

#[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
fn try_receive_sim_report() -> Option<Report> {
    #[cfg(not(feature = "_no_usb"))]
    {
        USB_REPORT_CHANNEL.try_receive().ok()
    }
    #[cfg(all(feature = "_no_usb", feature = "_ble"))]
    {
        crate::channel::BLE_REPORT_CHANNEL.try_receive().ok()
    }
}

async fn build_static_keymap_with_encoder<
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
>(
    keymap: [[[KeyAction; COL]; ROW]; NUM_LAYER],
    encoder_map: [[EncoderAction; NUM_ENCODER]; NUM_LAYER],
    behavior_config: BehaviorConfig,
    positional_config: PositionalConfig<ROW, COL>,
) -> &'static KeyMap<'static> {
    let data = Box::leak(Box::new(KeymapData::new_with_encoder(keymap, encoder_map)));
    let behavior_config = Box::leak(Box::new(behavior_config));
    let positional_config = Box::leak(Box::new(positional_config));
    let keymap = KeyMap::new(data, behavior_config, positional_config).await;
    Box::leak(Box::new(keymap))
}

#[cfg(all(feature = "storage", any(not(feature = "_no_usb"), feature = "_ble")))]
trait SimStorageRunner {
    fn run_until<'s>(
        &'s mut self,
        done: &'s Signal<CriticalSectionRawMutex, ()>,
    ) -> Pin<Box<dyn Future<Output = ()> + 's>>;
}

#[cfg(all(feature = "storage", any(not(feature = "_no_usb"), feature = "_ble")))]
impl<F, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize> SimStorageRunner
    for Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>
where
    F: AsyncNorFlash + 'static,
{
    fn run_until<'s>(
        &'s mut self,
        done: &'s Signal<CriticalSectionRawMutex, ()>,
    ) -> Pin<Box<dyn Future<Output = ()> + 's>> {
        Box::pin(async move {
            match select(self.run(), done.wait()).await {
                Either::First(_) => unreachable!("storage task should never return"),
                Either::Second(()) => {}
            }
        })
    }
}

#[cfg(all(feature = "storage", any(not(feature = "_no_usb"), feature = "_ble")))]
async fn drain_flash_until_done(done: &Signal<CriticalSectionRawMutex, ()>) {
    match select(crate::channel::drain_flash_channel_for_test(), done.wait()).await {
        Either::First(_) => unreachable!("flash drain should never return"),
        Either::Second(()) => {}
    }
}

#[cfg(all(feature = "storage", any(not(feature = "_no_usb"), feature = "_ble")))]
async fn build_static_keymap_with_storage<
    F,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
>(
    keymap: [[[KeyAction; COL]; ROW]; NUM_LAYER],
    encoder_map: [[EncoderAction; NUM_ENCODER]; NUM_LAYER],
    flash: F,
    storage_config: StorageConfig,
    behavior_config: BehaviorConfig,
    positional_config: PositionalConfig<ROW, COL>,
) -> (&'static KeyMap<'static>, Box<dyn SimStorageRunner>)
where
    F: AsyncNorFlash + 'static,
{
    let data = Box::leak(Box::new(KeymapData::new_with_encoder(keymap, encoder_map)));
    let behavior_config = Box::leak(Box::new(behavior_config));
    let positional_config = Box::leak(Box::new(positional_config));
    let (keymap, storage) =
        crate::initialize_keymap_and_storage(data, flash, &storage_config, behavior_config, positional_config).await;
    (Box::leak(Box::new(keymap)), Box::new(storage))
}

#[derive(Clone, Copy, Debug)]
pub struct KeymapOverride {
    layer: usize,
    row: usize,
    col: usize,
    action: KeyAction,
}

impl KeymapOverride {
    pub const fn new(layer: usize, row: usize, col: usize, action: KeyAction) -> Self {
        Self {
            layer,
            row,
            col,
            action,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct HandOverride {
    row: usize,
    col: usize,
    hand: Hand,
}

impl HandOverride {
    pub const fn new(row: usize, col: usize, hand: Hand) -> Self {
        Self { row, col, hand }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SimKeyboardSetup {
    key_overrides: &'static [KeymapOverride],
    hand_overrides: &'static [HandOverride],
    morse_patterns: &'static [(u16, Action)],
    vial_morses: &'static [(Action, Action, Action, Action, MorseProfile)],
    morse_profile: Option<MorseProfile>,
    morse_flow_tap: Option<bool>,
    morse_prior_idle_ms: Option<u64>,
}

impl SimKeyboardSetup {
    pub const fn new() -> Self {
        Self {
            key_overrides: &[],
            hand_overrides: &[],
            morse_patterns: &[],
            vial_morses: &[],
            morse_profile: None,
            morse_flow_tap: None,
            morse_prior_idle_ms: None,
        }
    }

    pub const fn keys(mut self, key_overrides: &'static [KeymapOverride]) -> Self {
        self.key_overrides = key_overrides;
        self
    }

    pub const fn hand_overrides(mut self, hands: &'static [HandOverride]) -> Self {
        self.hand_overrides = hands;
        self
    }

    pub const fn morse_patterns(mut self, patterns: &'static [(u16, Action)]) -> Self {
        self.morse_patterns = patterns;
        self
    }

    pub const fn vial_morses(mut self, morses: &'static [(Action, Action, Action, Action, MorseProfile)]) -> Self {
        self.vial_morses = morses;
        self
    }

    pub const fn morse_profile(mut self, profile: MorseProfile) -> Self {
        self.morse_profile = Some(profile);
        self
    }

    pub const fn morse_flow_tap(mut self, enable: bool) -> Self {
        self.morse_flow_tap = Some(enable);
        self
    }

    pub const fn morse_prior_idle_ms(mut self, prior_idle_ms: u64) -> Self {
        self.morse_prior_idle_ms = Some(prior_idle_ms);
        self
    }
}

impl Default for SimKeyboardSetup {
    fn default() -> Self {
        Self::new()
    }
}

pub struct NoSimStorage;

#[cfg(all(feature = "storage", any(not(feature = "_no_usb"), feature = "_ble")))]
pub struct SimStorage<F> {
    flash: F,
}

#[cfg(all(feature = "storage", any(not(feature = "_no_usb"), feature = "_ble")))]
impl<F> SimStorage<F> {
    fn new(flash: F) -> Self {
        Self { flash }
    }
}

pub struct SimKeyboardBuilder<
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
    const NUM_ENCODER: usize,
    S = NoSimStorage,
> {
    keymap: [[[KeyAction; COL]; ROW]; NUM_LAYER],
    encoder_map: [[EncoderAction; NUM_ENCODER]; NUM_LAYER],
    behavior_config: BehaviorConfig,
    positional_config: PositionalConfig<ROW, COL>,
    #[cfg(feature = "host")]
    host_config: Option<RmkConfig<'static>>,
    storage: S,
}

impl<const ROW: usize, const COL: usize, const NUM_LAYER: usize> SimKeyboardBuilder<ROW, COL, NUM_LAYER, 0> {
    fn new(keymap: [[[KeyAction; COL]; ROW]; NUM_LAYER]) -> Self {
        Self {
            keymap,
            encoder_map: [const { [] }; NUM_LAYER],
            behavior_config: BehaviorConfig::default(),
            positional_config: PositionalConfig::default(),
            #[cfg(feature = "host")]
            host_config: None,
            storage: NoSimStorage,
        }
    }
}

impl<const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize, S>
    SimKeyboardBuilder<ROW, COL, NUM_LAYER, NUM_ENCODER, S>
{
    pub fn key(mut self, layer: usize, row: usize, col: usize, action: KeyAction) -> Self {
        self.keymap[layer][row][col] = action;
        self
    }

    pub fn setup(mut self, setup: SimKeyboardSetup) -> Self {
        for key in setup.key_overrides {
            self.keymap[key.layer][key.row][key.col] = key.action;
        }
        for hand in setup.hand_overrides {
            self.positional_config.hand[hand.row][hand.col] = hand.hand;
        }
        if let Some(enable) = setup.morse_flow_tap {
            self = self.morse_flow_tap(enable);
        }
        if let Some(prior_idle_ms) = setup.morse_prior_idle_ms {
            self = self.morse_prior_idle_ms(prior_idle_ms);
        }
        if let Some(profile) = setup.morse_profile {
            self = self.morse_default_profile(profile);
        }
        if !setup.morse_patterns.is_empty() {
            self = self.morse_patterns_slice(setup.morse_patterns);
        }
        if !setup.vial_morses.is_empty() {
            self = self.morses_from_vial_slice(setup.vial_morses);
        }
        self
    }

    pub fn morse_default_profile(mut self, profile: MorseProfile) -> Self {
        self.behavior_config.morse.default_profile = profile;
        self
    }

    pub fn morse_flow_tap(mut self, enable: bool) -> Self {
        self.behavior_config.morse.enable_flow_tap = enable;
        self
    }

    pub fn morse_prior_idle_ms(mut self, prior_idle_ms: u64) -> Self {
        self.behavior_config.morse.prior_idle_time = Duration::from_millis(prior_idle_ms);
        self
    }

    pub fn morse(mut self, morse: Morse) -> Self {
        self.behavior_config
            .morse
            .morses
            .push(morse)
            .expect("simulator morse config exceeds MORSE_MAX_NUM");
        self
    }

    pub fn morse_from_vial(
        self,
        tap: Action,
        hold: Action,
        hold_after_tap: Action,
        double_tap: Action,
        profile: MorseProfile,
    ) -> Self {
        self.morse(Morse::new_from_vial(tap, hold, hold_after_tap, double_tap, profile))
    }

    fn morses_from_vial_slice(mut self, morses: &[(Action, Action, Action, Action, MorseProfile)]) -> Self {
        for &(tap, hold, hold_after_tap, double_tap, profile) in morses {
            self = self.morse_from_vial(tap, hold, hold_after_tap, double_tap, profile);
        }
        self
    }

    fn morse_patterns_slice(self, patterns: &[(u16, Action)]) -> Self {
        self.morse(Morse {
            actions: heapless::LinearMap::from_iter(
                patterns
                    .iter()
                    .copied()
                    .map(|(pattern, action)| (MorsePattern::from_u16(pattern), action)),
            ),
            ..Default::default()
        })
    }

    fn combo<const NUM_ACTION: usize>(
        mut self,
        actions: [KeyAction; NUM_ACTION],
        output: KeyAction,
        layer: Option<u8>,
    ) -> Self {
        let combo = Combo::new(ComboConfig::new(actions, output, layer));
        let slot = self
            .behavior_config
            .combo
            .combos
            .iter_mut()
            .find(|combo| combo.is_none())
            .expect("simulator combo config exceeds COMBO_MAX_NUM");
        *slot = Some(combo);
        self
    }

    pub fn combo_on_layer<const NUM_ACTION: usize>(
        self,
        layer: u8,
        actions: [KeyAction; NUM_ACTION],
        output: KeyAction,
    ) -> Self {
        self.combo(actions, output, Some(layer))
    }

    pub fn combo_global<const NUM_ACTION: usize>(self, actions: [KeyAction; NUM_ACTION], output: KeyAction) -> Self {
        self.combo(actions, output, None)
    }

    pub fn combos_on_layer<const NUM_COMBO: usize, const NUM_ACTION: usize>(
        mut self,
        layer: u8,
        combos: [([KeyAction; NUM_ACTION], KeyAction); NUM_COMBO],
    ) -> Self {
        for (actions, output) in combos {
            self = self.combo_on_layer(layer, actions, output);
        }
        self
    }

    pub fn combos_global<const NUM_COMBO: usize, const NUM_ACTION: usize>(
        mut self,
        combos: [([KeyAction; NUM_ACTION], KeyAction); NUM_COMBO],
    ) -> Self {
        for (actions, output) in combos {
            self = self.combo_global(actions, output);
        }
        self
    }

    pub fn combo_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.behavior_config.combo.timeout = Duration::from_millis(timeout_ms);
        self
    }

    pub fn one_shot_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.behavior_config.one_shot.timeout = Duration::from_millis(timeout_ms);
        self
    }

    pub fn one_shot_activate_on_keypress(mut self, activate_on_keypress: bool) -> Self {
        self.behavior_config.one_shot_modifiers.activate_on_keypress = activate_on_keypress;
        self
    }

    pub fn one_shot_quick_release(mut self, quick_release: bool) -> Self {
        self.behavior_config.one_shot_modifiers.quick_release = quick_release;
        self
    }

    pub fn macro_sequences(mut self, macro_sequences: [u8; crate::MACRO_SPACE_SIZE]) -> Self {
        self.behavior_config.keyboard_macros.macro_sequences = macro_sequences;
        self
    }

    pub fn encoders<const NEW_NUM_ENCODER: usize>(
        self,
        encoder_map: [[EncoderAction; NEW_NUM_ENCODER]; NUM_LAYER],
    ) -> SimKeyboardBuilder<ROW, COL, NUM_LAYER, NEW_NUM_ENCODER, S> {
        SimKeyboardBuilder {
            keymap: self.keymap,
            encoder_map,
            behavior_config: self.behavior_config,
            positional_config: self.positional_config,
            #[cfg(feature = "host")]
            host_config: self.host_config,
            storage: self.storage,
        }
    }

    #[cfg(feature = "host")]
    pub fn host_config(mut self, rmk_config: RmkConfig<'static>) -> Self {
        self.host_config = Some(rmk_config);
        self
    }

    #[cfg(all(feature = "storage", any(not(feature = "_no_usb"), feature = "_ble")))]
    pub fn storage_flash<F>(self, flash: F) -> SimKeyboardBuilder<ROW, COL, NUM_LAYER, NUM_ENCODER, SimStorage<F>>
    where
        F: AsyncNorFlash + 'static,
    {
        SimKeyboardBuilder {
            keymap: self.keymap,
            encoder_map: self.encoder_map,
            behavior_config: self.behavior_config,
            positional_config: self.positional_config,
            #[cfg(feature = "host")]
            host_config: self.host_config,
            storage: SimStorage::new(flash),
        }
    }
}

impl<const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    SimKeyboardBuilder<ROW, COL, NUM_LAYER, NUM_ENCODER, NoSimStorage>
{
    pub async fn build(self) -> SimKeyboard<'static> {
        let keymap = build_static_keymap_with_encoder(
            self.keymap,
            self.encoder_map,
            self.behavior_config,
            self.positional_config,
        )
        .await;
        let keyboard = SimKeyboard::new(Keyboard::new(keymap));
        #[cfg(feature = "host")]
        let keyboard = {
            let mut keyboard = keyboard;
            keyboard.host_config = self.host_config;
            keyboard
        };
        keyboard
    }
}

#[cfg(all(feature = "storage", any(not(feature = "_no_usb"), feature = "_ble")))]
impl<F, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    SimKeyboardBuilder<ROW, COL, NUM_LAYER, NUM_ENCODER, SimStorage<F>>
where
    F: AsyncNorFlash + 'static,
{
    pub async fn build(self) -> SimKeyboard<'static> {
        let (keymap, storage) = build_static_keymap_with_storage(
            self.keymap,
            self.encoder_map,
            self.storage.flash,
            StorageConfig::default(),
            self.behavior_config,
            self.positional_config,
        )
        .await;
        let keyboard = SimKeyboard::new(Keyboard::new(keymap)).with_storage(storage);
        #[cfg(feature = "host")]
        let keyboard = {
            let mut keyboard = keyboard;
            keyboard.host_config = self.host_config;
            keyboard
        };
        keyboard
    }
}

#[derive(Debug)]
enum SimStep {
    Event(KeyboardEvent),
    Delay(Duration),
    #[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
    ExpectKeyboardState {
        modifier: u8,
        keycodes: Vec<u8>,
    },
    #[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
    ExpectReport(Report),
    #[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
    ExpectNoReport(Duration),
    #[cfg(feature = "vial")]
    VialPacket {
        data: [u8; 32],
        expected: [u8; 32],
    },
    #[cfg(feature = "rynk")]
    RynkPacket {
        request: Vec<u8>,
        expected: Vec<u8>,
    },
    #[cfg(all(feature = "storage", any(not(feature = "_no_usb"), feature = "_ble")))]
    WaitStorage,
    #[cfg(feature = "passkey_entry")]
    BeginPasskeyEntry,
    #[cfg(feature = "passkey_entry")]
    ExpectPasskeyResponse(Option<u32>),
    #[cfg(feature = "passkey_entry")]
    EndPasskeyEntry,
}

/// Simulator for a complete keyboard device, excluding physical input and
/// transport I/O.
pub struct SimKeyboard<'a> {
    keyboard: Keyboard<'a>,
    steps: Vec<SimStep>,
    #[cfg(all(feature = "storage", any(not(feature = "_no_usb"), feature = "_ble")))]
    storage: Option<Box<dyn SimStorageRunner>>,
    #[cfg(feature = "host")]
    host_config: Option<RmkConfig<'static>>,
}

impl<'a> SimKeyboard<'a> {
    fn new(keyboard: Keyboard<'a>) -> Self {
        Self {
            keyboard,
            steps: Vec::new(),
            #[cfg(all(feature = "storage", any(not(feature = "_no_usb"), feature = "_ble")))]
            storage: None,
            #[cfg(feature = "host")]
            host_config: None,
        }
    }

    #[cfg(all(feature = "storage", any(not(feature = "_no_usb"), feature = "_ble")))]
    fn with_storage(mut self, storage: Box<dyn SimStorageRunner>) -> Self {
        self.storage = Some(storage);
        self
    }

    pub fn keymap(&self) -> &'a KeyMap<'a> {
        self.keyboard.keymap
    }

    pub fn press(&mut self, row: u8, col: u8) -> &mut Self {
        self.event(KeyboardEvent::key(row, col, true))
    }

    pub fn release(&mut self, row: u8, col: u8) -> &mut Self {
        self.event(KeyboardEvent::key(row, col, false))
    }

    pub fn tap(&mut self, row: u8, col: u8, hold_ms: u64) -> &mut Self {
        self.press(row, col).delay(hold_ms).release(row, col)
    }

    pub fn delay(&mut self, ms: u64) -> &mut Self {
        self.steps.push(SimStep::Delay(Duration::from_millis(ms)));
        self
    }

    pub fn rotary_cw(&mut self, id: u8) -> &mut Self {
        self.rotary(id, Direction::Clockwise)
    }

    pub fn rotary_ccw(&mut self, id: u8) -> &mut Self {
        self.rotary(id, Direction::CounterClockwise)
    }

    fn rotary(&mut self, id: u8, direction: Direction) -> &mut Self {
        self.event(KeyboardEvent::rotary_encoder(id, direction, true))
            .event(KeyboardEvent::rotary_encoder(id, direction, false))
    }

    pub fn event(&mut self, event: KeyboardEvent) -> &mut Self {
        self.steps.push(SimStep::Event(event));
        self
    }

    #[cfg(feature = "passkey_entry")]
    pub fn begin_passkey_entry(&mut self) -> &mut Self {
        self.steps.push(SimStep::BeginPasskeyEntry);
        self
    }

    #[cfg(feature = "passkey_entry")]
    pub fn expect_passkey_response(&mut self, expected: Option<u32>) -> &mut Self {
        self.steps.push(SimStep::ExpectPasskeyResponse(expected));
        self
    }

    #[cfg(feature = "passkey_entry")]
    pub fn end_passkey_entry(&mut self) -> &mut Self {
        self.steps.push(SimStep::EndPasskeyEntry);
        self
    }

    #[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
    pub fn expect_keys<const N: usize>(&mut self, keycodes: [HidKeyCode; N]) -> &mut Self {
        self.expect_keys_with_mods(0, keycodes)
    }

    #[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
    pub fn expect_keys_with_mods<const N: usize>(&mut self, modifier: u8, keycodes: [HidKeyCode; N]) -> &mut Self {
        let keycodes = keycodes.iter().map(|keycode| *keycode as u8).collect();
        self.expect_keyboard_state(modifier, keycodes)
    }

    #[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
    fn expect_keyboard_state(&mut self, modifier: u8, keycodes: Vec<u8>) -> &mut Self {
        assert!(
            keycodes.len() <= KeyboardReport::default().keycodes.len(),
            "keyboard HID reports can carry at most {} simultaneous keycodes",
            KeyboardReport::default().keycodes.len()
        );
        self.steps.push(SimStep::ExpectKeyboardState { modifier, keycodes });
        self
    }

    #[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
    pub fn expect_only_mods(&mut self, modifier: u8) -> &mut Self {
        self.expect_keyboard_state(modifier, Vec::new())
    }

    #[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
    pub fn expect_all_up(&mut self) -> &mut Self {
        self.expect_keyboard_state(0, Vec::new())
    }

    #[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
    pub fn expect_keyboard_report(&mut self, report: KeyboardReport) -> &mut Self {
        self.expect_report(Report::KeyboardReport(report))
    }

    #[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
    pub fn expect_report(&mut self, report: Report) -> &mut Self {
        self.steps.push(SimStep::ExpectReport(report));
        self
    }

    #[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
    pub fn expect_no_report(&mut self, ms: u64) -> &mut Self {
        self.steps.push(SimStep::ExpectNoReport(Duration::from_millis(ms)));
        self
    }

    #[cfg(feature = "host")]
    fn enable_host(&mut self) {
        if self.host_config.is_none() {
            self.host_config = Some(RmkConfig::default());
        }
    }

    #[cfg(feature = "vial")]
    fn vial_packet(&mut self, data: [u8; 32], expected: [u8; 32]) -> &mut Self {
        self.steps.push(SimStep::VialPacket { data, expected });
        self
    }

    #[cfg(all(feature = "storage", any(not(feature = "_no_usb"), feature = "_ble")))]
    pub fn wait_storage(&mut self) -> &mut Self {
        self.steps.push(SimStep::WaitStorage);
        self
    }

    #[cfg(feature = "rynk")]
    fn rynk_packet(&mut self, request: Vec<u8>, expected: Vec<u8>) -> &mut Self {
        self.steps.push(SimStep::RynkPacket { request, expected });
        self
    }

    #[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
    pub async fn run(&mut self) {
        let keyboard_done = Signal::<CriticalSectionRawMutex, ()>::new();
        #[cfg(all(feature = "storage", any(not(feature = "_no_usb"), feature = "_ble")))]
        let storage_done = Signal::<CriticalSectionRawMutex, ()>::new();
        #[cfg(all(feature = "storage", any(not(feature = "_no_usb"), feature = "_ble")))]
        let flash_drain_done = Signal::<CriticalSectionRawMutex, ()>::new();
        let steps = core::mem::take(&mut self.steps);
        #[cfg(feature = "host")]
        let protocol_config = self.host_config.as_ref();
        #[cfg(not(feature = "host"))]
        let protocol_config: Option<&RmkConfig<'static>> = None;

        reset();
        let keymap = self.keyboard.keymap;

        #[cfg(all(feature = "storage", any(not(feature = "_no_usb"), feature = "_ble")))]
        if let Some(storage) = self.storage.as_deref_mut() {
            join!(
                Self::run_keyboard_until_done(&mut self.keyboard, &keyboard_done),
                storage.run_until(&storage_done),
                flash_drain_done.wait(),
                async {
                    Self::run_steps(keymap, steps, DEFAULT_TIMEOUT, protocol_config).await;
                    keyboard_done.signal(());
                    storage_done.signal(());
                    flash_drain_done.signal(());
                }
            );

            self.assert_clean();
            return;
        }

        #[cfg(feature = "storage")]
        join!(
            Self::run_keyboard_until_done(&mut self.keyboard, &keyboard_done),
            async {
                Self::run_steps(keymap, steps, DEFAULT_TIMEOUT, protocol_config).await;
                keyboard_done.signal(());
                flash_drain_done.signal(());
            },
            drain_flash_until_done(&flash_drain_done)
        );
        #[cfg(not(feature = "storage"))]
        join!(
            Self::run_keyboard_until_done(&mut self.keyboard, &keyboard_done),
            async {
                Self::run_steps(keymap, steps, DEFAULT_TIMEOUT, protocol_config).await;
                keyboard_done.signal(());
            }
        );

        self.assert_clean();
    }

    fn assert_clean(&self) {
        if !self.keyboard.held_buffer.is_empty() {
            panic!(
                "leak after buffer cleanup, buffer contains {:?}",
                self.keyboard.held_buffer
            );
        }
        if !self.keyboard.unprocessed_events.is_empty() {
            panic!(
                "simulator ended with unprocessed keyboard events: {:?}",
                self.keyboard.unprocessed_events
            );
        }
    }

    #[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
    async fn run_keyboard_until_done(keyboard: &mut Keyboard<'_>, done: &Signal<CriticalSectionRawMutex, ()>) {
        match select(keyboard.run(), done.wait()).await {
            Either::First(_) => unreachable!("keyboard task should never return"),
            Either::Second(()) => {}
        }
    }

    #[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
    async fn run_steps<'k>(
        keymap: &'k KeyMap<'k>,
        steps: Vec<SimStep>,
        timeout: Duration,
        protocol_config: Option<&RmkConfig<'static>>,
    ) {
        #[cfg(not(any(feature = "vial", feature = "rynk")))]
        let _ = (keymap, protocol_config);

        let sender = KeyboardEvent::publisher_async();
        #[cfg(feature = "host")]
        let host_service = protocol_config.map(|config| crate::host::HostService::new(keymap, config));
        let mut pressed_inputs = Vec::<KeyboardEventPos>::new();

        for (idx, step) in steps.into_iter().enumerate() {
            match step {
                SimStep::Event(event) => {
                    if event.pressed {
                        assert!(
                            !pressed_inputs.contains(&event.pos),
                            "input {:?} was pressed twice without a release at step #{idx}",
                            event.pos
                        );
                        pressed_inputs.push(event.pos);
                    } else {
                        let Some(pos) = pressed_inputs.iter().position(|pressed| *pressed == event.pos) else {
                            panic!(
                                "input {:?} was released without a matching press at step #{idx}",
                                event.pos
                            );
                        };
                        pressed_inputs.swap_remove(pos);
                    }

                    match select(Timer::after(timeout), sender.publish_async(event)).await {
                        Either::First(_) => panic!("simulator timed out publishing keyboard event at step #{idx}"),
                        Either::Second(()) => {}
                    }
                }
                SimStep::Delay(duration) => {
                    Timer::after(duration).await;
                }
                SimStep::ExpectKeyboardState { modifier, keycodes } => {
                    let actual = match select(Timer::after(timeout), receive_sim_report()).await {
                        Either::First(_) => panic!("simulator timed out waiting for keyboard report at step #{idx}"),
                        Either::Second(report) => report,
                    };
                    Self::assert_keyboard_state_eq(modifier, keycodes, actual, idx);
                }
                SimStep::ExpectReport(expected) => {
                    let actual = match select(Timer::after(timeout), receive_sim_report()).await {
                        Either::First(_) => panic!("simulator timed out waiting for HID report at step #{idx}"),
                        Either::Second(report) => report,
                    };
                    Self::assert_report_eq(expected, actual, idx);
                }
                SimStep::ExpectNoReport(duration) => match select(Timer::after(duration), receive_sim_report()).await {
                    Either::First(_) => {}
                    Either::Second(report) => {
                        panic!("unexpected HID report at step #{idx}: {:?}", report);
                    }
                },
                #[cfg(feature = "vial")]
                SimStep::VialPacket { data, expected } => {
                    #[cfg(feature = "storage")]
                    FLASH_OPERATION_FINISHED.reset();
                    let service = host_service
                        .as_ref()
                        .expect("simulator Vial config must be enabled before running Vial steps");
                    let actual = match select(Timer::after(timeout), service.process_packet(data)).await {
                        Either::First(_) => panic!("simulator timed out dispatching Vial packet at step #{idx}"),
                        Either::Second(reply) => reply,
                    };
                    assert_eq!(expected, actual, "on Vial reply at step #{idx}");
                }
                #[cfg(feature = "rynk")]
                SimStep::RynkPacket { mut request, expected } => {
                    #[cfg(feature = "storage")]
                    FLASH_OPERATION_FINISHED.reset();
                    let service = host_service
                        .as_ref()
                        .expect("simulator Rynk config must be enabled before running Rynk steps");
                    let mut msg = RynkMessage::try_from(request.as_mut_slice())
                        .expect("simulator Rynk request should be a valid frame");
                    match select(Timer::after(timeout), service.dispatch(&mut msg)).await {
                        Either::First(_) => panic!("simulator timed out dispatching Rynk packet at step #{idx}"),
                        Either::Second(()) => {}
                    }
                    let frame_len = msg.frame_len();
                    assert_eq!(
                        expected,
                        request[..frame_len],
                        "on Rynk reply at step #{idx}: expected {:?}, actual {:?}",
                        expected,
                        &request[..frame_len]
                    );
                }
                #[cfg(all(feature = "storage", any(not(feature = "_no_usb"), feature = "_ble")))]
                SimStep::WaitStorage => match select(Timer::after(timeout), FLASH_OPERATION_FINISHED.wait()).await {
                    Either::First(_) => panic!("simulator timed out waiting for storage write at step #{idx}"),
                    Either::Second(true) => {}
                    Either::Second(false) => panic!("storage write failed at step #{idx}"),
                },
                #[cfg(feature = "passkey_entry")]
                SimStep::BeginPasskeyEntry => {
                    crate::ble::passkey::begin_passkey_entry_session();
                }
                #[cfg(feature = "passkey_entry")]
                SimStep::ExpectPasskeyResponse(expected) => {
                    match select(Timer::after(timeout), crate::ble::passkey::PASSKEY_RESPONSE.wait()).await {
                        Either::First(_) => panic!("simulator timed out waiting for passkey response at step #{idx}"),
                        Either::Second(actual) => assert_eq!(
                            expected, actual,
                            "on passkey response at step #{idx}: expected {:?}, actual {:?}",
                            expected, actual
                        ),
                    }
                }
                #[cfg(feature = "passkey_entry")]
                SimStep::EndPasskeyEntry => {
                    crate::ble::passkey::end_passkey_entry_session();
                }
            }
        }

        match select(Timer::after(timeout), async {
            while !sender.is_empty() {
                yield_now().await;
            }
        })
        .await
        {
            Either::First(_) => panic!("simulator timed out draining keyboard events after the final step"),
            Either::Second(()) => {}
        }

        // The queue becomes empty when the keyboard receives the final event;
        // allow that in-flight processing to finish before checking state.
        Timer::after(Duration::from_millis(1)).await;
        if let Some(report) = try_receive_sim_report() {
            panic!(
                "unexpected trailing HID report after final simulator step: {:?}",
                report
            );
        }
        assert!(
            pressed_inputs.is_empty(),
            "simulator ended with pressed inputs: {:?}",
            pressed_inputs
        );
    }

    #[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
    fn assert_report_eq(expected: Report, actual: Report, step_idx: usize) {
        use usbd_hid::descriptor::AsInputReport;

        let mut expected_buf = [0u8; 64];
        let mut actual_buf = [0u8; 64];
        let expected_len = expected
            .serialize(&mut expected_buf)
            .expect("expected report should serialize");
        let actual_len = actual
            .serialize(&mut actual_buf)
            .expect("actual report should serialize");

        assert_eq!(
            &expected_buf[..expected_len],
            &actual_buf[..actual_len],
            "on HID report at step #{step_idx}: expected {:?}, actual {:?}",
            expected,
            actual
        );
    }

    #[cfg(any(not(feature = "_no_usb"), feature = "_ble"))]
    fn assert_keyboard_state_eq(modifier: u8, mut keycodes: Vec<u8>, actual: Report, step_idx: usize) {
        let actual = match actual {
            Report::KeyboardReport(report) => report,
            report => panic!("expected keyboard report at step #{step_idx}, actual {:?}", report),
        };
        let mut actual_keycodes: Vec<u8> = actual
            .keycodes
            .iter()
            .copied()
            .filter(|keycode| *keycode != 0)
            .collect();
        keycodes.sort_unstable();
        actual_keycodes.sort_unstable();

        assert_eq!(
            modifier, actual.modifier,
            "on keyboard report modifier at step #{step_idx}: expected keycodes {:?}, actual {:?}",
            keycodes, actual
        );
        assert_eq!(
            keycodes, actual_keycodes,
            "on keyboard report keycodes at step #{step_idx}: actual {:?}",
            actual
        );
    }
}

impl SimKeyboard<'static> {
    pub fn builder<const ROW: usize, const COL: usize, const NUM_LAYER: usize>(
        keymap: [[[KeyAction; COL]; ROW]; NUM_LAYER],
    ) -> SimKeyboardBuilder<ROW, COL, NUM_LAYER, 0> {
        SimKeyboardBuilder::new(keymap)
    }

    pub async fn create<const ROW: usize, const COL: usize, const NUM_LAYER: usize>(
        keymap: [[[KeyAction; COL]; ROW]; NUM_LAYER],
    ) -> Self {
        Self::builder(keymap).build().await
    }
}

#[cfg(feature = "host")]
#[derive(Default)]
pub struct SimHost;

#[cfg(feature = "host")]
impl SimHost {
    pub const fn new() -> Self {
        Self
    }
}

#[cfg(feature = "storage")]
pub mod flash;
#[cfg(feature = "rynk")]
pub mod rynk;
#[cfg(feature = "vial")]
pub mod vial;

mod executor;

pub fn test_block_on<F: core::future::Future>(future: F) -> F::Output {
    executor::test_block_on(future)
}
