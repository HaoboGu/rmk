//! Host-side simulation helpers for RMK tests.
//!
//! This module deliberately stops at RMK's transport boundaries. It drives the
//! same keyboard task used by firmware, publishes input events, captures HID
//! reports, and dispatches host requests through the production protocol
//! services. It does not emulate USB enumeration, BLE radio state, or real GPIO
//! electrical behavior.

use std::boxed::Box;
#[cfg(all(feature = "storage", not(feature = "_no_usb")))]
use std::future::Future;
#[cfg(feature = "rynk")]
use std::marker::PhantomData;
#[cfg(all(feature = "storage", not(feature = "_no_usb")))]
use std::pin::Pin;
use std::vec::Vec;

#[cfg(any(not(feature = "_no_usb"), all(feature = "host", feature = "_ble")))]
use embassy_futures::select::{Either, select};
#[cfg(not(feature = "_no_usb"))]
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
#[cfg(not(feature = "_no_usb"))]
use embassy_sync::signal::Signal;
use embassy_time::Duration;
#[cfg(any(not(feature = "_no_usb"), all(feature = "host", feature = "_ble")))]
use embassy_time::Timer;
#[cfg(all(feature = "storage", not(feature = "_no_usb")))]
use embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash;
#[cfg(not(feature = "_no_usb"))]
use futures::join;
#[cfg(feature = "host")]
use rmk_types::connection::ConnectionType;
use rmk_types::connection::{ConnectionStatus, UsbState};
#[cfg(not(feature = "_no_usb"))]
use rmk_types::keycode::HidKeyCode;
use rmk_types::morse::{Morse, MorsePattern, MorseProfile};
#[cfg(feature = "rynk")]
use rmk_types::protocol::rynk::{Cmd, RynkError, RynkMessage, command, endpoint::Endpoint};
#[cfg(feature = "vial")]
use rmk_types::protocol::vial::{SettingKey, ViaCommand, VialCommand, VialDynamic};

#[cfg(not(feature = "_no_usb"))]
use crate::channel::USB_REPORT_CHANNEL;
#[cfg(any(feature = "host", not(feature = "_no_usb")))]
use crate::config::RmkConfig;
#[cfg(all(feature = "storage", not(feature = "_no_usb")))]
use crate::config::StorageConfig;
use crate::config::{BehaviorConfig, CombosConfig, Hand, MorsesConfig, PositionalConfig};
#[cfg(not(feature = "_no_usb"))]
use crate::core_traits::Runnable;
#[cfg(not(feature = "_no_usb"))]
use crate::event::AsyncEventPublisher;
use crate::event::{AsyncPublishableEvent, KeyboardEvent};
#[cfg(not(feature = "_no_usb"))]
use crate::hid::{KeyboardReport, Report};
#[cfg(feature = "vial")]
use crate::host::via::keycode_convert::to_via_keycode;
use crate::input_device::rotary_encoder::Direction;
use crate::keyboard::Keyboard;
use crate::keyboard::combo::{Combo, ComboConfig};
use crate::keymap::{KeyMap, KeymapData};
use crate::state::{CONNECTION_STATUS, set_usb_state};
#[cfg(all(feature = "storage", not(feature = "_no_usb")))]
use crate::storage::FLASH_OPERATION_FINISHED;
#[cfg(all(feature = "storage", feature = "host"))]
use crate::storage::FlashOperationMessage;
#[cfg(all(feature = "storage", not(feature = "_no_usb")))]
use crate::storage::Storage;
use crate::types::action::{Action, EncoderAction, KeyAction};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// Reset global test state that is backed by static channels.
///
/// Nextest isolates test cases by process for the mock time driver, but many RMK
/// channels are intentionally static. A simulator scenario should call this at
/// the start so queued events and reports cannot leak across scenarios in the
/// same process.
pub fn reset() {
    KeyboardEvent::publisher_async().clear();

    CONNECTION_STATUS.lock(|c| c.set(ConnectionStatus::default()));
    set_usb_state(UsbState::Configured);

    #[cfg(not(feature = "_no_usb"))]
    USB_REPORT_CHANNEL.clear();
    #[cfg(feature = "_ble")]
    crate::channel::BLE_REPORT_CHANNEL.clear();

    #[cfg(feature = "storage")]
    crate::channel::FLASH_CHANNEL.clear();
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

#[cfg(all(feature = "storage", not(feature = "_no_usb")))]
trait SimStorageRunner {
    fn run_until<'s>(
        &'s mut self,
        done: &'s Signal<CriticalSectionRawMutex, ()>,
    ) -> Pin<Box<dyn Future<Output = ()> + 's>>;
}

#[cfg(all(feature = "storage", not(feature = "_no_usb")))]
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

#[cfg(all(feature = "storage", not(feature = "_no_usb")))]
async fn drain_flash_until_done(done: &Signal<CriticalSectionRawMutex, ()>, enabled: bool) {
    if !enabled {
        done.wait().await;
        return;
    }

    match select(crate::channel::drain_flash_channel_for_test(), done.wait()).await {
        Either::First(_) => unreachable!("flash drain should never return"),
        Either::Second(()) => {}
    }
}

#[cfg(all(feature = "storage", not(feature = "_no_usb")))]
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
pub struct SimMorseSetup {
    patterns: &'static [(u16, Action)],
    vial_morses: &'static [(Action, Action, Action, Action, MorseProfile)],
    profile: Option<MorseProfile>,
    flow_tap: Option<bool>,
    prior_idle_ms: Option<u64>,
}

impl SimMorseSetup {
    pub const fn new() -> Self {
        Self {
            patterns: &[],
            vial_morses: &[],
            profile: None,
            flow_tap: None,
            prior_idle_ms: None,
        }
    }

    pub const fn patterns(mut self, patterns: &'static [(u16, Action)]) -> Self {
        self.patterns = patterns;
        self
    }

    pub const fn vial_morses(mut self, morses: &'static [(Action, Action, Action, Action, MorseProfile)]) -> Self {
        self.vial_morses = morses;
        self
    }

    pub const fn profile(mut self, profile: MorseProfile) -> Self {
        self.profile = Some(profile);
        self
    }

    pub const fn flow_tap(mut self, enable: bool) -> Self {
        self.flow_tap = Some(enable);
        self
    }

    pub const fn prior_idle_ms(mut self, prior_idle_ms: u64) -> Self {
        self.prior_idle_ms = Some(prior_idle_ms);
        self
    }
}

impl Default for SimMorseSetup {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SimKeyboardSetup<const ROW: usize, const COL: usize> {
    key_overrides: &'static [KeymapOverride],
    extra_key_overrides: &'static [KeymapOverride],
    hands: Option<[[Hand; COL]; ROW]>,
    hand_overrides: &'static [HandOverride],
    morse: Option<SimMorseSetup>,
}

impl<const ROW: usize, const COL: usize> SimKeyboardSetup<ROW, COL> {
    pub const fn new() -> Self {
        Self {
            key_overrides: &[],
            extra_key_overrides: &[],
            hands: None,
            hand_overrides: &[],
            morse: None,
        }
    }

    pub const fn keys(mut self, key_overrides: &'static [KeymapOverride]) -> Self {
        self.key_overrides = key_overrides;
        self
    }

    pub const fn extra_keys(mut self, key_overrides: &'static [KeymapOverride]) -> Self {
        self.extra_key_overrides = key_overrides;
        self
    }

    pub const fn hands(mut self, hands: [[Hand; COL]; ROW]) -> Self {
        self.hands = Some(hands);
        self
    }

    pub const fn hand_overrides(mut self, hands: &'static [HandOverride]) -> Self {
        self.hand_overrides = hands;
        self
    }

    pub const fn morse(mut self, morse: SimMorseSetup) -> Self {
        self.morse = Some(morse);
        self
    }

    pub const fn morse_profile(mut self, profile: MorseProfile) -> Self {
        self.morse = Some(match self.morse {
            Some(morse) => morse.profile(profile),
            None => SimMorseSetup::new().profile(profile),
        });
        self
    }

    pub const fn morse_flow_tap(mut self, enable: bool) -> Self {
        self.morse = Some(match self.morse {
            Some(morse) => morse.flow_tap(enable),
            None => SimMorseSetup::new().flow_tap(enable),
        });
        self
    }

    pub const fn morse_prior_idle_ms(mut self, prior_idle_ms: u64) -> Self {
        self.morse = Some(match self.morse {
            Some(morse) => morse.prior_idle_ms(prior_idle_ms),
            None => SimMorseSetup::new().prior_idle_ms(prior_idle_ms),
        });
        self
    }
}

impl<const ROW: usize, const COL: usize> Default for SimKeyboardSetup<ROW, COL> {
    fn default() -> Self {
        Self::new()
    }
}

pub struct NoSimStorage;

#[cfg(all(feature = "storage", not(feature = "_no_usb")))]
pub struct SimStorage<F> {
    flash: F,
    config: StorageConfig,
}

#[cfg(all(feature = "storage", not(feature = "_no_usb")))]
impl<F> SimStorage<F> {
    fn new(flash: F) -> Self {
        Self {
            flash,
            config: StorageConfig::default(),
        }
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
    pub fn behavior(mut self, behavior_config: BehaviorConfig) -> Self {
        self.behavior_config = behavior_config;
        self
    }

    pub fn positional(mut self, positional_config: PositionalConfig<ROW, COL>) -> Self {
        self.positional_config = positional_config;
        self
    }

    pub fn hands(mut self, hand: [[Hand; COL]; ROW]) -> Self {
        self.positional_config = PositionalConfig::new(hand);
        self
    }

    pub fn hand(mut self, row: usize, col: usize, hand: Hand) -> Self {
        self.positional_config.hand[row][col] = hand;
        self
    }

    pub fn key(mut self, layer: usize, row: usize, col: usize, action: KeyAction) -> Self {
        self.keymap[layer][row][col] = action;
        self
    }

    pub fn setup(mut self, setup: SimKeyboardSetup<ROW, COL>) -> Self {
        for key in setup.key_overrides.iter().chain(setup.extra_key_overrides) {
            self.keymap[key.layer][key.row][key.col] = key.action;
        }
        if let Some(hands) = setup.hands {
            self = self.hands(hands);
        }
        for hand in setup.hand_overrides {
            self.positional_config.hand[hand.row][hand.col] = hand.hand;
        }
        if let Some(morse) = setup.morse {
            if let Some(enable) = morse.flow_tap {
                self = self.morse_flow_tap(enable);
            }
            if let Some(prior_idle_ms) = morse.prior_idle_ms {
                self = self.morse_prior_idle_ms(prior_idle_ms);
            }
            if let Some(profile) = morse.profile {
                self = self.morse_default_profile(profile);
            }
            if !morse.patterns.is_empty() {
                self = self.morse_patterns_slice(morse.patterns);
            }
            if !morse.vial_morses.is_empty() {
                self = self.morses_from_vial_slice(morse.vial_morses);
            }
        }
        self
    }

    pub fn morse_config(mut self, morse_config: MorsesConfig) -> Self {
        self.behavior_config.morse = morse_config;
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

    pub fn morse_prior_idle_time(mut self, prior_idle_time: Duration) -> Self {
        self.behavior_config.morse.prior_idle_time = prior_idle_time;
        self
    }

    pub fn morse_prior_idle_ms(self, prior_idle_ms: u64) -> Self {
        self.morse_prior_idle_time(Duration::from_millis(prior_idle_ms))
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

    pub fn morses_from_vial<const NUM_MORSE: usize>(
        mut self,
        morses: [(Action, Action, Action, Action, MorseProfile); NUM_MORSE],
    ) -> Self {
        for (tap, hold, hold_after_tap, double_tap, profile) in morses {
            self = self.morse_from_vial(tap, hold, hold_after_tap, double_tap, profile);
        }
        self
    }

    fn morses_from_vial_slice(mut self, morses: &[(Action, Action, Action, Action, MorseProfile)]) -> Self {
        for &(tap, hold, hold_after_tap, double_tap, profile) in morses {
            self = self.morse_from_vial(tap, hold, hold_after_tap, double_tap, profile);
        }
        self
    }

    pub fn morse_patterns<const NUM_PATTERN: usize>(self, patterns: [(u16, Action); NUM_PATTERN]) -> Self {
        self.morse(Morse {
            actions: heapless::LinearMap::from_iter(
                patterns
                    .into_iter()
                    .map(|(pattern, action)| (MorsePattern::from_u16(pattern), action)),
            ),
            ..Default::default()
        })
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

    pub fn morse_patterns_with_profile<const NUM_PATTERN: usize>(
        self,
        profile: MorseProfile,
        patterns: [(u16, Action); NUM_PATTERN],
    ) -> Self {
        self.morse_default_profile(profile).morse_patterns(patterns)
    }

    pub fn combo<const NUM_ACTION: usize>(
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

    pub fn combo_timeout(mut self, timeout: Duration) -> Self {
        self.behavior_config.combo.timeout = timeout;
        self
    }

    pub fn combo_timeout_ms(self, timeout_ms: u64) -> Self {
        self.combo_timeout(Duration::from_millis(timeout_ms))
    }

    pub fn combo_prior_idle_time(mut self, prior_idle_time: Option<Duration>) -> Self {
        self.behavior_config.combo.prior_idle_time = prior_idle_time;
        self
    }

    pub fn combo_config(mut self, combo_config: CombosConfig) -> Self {
        self.behavior_config.combo = combo_config;
        self
    }

    pub fn one_shot_timeout(mut self, timeout: Duration) -> Self {
        self.behavior_config.one_shot.timeout = timeout;
        self
    }

    pub fn one_shot_timeout_ms(self, timeout_ms: u64) -> Self {
        self.one_shot_timeout(Duration::from_millis(timeout_ms))
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

    #[cfg(all(feature = "storage", not(feature = "_no_usb")))]
    pub fn storage(
        self,
    ) -> SimKeyboardBuilder<ROW, COL, NUM_LAYER, NUM_ENCODER, SimStorage<flash::InMemoryFlash<8192>>> {
        self.storage_flash(flash::InMemoryFlash::new())
    }

    #[cfg(all(feature = "storage", not(feature = "_no_usb")))]
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

#[cfg(all(feature = "storage", not(feature = "_no_usb")))]
impl<F, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    SimKeyboardBuilder<ROW, COL, NUM_LAYER, NUM_ENCODER, SimStorage<F>>
where
    F: AsyncNorFlash + 'static,
{
    pub fn storage_config(mut self, config: StorageConfig) -> Self {
        self.storage.config = config;
        self
    }

    pub async fn build(self) -> SimKeyboard<'static> {
        let (keymap, storage) = build_static_keymap_with_storage(
            self.keymap,
            self.encoder_map,
            self.storage.flash,
            self.storage.config,
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
    #[cfg(not(feature = "_no_usb"))]
    ExpectKeyboardState {
        modifier: u8,
        keycodes: Vec<u8>,
    },
    #[cfg(not(feature = "_no_usb"))]
    ExpectReport(Report),
    #[cfg(not(feature = "_no_usb"))]
    ExpectNoReport(Duration),
    #[cfg(feature = "vial")]
    VialPacket {
        transport: ConnectionType,
        data: [u8; 32],
        timeout: Duration,
        expected: VialReplyExpectation,
    },
    #[cfg(feature = "rynk")]
    RynkPacket {
        transport: ConnectionType,
        timeout: Duration,
        request: Vec<u8>,
        expected: Vec<u8>,
    },
    #[cfg(all(feature = "storage", feature = "host"))]
    ExpectFlashOperation(FlashExpectation),
    #[cfg(all(feature = "storage", not(feature = "_no_usb")))]
    WaitStorage,
    #[cfg(feature = "passkey_entry")]
    BeginPasskeyEntry,
    #[cfg(feature = "passkey_entry")]
    ExpectPasskeyResponse(Option<u32>),
    #[cfg(feature = "passkey_entry")]
    EndPasskeyEntry,
}

#[cfg(feature = "vial")]
#[derive(Debug)]
enum VialReplyExpectation {
    Exact([u8; 32]),
    Command(u8),
}

#[cfg(all(feature = "storage", feature = "host"))]
#[derive(Debug)]
enum FlashExpectation {
    KeymapKey {
        layer: u8,
        row: u8,
        col: u8,
        action: KeyAction,
    },
    Encoder {
        layer: u8,
        idx: u8,
        action: EncoderAction,
    },
    ComboTimeout(u16),
    OneShotTimeout(u16),
    TapInterval(u16),
    TapCapslockInterval(u16),
    PriorIdleTime(u16),
    MorseDefaultProfile(MorseProfile),
}

/// Simulator for a complete keyboard device, excluding physical input and
/// transport I/O.
pub struct SimKeyboard<'a> {
    keyboard: Keyboard<'a>,
    timeout: Duration,
    steps: Vec<SimStep>,
    #[cfg(all(feature = "storage", not(feature = "_no_usb")))]
    storage: Option<Box<dyn SimStorageRunner>>,
    #[cfg(feature = "host")]
    host_config: Option<RmkConfig<'static>>,
}

impl<'a> SimKeyboard<'a> {
    fn new(keyboard: Keyboard<'a>) -> Self {
        Self {
            keyboard,
            timeout: DEFAULT_TIMEOUT,
            steps: Vec::new(),
            #[cfg(all(feature = "storage", not(feature = "_no_usb")))]
            storage: None,
            #[cfg(feature = "host")]
            host_config: None,
        }
    }

    #[cfg(all(feature = "storage", not(feature = "_no_usb")))]
    fn with_storage(mut self, storage: Box<dyn SimStorageRunner>) -> Self {
        self.storage = Some(storage);
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
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
        self.delay_duration(Duration::from_millis(ms))
    }

    pub fn delay_duration(&mut self, duration: Duration) -> &mut Self {
        self.steps.push(SimStep::Delay(duration));
        self
    }

    pub fn rotary_cw(&mut self, id: u8) -> &mut Self {
        self.rotary(id, Direction::Clockwise)
    }

    pub fn rotary_ccw(&mut self, id: u8) -> &mut Self {
        self.rotary(id, Direction::CounterClockwise)
    }

    pub fn rotary(&mut self, id: u8, direction: Direction) -> &mut Self {
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

    #[cfg(not(feature = "_no_usb"))]
    pub fn expect_keys<const N: usize>(&mut self, keycodes: [HidKeyCode; N]) -> &mut Self {
        self.expect_keys_with_mods(0, keycodes)
    }

    #[cfg(not(feature = "_no_usb"))]
    pub fn expect_keys_with_mods<const N: usize>(&mut self, modifier: u8, keycodes: [HidKeyCode; N]) -> &mut Self {
        let keycodes = keycodes.iter().map(|keycode| *keycode as u8).collect();
        self.expect_keyboard_state(modifier, keycodes)
    }

    #[cfg(not(feature = "_no_usb"))]
    fn expect_keyboard_state(&mut self, modifier: u8, keycodes: Vec<u8>) -> &mut Self {
        assert!(
            keycodes.len() <= KeyboardReport::default().keycodes.len(),
            "keyboard HID reports can carry at most {} simultaneous keycodes",
            KeyboardReport::default().keycodes.len()
        );
        self.steps.push(SimStep::ExpectKeyboardState { modifier, keycodes });
        self
    }

    #[cfg(not(feature = "_no_usb"))]
    pub fn expect_only_mods(&mut self, modifier: u8) -> &mut Self {
        self.expect_keyboard_state(modifier, Vec::new())
    }

    #[cfg(not(feature = "_no_usb"))]
    pub fn expect_all_up(&mut self) -> &mut Self {
        self.expect_keyboard_state(0, Vec::new())
    }

    #[cfg(not(feature = "_no_usb"))]
    pub fn expect_keyboard_report(&mut self, report: KeyboardReport) -> &mut Self {
        self.expect_report(Report::KeyboardReport(report))
    }

    #[cfg(not(feature = "_no_usb"))]
    pub fn expect_report(&mut self, report: Report) -> &mut Self {
        self.steps.push(SimStep::ExpectReport(report));
        self
    }

    #[cfg(not(feature = "_no_usb"))]
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
    fn vial_packet(
        &mut self,
        transport: ConnectionType,
        data: [u8; 32],
        timeout: Duration,
        expected: VialReplyExpectation,
    ) -> &mut Self {
        self.steps.push(SimStep::VialPacket {
            transport,
            data,
            timeout,
            expected,
        });
        self
    }

    #[cfg(all(feature = "storage", feature = "host"))]
    pub fn expect_flash_key(&mut self, layer: u8, row: u8, col: u8, action: KeyAction) -> &mut Self {
        self.steps
            .push(SimStep::ExpectFlashOperation(FlashExpectation::KeymapKey {
                layer,
                row,
                col,
                action,
            }));
        self
    }

    #[cfg(all(feature = "storage", feature = "host"))]
    pub fn expect_flash_encoder(&mut self, layer: u8, idx: u8, action: EncoderAction) -> &mut Self {
        self.steps
            .push(SimStep::ExpectFlashOperation(FlashExpectation::Encoder {
                layer,
                idx,
                action,
            }));
        self
    }

    #[cfg(all(feature = "storage", feature = "host"))]
    pub fn expect_flash_combo_timeout(&mut self, value: u16) -> &mut Self {
        self.steps
            .push(SimStep::ExpectFlashOperation(FlashExpectation::ComboTimeout(value)));
        self
    }

    #[cfg(all(feature = "storage", feature = "host"))]
    pub fn expect_flash_one_shot_timeout(&mut self, value: u16) -> &mut Self {
        self.steps
            .push(SimStep::ExpectFlashOperation(FlashExpectation::OneShotTimeout(value)));
        self
    }

    #[cfg(all(feature = "storage", feature = "host"))]
    pub fn expect_flash_tap_interval(&mut self, value: u16) -> &mut Self {
        self.steps
            .push(SimStep::ExpectFlashOperation(FlashExpectation::TapInterval(value)));
        self
    }

    #[cfg(all(feature = "storage", feature = "host"))]
    pub fn expect_flash_tap_capslock_interval(&mut self, value: u16) -> &mut Self {
        self.steps
            .push(SimStep::ExpectFlashOperation(FlashExpectation::TapCapslockInterval(
                value,
            )));
        self
    }

    #[cfg(all(feature = "storage", feature = "host"))]
    pub fn expect_flash_prior_idle_time(&mut self, value: u16) -> &mut Self {
        self.steps
            .push(SimStep::ExpectFlashOperation(FlashExpectation::PriorIdleTime(value)));
        self
    }

    #[cfg(all(feature = "storage", feature = "host"))]
    pub fn expect_flash_morse_default_profile(&mut self, profile: MorseProfile) -> &mut Self {
        self.steps
            .push(SimStep::ExpectFlashOperation(FlashExpectation::MorseDefaultProfile(
                profile,
            )));
        self
    }

    #[cfg(all(feature = "storage", not(feature = "_no_usb")))]
    pub fn wait_storage(&mut self) -> &mut Self {
        self.steps.push(SimStep::WaitStorage);
        self
    }

    #[cfg(feature = "rynk")]
    fn rynk_packet(
        &mut self,
        transport: ConnectionType,
        timeout: Duration,
        request: Vec<u8>,
        expected: Vec<u8>,
    ) -> &mut Self {
        self.steps.push(SimStep::RynkPacket {
            transport,
            timeout,
            request,
            expected,
        });
        self
    }

    #[cfg(not(feature = "_no_usb"))]
    pub async fn run(&mut self) {
        let keyboard_done = Signal::<CriticalSectionRawMutex, ()>::new();
        #[cfg(all(feature = "storage", not(feature = "_no_usb")))]
        let storage_done = Signal::<CriticalSectionRawMutex, ()>::new();
        #[cfg(all(feature = "storage", not(feature = "_no_usb")))]
        let flash_drain_done = Signal::<CriticalSectionRawMutex, ()>::new();
        let steps = core::mem::take(&mut self.steps);
        let timeout = self.timeout;
        #[cfg(all(feature = "storage", feature = "host"))]
        let drain_flash = !steps
            .iter()
            .any(|step| matches!(step, SimStep::ExpectFlashOperation(_)));
        #[cfg(all(feature = "storage", not(feature = "host")))]
        let drain_flash = true;
        #[cfg(feature = "host")]
        let protocol_config = self.host_config.as_ref();
        #[cfg(not(feature = "host"))]
        let protocol_config: Option<&RmkConfig<'static>> = None;

        reset();
        let keymap = self.keyboard.keymap;

        #[cfg(all(feature = "storage", not(feature = "_no_usb")))]
        if let Some(storage) = self.storage.as_deref_mut() {
            join!(
                Self::run_keyboard_until_done(&mut self.keyboard, &keyboard_done),
                storage.run_until(&storage_done),
                drain_flash_until_done(&flash_drain_done, false),
                async {
                    Self::run_steps(keymap, steps, timeout, protocol_config).await;
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
                Self::run_steps(keymap, steps, timeout, protocol_config).await;
                keyboard_done.signal(());
                flash_drain_done.signal(());
            },
            drain_flash_until_done(&flash_drain_done, drain_flash)
        );
        #[cfg(not(feature = "storage"))]
        join!(
            Self::run_keyboard_until_done(&mut self.keyboard, &keyboard_done),
            async {
                Self::run_steps(keymap, steps, timeout, protocol_config).await;
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
    }

    #[cfg(not(feature = "_no_usb"))]
    async fn run_keyboard_until_done(keyboard: &mut Keyboard<'_>, done: &Signal<CriticalSectionRawMutex, ()>) {
        match select(keyboard.run(), done.wait()).await {
            Either::First(_) => unreachable!("keyboard task should never return"),
            Either::Second(()) => {}
        }
    }

    #[cfg(not(feature = "_no_usb"))]
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

        for (idx, step) in steps.into_iter().enumerate() {
            match step {
                SimStep::Event(event) => match select(Timer::after(timeout), sender.publish_async(event)).await {
                    Either::First(_) => panic!("simulator timed out publishing keyboard event at step #{idx}"),
                    Either::Second(()) => {}
                },
                SimStep::Delay(duration) => {
                    Timer::after(duration).await;
                }
                SimStep::ExpectKeyboardState { modifier, keycodes } => {
                    let actual = match select(Timer::after(timeout), USB_REPORT_CHANNEL.receive()).await {
                        Either::First(_) => panic!("simulator timed out waiting for keyboard report at step #{idx}"),
                        Either::Second(report) => report,
                    };
                    Self::assert_keyboard_state_eq(modifier, keycodes, actual, idx);
                }
                SimStep::ExpectReport(expected) => {
                    let actual = match select(Timer::after(timeout), USB_REPORT_CHANNEL.receive()).await {
                        Either::First(_) => panic!("simulator timed out waiting for HID report at step #{idx}"),
                        Either::Second(report) => report,
                    };
                    Self::assert_report_eq(expected, actual, idx);
                }
                SimStep::ExpectNoReport(duration) => {
                    match select(Timer::after(duration), USB_REPORT_CHANNEL.receive()).await {
                        Either::First(_) => {}
                        Either::Second(report) => {
                            panic!("unexpected HID report at step #{idx}: {:?}", report);
                        }
                    }
                }
                #[cfg(feature = "vial")]
                SimStep::VialPacket {
                    transport,
                    data,
                    timeout,
                    expected,
                } => {
                    #[cfg(not(feature = "_ble"))]
                    assert_ne!(
                        transport,
                        ConnectionType::Ble,
                        "BLE host transactions require the `_ble` feature"
                    );
                    #[cfg(feature = "_ble")]
                    let _ = transport;
                    #[cfg(feature = "storage")]
                    FLASH_OPERATION_FINISHED.reset();
                    let service = host_service
                        .as_ref()
                        .expect("simulator Vial config must be enabled before running Vial steps");
                    let actual = match select(Timer::after(timeout), service.process_packet(data)).await {
                        Either::First(_) => panic!("simulator timed out dispatching Vial packet at step #{idx}"),
                        Either::Second(reply) => reply,
                    };
                    match expected {
                        VialReplyExpectation::Exact(expected) => {
                            assert_eq!(expected, actual, "on Vial reply at step #{idx}");
                        }
                        VialReplyExpectation::Command(command) => {
                            assert_eq!(command, actual[0], "on Vial reply command at step #{idx}");
                        }
                    }
                }
                #[cfg(feature = "rynk")]
                SimStep::RynkPacket {
                    transport,
                    timeout,
                    mut request,
                    expected,
                } => {
                    #[cfg(not(feature = "_ble"))]
                    assert_ne!(
                        transport,
                        ConnectionType::Ble,
                        "BLE host transactions require the `_ble` feature"
                    );
                    #[cfg(feature = "_ble")]
                    let _ = transport;
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
                #[cfg(all(feature = "storage", feature = "host"))]
                SimStep::ExpectFlashOperation(expected) => {
                    let actual = match select(Timer::after(timeout), crate::channel::FLASH_CHANNEL.receive()).await {
                        Either::First(_) => panic!("simulator timed out waiting for flash operation at step #{idx}"),
                        Either::Second(operation) => operation,
                    };
                    Self::assert_flash_operation_eq(expected, actual, idx);
                }
                #[cfg(all(feature = "storage", not(feature = "_no_usb")))]
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

        // Give the keyboard task a chance to consume a final event even when
        // the timeline has no following expectation, then reject output that
        // the scenario did not explicitly assert.
        Timer::after(Duration::from_millis(1)).await;
        if let Ok(report) = USB_REPORT_CHANNEL.try_receive() {
            panic!(
                "unexpected trailing HID report after final simulator step: {:?}",
                report
            );
        }
    }

    #[cfg(all(feature = "storage", feature = "host"))]
    fn assert_flash_operation_eq(expected: FlashExpectation, actual: FlashOperationMessage, step_idx: usize) {
        match (expected, actual) {
            (
                FlashExpectation::KeymapKey {
                    layer,
                    row,
                    col,
                    action,
                },
                FlashOperationMessage::KeymapKey {
                    layer: actual_layer,
                    row: actual_row,
                    col: actual_col,
                    action: actual_action,
                },
            ) => {
                assert_eq!(
                    (layer, row, col, action),
                    (actual_layer, actual_row, actual_col, actual_action),
                    "on flash operation at step #{step_idx}"
                );
            }
            (
                FlashExpectation::Encoder { layer, idx, action },
                FlashOperationMessage::Encoder {
                    layer: actual_layer,
                    idx: actual_idx,
                    action: actual_action,
                },
            ) => {
                assert_eq!(
                    (layer, idx, action),
                    (actual_layer, actual_idx, actual_action),
                    "on flash operation at step #{step_idx}"
                );
            }
            (FlashExpectation::ComboTimeout(value), FlashOperationMessage::ComboTimeout(actual)) => {
                assert_eq!(value, actual, "on flash operation at step #{step_idx}");
            }
            (FlashExpectation::OneShotTimeout(value), FlashOperationMessage::OneShotTimeout(actual)) => {
                assert_eq!(value, actual, "on flash operation at step #{step_idx}");
            }
            (FlashExpectation::TapInterval(value), FlashOperationMessage::TapInterval(actual)) => {
                assert_eq!(value, actual, "on flash operation at step #{step_idx}");
            }
            (FlashExpectation::TapCapslockInterval(value), FlashOperationMessage::TapCapslockInterval(actual)) => {
                assert_eq!(value, actual, "on flash operation at step #{step_idx}");
            }
            (FlashExpectation::PriorIdleTime(value), FlashOperationMessage::PriorIdleTime(actual)) => {
                assert_eq!(value, actual, "on flash operation at step #{step_idx}");
            }
            (FlashExpectation::MorseDefaultProfile(value), FlashOperationMessage::MorseDefaultProfile(actual)) => {
                assert_eq!(value, actual, "on flash operation at step #{step_idx}");
            }
            (expected, actual) => {
                panic!(
                    "unexpected flash operation at step #{step_idx}: expected {:?}, actual {:?}",
                    expected, actual
                );
            }
        }
    }

    #[cfg(not(feature = "_no_usb"))]
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

    #[cfg(not(feature = "_no_usb"))]
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
    pub fn single_key(action: KeyAction) -> SimKeyboardBuilder<1, 1, 1, 0> {
        Self::builder([[[action]]])
    }

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
pub struct SimHost {
    transport: ConnectionType,
    timeout: Duration,
}

#[cfg(feature = "host")]
impl Default for SimHost {
    fn default() -> Self {
        Self {
            transport: ConnectionType::Usb,
            timeout: DEFAULT_TIMEOUT,
        }
    }
}

#[cfg(feature = "host")]
impl SimHost {
    pub fn usb() -> Self {
        Self::default()
    }

    pub fn ble() -> Self {
        Self::default().with_transport(ConnectionType::Ble)
    }

    pub fn with_transport(mut self, transport: ConnectionType) -> Self {
        self.transport = transport;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    #[cfg(feature = "vial")]
    pub fn send_packet_to<'k, 'a>(&self, keyboard: &'k mut SimKeyboard<'a>, data: [u8; 32]) -> SimHostReply<'k, 'a> {
        keyboard.enable_host();
        SimHostReply {
            keyboard,
            transport: self.transport,
            timeout: self.timeout,
            data,
        }
    }

    #[cfg(feature = "vial")]
    pub fn vial<'k, 'a>(&self, keyboard: &'k mut SimKeyboard<'a>) -> SimVial<'k, 'a> {
        keyboard.enable_host();
        SimVial {
            keyboard,
            transport: self.transport,
            timeout: self.timeout,
        }
    }

    #[cfg(feature = "rynk")]
    pub fn rynk<'k, 'a>(&self, keyboard: &'k mut SimKeyboard<'a>) -> SimRynk<'k, 'a> {
        keyboard.enable_host();
        SimRynk {
            keyboard,
            transport: self.transport,
            timeout: self.timeout,
            seq: 0,
        }
    }
}

#[cfg(feature = "vial")]
#[must_use = "host transactions must end with an expectation"]
pub struct SimHostReply<'k, 'a> {
    keyboard: &'k mut SimKeyboard<'a>,
    transport: ConnectionType,
    timeout: Duration,
    data: [u8; 32],
}

#[cfg(feature = "vial")]
impl<'k, 'a> SimHostReply<'k, 'a> {
    pub fn expect_ok(self) -> &'k mut SimKeyboard<'a> {
        self.keyboard.vial_packet(
            self.transport,
            self.data,
            self.timeout,
            VialReplyExpectation::Command(self.data[0]),
        );
        self.keyboard
    }

    pub fn expect_echo(self) -> &'k mut SimKeyboard<'a> {
        self.keyboard.vial_packet(
            self.transport,
            self.data,
            self.timeout,
            VialReplyExpectation::Exact(self.data),
        );
        self.keyboard
    }

    pub fn expect_command(self, command: u8) -> &'k mut SimKeyboard<'a> {
        self.keyboard.vial_packet(
            self.transport,
            self.data,
            self.timeout,
            VialReplyExpectation::Command(command),
        );
        self.keyboard
    }

    pub fn expect(self, reply: [u8; 32]) -> &'k mut SimKeyboard<'a> {
        self.keyboard.vial_packet(
            self.transport,
            self.data,
            self.timeout,
            VialReplyExpectation::Exact(reply),
        );
        self.keyboard
    }
}

#[cfg(feature = "vial")]
pub struct SimVial<'k, 'a> {
    keyboard: &'k mut SimKeyboard<'a>,
    transport: ConnectionType,
    timeout: Duration,
}

#[cfg(feature = "vial")]
impl<'k, 'a> SimVial<'k, 'a> {
    pub fn raw(self, data: [u8; 32]) -> SimHostReply<'k, 'a> {
        SimHostReply {
            keyboard: self.keyboard,
            transport: self.transport,
            timeout: self.timeout,
            data,
        }
    }

    pub fn get_protocol_version(self) -> SimHostReply<'k, 'a> {
        let mut data = [0u8; 32];
        data[0] = ViaCommand::GetProtocolVersion as u8;
        self.raw(data)
    }

    pub fn get_key(self, layer: u8, row: u8, col: u8) -> SimVialKeyReply<'k, 'a> {
        let mut data = [0u8; 32];
        data[0] = ViaCommand::DynamicKeymapGetKeyCode as u8;
        data[1] = layer;
        data[2] = row;
        data[3] = col;
        SimVialKeyReply { reply: self.raw(data) }
    }

    pub fn set_key(self, layer: u8, row: u8, col: u8, action: KeyAction) -> SimHostReply<'k, 'a> {
        let mut data = [0u8; 32];
        data[0] = ViaCommand::DynamicKeymapSetKeyCode as u8;
        data[1] = layer;
        data[2] = row;
        data[3] = col;
        data[4..6].copy_from_slice(&to_via_keycode(action).to_be_bytes());
        self.raw(data)
    }

    pub fn get_encoder(self, layer: u8, encoder_id: u8) -> SimVialEncoderReply<'k, 'a> {
        let mut data = [0u8; 32];
        data[0] = ViaCommand::Vial as u8;
        data[1] = VialCommand::GetEncoder as u8;
        data[2] = layer;
        data[3] = encoder_id;
        SimVialEncoderReply { reply: self.raw(data) }
    }

    pub fn set_encoder(self, layer: u8, encoder_id: u8, action: EncoderAction) -> SimVialSetEncoderReply<'k, 'a> {
        let mut clockwise = [0u8; 32];
        clockwise[0] = ViaCommand::Vial as u8;
        clockwise[1] = VialCommand::SetEncoder as u8;
        clockwise[2] = layer;
        clockwise[3] = encoder_id;
        clockwise[4] = 1;
        clockwise[5..7].copy_from_slice(&to_via_keycode(action.clockwise).to_be_bytes());

        let mut counter_clockwise = [0u8; 32];
        counter_clockwise[0] = ViaCommand::Vial as u8;
        counter_clockwise[1] = VialCommand::SetEncoder as u8;
        counter_clockwise[2] = layer;
        counter_clockwise[3] = encoder_id;
        counter_clockwise[4] = 0;
        counter_clockwise[5..7].copy_from_slice(&to_via_keycode(action.counter_clockwise).to_be_bytes());

        SimVialSetEncoderReply {
            keyboard: self.keyboard,
            transport: self.transport,
            timeout: self.timeout,
            clockwise,
            counter_clockwise,
        }
    }

    pub fn get_behavior_setting(self, setting: SettingKey) -> SimVialBehaviorSettingReply<'k, 'a> {
        let mut data = [0u8; 32];
        data[0] = ViaCommand::Vial as u8;
        data[1] = VialCommand::GetBehaviorSetting as u8;
        data[2..4].copy_from_slice(&(setting as u16).to_le_bytes());
        SimVialBehaviorSettingReply { reply: self.raw(data) }
    }

    pub fn set_behavior_setting_u16(self, setting: SettingKey, value: u16) -> SimHostReply<'k, 'a> {
        let mut data = [0u8; 32];
        data[0] = ViaCommand::Vial as u8;
        data[1] = VialCommand::SetBehaviorSetting as u8;
        data[2..4].copy_from_slice(&(setting as u16).to_le_bytes());
        data[4..6].copy_from_slice(&value.to_le_bytes());
        self.raw(data)
    }

    pub fn set_behavior_setting_bool(self, setting: SettingKey, value: bool) -> SimHostReply<'k, 'a> {
        let mut data = [0u8; 32];
        data[0] = ViaCommand::Vial as u8;
        data[1] = VialCommand::SetBehaviorSetting as u8;
        data[2..4].copy_from_slice(&(setting as u16).to_le_bytes());
        data[4] = value as u8;
        self.raw(data)
    }

    pub fn get_morse(self, index: u8) -> SimVialMorseReply<'k, 'a> {
        let mut data = [0u8; 32];
        data[0] = ViaCommand::Vial as u8;
        data[1] = VialCommand::DynamicEntryOp as u8;
        data[2] = VialDynamic::DynamicVialMorseGet as u8;
        data[3] = index;
        SimVialMorseReply { reply: self.raw(data) }
    }

    pub fn set_morse(
        self,
        index: u8,
        tap: KeyAction,
        hold: KeyAction,
        double_tap: KeyAction,
        hold_after_tap: KeyAction,
        timeout_ms: u16,
    ) -> SimVialDynamicSetReply<'k, 'a> {
        let mut data = [0u8; 32];
        data[0] = ViaCommand::Vial as u8;
        data[1] = VialCommand::DynamicEntryOp as u8;
        data[2] = VialDynamic::DynamicVialMorseSet as u8;
        data[3] = index;
        data[4..6].copy_from_slice(&to_via_keycode(tap).to_le_bytes());
        data[6..8].copy_from_slice(&to_via_keycode(hold).to_le_bytes());
        data[8..10].copy_from_slice(&to_via_keycode(double_tap).to_le_bytes());
        data[10..12].copy_from_slice(&to_via_keycode(hold_after_tap).to_le_bytes());
        data[12..14].copy_from_slice(&timeout_ms.to_le_bytes());
        SimVialDynamicSetReply { reply: self.raw(data) }
    }

    pub fn get_combo(self, index: u8) -> SimVialComboReply<'k, 'a> {
        let mut data = [0u8; 32];
        data[0] = ViaCommand::Vial as u8;
        data[1] = VialCommand::DynamicEntryOp as u8;
        data[2] = VialDynamic::DynamicVialComboGet as u8;
        data[3] = index;
        SimVialComboReply { reply: self.raw(data) }
    }

    pub fn set_combo<const N: usize>(
        self,
        index: u8,
        actions: [KeyAction; N],
        output: KeyAction,
    ) -> SimVialDynamicSetReply<'k, 'a> {
        assert!(
            N <= crate::COMBO_MAX_LENGTH,
            "simulator combo helper received too many actions"
        );

        let mut data = [0u8; 32];
        data[0] = ViaCommand::Vial as u8;
        data[1] = VialCommand::DynamicEntryOp as u8;
        data[2] = VialDynamic::DynamicVialComboSet as u8;
        data[3] = index;
        for (idx, action) in actions.into_iter().enumerate() {
            let start = 4 + idx * 2;
            data[start..start + 2].copy_from_slice(&to_via_keycode(action).to_le_bytes());
        }
        let output_start = 4 + crate::COMBO_MAX_LENGTH * 2;
        data[output_start..output_start + 2].copy_from_slice(&to_via_keycode(output).to_le_bytes());

        SimVialDynamicSetReply { reply: self.raw(data) }
    }

    pub fn unsupported_dynamic_entry(self) -> SimHostReply<'k, 'a> {
        let mut data = [0u8; 32];
        data[0] = ViaCommand::Vial as u8;
        data[1] = VialCommand::DynamicEntryOp as u8;
        data[2] = VialDynamic::Unhandled as u8;
        self.raw(data)
    }
}

#[cfg(feature = "vial")]
#[must_use = "Vial requests must end with an expectation"]
pub struct SimVialKeyReply<'k, 'a> {
    reply: SimHostReply<'k, 'a>,
}

#[cfg(feature = "vial")]
impl<'k, 'a> SimVialKeyReply<'k, 'a> {
    pub fn expect(self, action: KeyAction) -> &'k mut SimKeyboard<'a> {
        let mut expected = self.reply.data;
        expected[4..6].copy_from_slice(&to_via_keycode(action).to_be_bytes());
        self.reply.expect(expected)
    }
}

#[cfg(feature = "vial")]
#[must_use = "Vial requests must end with an expectation"]
pub struct SimVialEncoderReply<'k, 'a> {
    reply: SimHostReply<'k, 'a>,
}

#[cfg(feature = "vial")]
impl<'k, 'a> SimVialEncoderReply<'k, 'a> {
    pub fn expect(self, action: EncoderAction) -> &'k mut SimKeyboard<'a> {
        let mut expected = [0u8; 32];
        expected[0..2].copy_from_slice(&to_via_keycode(action.counter_clockwise).to_be_bytes());
        expected[2..4].copy_from_slice(&to_via_keycode(action.clockwise).to_be_bytes());
        self.reply.expect(expected)
    }
}

#[cfg(feature = "vial")]
#[must_use = "Vial requests must end with an expectation"]
pub struct SimVialSetEncoderReply<'k, 'a> {
    keyboard: &'k mut SimKeyboard<'a>,
    transport: ConnectionType,
    timeout: Duration,
    clockwise: [u8; 32],
    counter_clockwise: [u8; 32],
}

#[cfg(feature = "vial")]
impl<'k, 'a> SimVialSetEncoderReply<'k, 'a> {
    pub fn expect_ok(self) -> &'k mut SimKeyboard<'a> {
        self.keyboard.vial_packet(
            self.transport,
            self.clockwise,
            self.timeout,
            VialReplyExpectation::Exact(self.clockwise),
        );
        self.keyboard.vial_packet(
            self.transport,
            self.counter_clockwise,
            self.timeout,
            VialReplyExpectation::Exact(self.counter_clockwise),
        );
        self.keyboard
    }
}

#[cfg(feature = "vial")]
#[must_use = "Vial requests must end with an expectation"]
pub struct SimVialBehaviorSettingReply<'k, 'a> {
    reply: SimHostReply<'k, 'a>,
}

#[cfg(feature = "vial")]
impl<'k, 'a> SimVialBehaviorSettingReply<'k, 'a> {
    pub fn expect_u16(self, value: u16) -> &'k mut SimKeyboard<'a> {
        let mut expected = [0xFF; 32];
        expected[0] = 0;
        expected[1..3].copy_from_slice(&value.to_le_bytes());
        self.reply.expect(expected)
    }

    pub fn expect_bool(self, value: bool) -> &'k mut SimKeyboard<'a> {
        let mut expected = [0xFF; 32];
        expected[0] = 0;
        expected[1] = value as u8;
        self.reply.expect(expected)
    }
}

#[cfg(feature = "vial")]
#[must_use = "Vial requests must end with an expectation"]
pub struct SimVialMorseReply<'k, 'a> {
    reply: SimHostReply<'k, 'a>,
}

#[cfg(feature = "vial")]
impl<'k, 'a> SimVialMorseReply<'k, 'a> {
    pub fn expect(
        self,
        tap: KeyAction,
        hold: KeyAction,
        double_tap: KeyAction,
        hold_after_tap: KeyAction,
        timeout_ms: u16,
    ) -> &'k mut SimKeyboard<'a> {
        let mut expected = self.reply.data;
        expected[0] = 0;
        expected[1..3].copy_from_slice(&to_via_keycode(tap).to_le_bytes());
        expected[3..5].copy_from_slice(&to_via_keycode(hold).to_le_bytes());
        expected[5..7].copy_from_slice(&to_via_keycode(double_tap).to_le_bytes());
        expected[7..9].copy_from_slice(&to_via_keycode(hold_after_tap).to_le_bytes());
        expected[9..11].copy_from_slice(&timeout_ms.to_le_bytes());
        self.reply.expect(expected)
    }
}

#[cfg(feature = "vial")]
#[must_use = "Vial requests must end with an expectation"]
pub struct SimVialComboReply<'k, 'a> {
    reply: SimHostReply<'k, 'a>,
}

#[cfg(feature = "vial")]
impl<'k, 'a> SimVialComboReply<'k, 'a> {
    pub fn expect<const N: usize>(self, actions: [KeyAction; N], output: KeyAction) -> &'k mut SimKeyboard<'a> {
        assert!(
            N <= crate::COMBO_MAX_LENGTH,
            "simulator combo helper received too many actions"
        );

        let mut expected = self.reply.data;
        expected[0] = 0;
        for (idx, action) in actions.into_iter().enumerate() {
            let start = 1 + idx * 2;
            expected[start..start + 2].copy_from_slice(&to_via_keycode(action).to_le_bytes());
        }
        let output_start = 1 + crate::COMBO_MAX_LENGTH * 2;
        expected[output_start..output_start + 2].copy_from_slice(&to_via_keycode(output).to_le_bytes());
        self.reply.expect(expected)
    }
}

#[cfg(feature = "vial")]
#[must_use = "Vial requests must end with an expectation"]
pub struct SimVialDynamicSetReply<'k, 'a> {
    reply: SimHostReply<'k, 'a>,
}

#[cfg(feature = "vial")]
impl<'k, 'a> SimVialDynamicSetReply<'k, 'a> {
    pub fn expect_ok(self) -> &'k mut SimKeyboard<'a> {
        let mut expected = self.reply.data;
        expected[0] = 0;
        self.reply.expect(expected)
    }
}

#[cfg(feature = "rynk")]
pub struct SimRynk<'k, 'a> {
    keyboard: &'k mut SimKeyboard<'a>,
    transport: ConnectionType,
    timeout: Duration,
    seq: u8,
}

#[cfg(feature = "rynk")]
impl<'k, 'a> SimRynk<'k, 'a> {
    pub fn with_seq(mut self, seq: u8) -> Self {
        self.seq = seq;
        self
    }

    pub fn request<E: Endpoint>(self, payload: E::Request) -> SimRynkReply<'k, 'a, E> {
        SimRynkReply {
            keyboard: self.keyboard,
            transport: self.transport,
            timeout: self.timeout,
            seq: self.seq,
            request: rynk_request_frame(E::CMD, self.seq, &payload),
            endpoint: PhantomData,
        }
    }

    pub fn get_version(self) -> SimRynkReply<'k, 'a, command::GetVersion> {
        self.request::<command::GetVersion>(())
    }

    pub fn get_key(self, layer: u8, row: u8, col: u8) -> SimRynkReply<'k, 'a, command::GetKeyAction> {
        self.request::<command::GetKeyAction>(rmk_types::protocol::rynk::KeyPosition { layer, row, col })
    }

    pub fn set_key(
        self,
        layer: u8,
        row: u8,
        col: u8,
        action: KeyAction,
    ) -> SimRynkReply<'k, 'a, command::SetKeyAction> {
        self.request::<command::SetKeyAction>(rmk_types::protocol::rynk::SetKeyRequest {
            position: rmk_types::protocol::rynk::KeyPosition { layer, row, col },
            action,
        })
    }

    pub fn get_encoder(self, layer: u8, encoder_id: u8) -> SimRynkReply<'k, 'a, command::GetEncoderAction> {
        self.request::<command::GetEncoderAction>(rmk_types::protocol::rynk::GetEncoderRequest { encoder_id, layer })
    }

    pub fn set_encoder(
        self,
        layer: u8,
        encoder_id: u8,
        action: EncoderAction,
    ) -> SimRynkReply<'k, 'a, command::SetEncoderAction> {
        self.request::<command::SetEncoderAction>(rmk_types::protocol::rynk::SetEncoderRequest {
            encoder_id,
            layer,
            action,
        })
    }
}

#[cfg(feature = "rynk")]
#[must_use = "Rynk requests must end with an expectation"]
pub struct SimRynkReply<'k, 'a, E: Endpoint> {
    keyboard: &'k mut SimKeyboard<'a>,
    transport: ConnectionType,
    timeout: Duration,
    seq: u8,
    request: Vec<u8>,
    endpoint: PhantomData<E>,
}

#[cfg(feature = "rynk")]
impl<'k, 'a, E: Endpoint> SimRynkReply<'k, 'a, E> {
    pub fn expect(self, response: E::Response) -> &'k mut SimKeyboard<'a> {
        let expected = rynk_response_frame(E::CMD, self.seq, &response);
        self.keyboard
            .rynk_packet(self.transport, self.timeout, self.request, expected);
        self.keyboard
    }

    pub fn expect_ok(self) -> &'k mut SimKeyboard<'a>
    where
        E: Endpoint<Response = ()>,
    {
        self.expect(())
    }

    pub fn expect_error(self, error: RynkError) -> &'k mut SimKeyboard<'a> {
        let expected = rynk_error_response_frame(E::CMD, self.seq, error);
        self.keyboard
            .rynk_packet(self.transport, self.timeout, self.request, expected);
        self.keyboard
    }
}

#[cfg(feature = "rynk")]
fn rynk_request_frame<T: serde::Serialize>(cmd: Cmd, seq: u8, payload: &T) -> Vec<u8> {
    let mut buf = std::vec![0u8; rmk_types::constants::RYNK_BUFFER_SIZE];
    RynkMessage::build(&mut buf, cmd, seq, payload).expect("simulator Rynk request should encode");
    buf
}

#[cfg(feature = "rynk")]
fn rynk_response_frame<T: serde::Serialize>(cmd: Cmd, seq: u8, payload: &T) -> Vec<u8> {
    let mut buf = std::vec![0u8; rmk_types::constants::RYNK_BUFFER_SIZE];
    let msg = RynkMessage::build(&mut buf, cmd, seq, &Ok::<&T, RynkError>(payload))
        .expect("simulator Rynk response should encode");
    let frame_len = msg.frame_len();
    buf.truncate(frame_len);
    buf
}

#[cfg(feature = "rynk")]
fn rynk_error_response_frame(cmd: Cmd, seq: u8, error: RynkError) -> Vec<u8> {
    let mut buf = std::vec![0u8; rmk_types::constants::RYNK_BUFFER_SIZE];
    let msg = RynkMessage::build(&mut buf, cmd, seq, &Err::<(), RynkError>(error))
        .expect("simulator Rynk error response should encode");
    let frame_len = msg.frame_len();
    buf.truncate(frame_len);
    buf
}

#[cfg(feature = "storage")]
pub mod flash {
    use std::sync::{Arc, Mutex};
    use std::vec::Vec;

    use embedded_storage::nor_flash::{
        ErrorType, NorFlash, NorFlashError, NorFlashErrorKind, ReadNorFlash, check_erase, check_read, check_write,
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum InMemoryFlashError {
        OutOfBounds,
        NotAligned,
        WriteRequiresErase,
        Poisoned,
    }

    impl NorFlashError for InMemoryFlashError {
        fn kind(&self) -> NorFlashErrorKind {
            match self {
                Self::OutOfBounds => NorFlashErrorKind::OutOfBounds,
                Self::NotAligned => NorFlashErrorKind::NotAligned,
                Self::WriteRequiresErase | Self::Poisoned => NorFlashErrorKind::Other,
            }
        }
    }

    #[derive(Debug, Clone)]
    pub struct InMemoryFlash<const SIZE: usize, const ERASE: usize = 4096, const WRITE: usize = 4> {
        data: Arc<Mutex<Vec<u8>>>,
    }

    impl<const SIZE: usize, const ERASE: usize, const WRITE: usize> Default for InMemoryFlash<SIZE, ERASE, WRITE> {
        fn default() -> Self {
            Self::new()
        }
    }

    impl<const SIZE: usize, const ERASE: usize, const WRITE: usize> InMemoryFlash<SIZE, ERASE, WRITE> {
        pub fn new() -> Self {
            Self {
                data: Arc::new(Mutex::new(std::vec![0xFF; SIZE])),
            }
        }

        pub fn snapshot(&self) -> Vec<u8> {
            self.data.lock().expect("in-memory flash mutex poisoned").clone()
        }

        fn map_error(kind: NorFlashErrorKind) -> InMemoryFlashError {
            match kind {
                NorFlashErrorKind::OutOfBounds => InMemoryFlashError::OutOfBounds,
                NorFlashErrorKind::NotAligned => InMemoryFlashError::NotAligned,
                _ => InMemoryFlashError::Poisoned,
            }
        }
    }

    impl<const SIZE: usize, const ERASE: usize, const WRITE: usize> ErrorType for InMemoryFlash<SIZE, ERASE, WRITE> {
        type Error = InMemoryFlashError;
    }

    impl<const SIZE: usize, const ERASE: usize, const WRITE: usize> ReadNorFlash for InMemoryFlash<SIZE, ERASE, WRITE> {
        const READ_SIZE: usize = 1;

        fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
            check_read(self, offset, bytes.len()).map_err(Self::map_error)?;
            let data = self.data.lock().map_err(|_| InMemoryFlashError::Poisoned)?;
            let offset = offset as usize;
            bytes.copy_from_slice(&data[offset..offset + bytes.len()]);
            Ok(())
        }

        fn capacity(&self) -> usize {
            SIZE
        }
    }

    impl<const SIZE: usize, const ERASE: usize, const WRITE: usize> NorFlash for InMemoryFlash<SIZE, ERASE, WRITE> {
        const WRITE_SIZE: usize = WRITE;
        const ERASE_SIZE: usize = ERASE;

        fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
            check_erase(self, from, to).map_err(Self::map_error)?;
            let mut data = self.data.lock().map_err(|_| InMemoryFlashError::Poisoned)?;
            data[from as usize..to as usize].fill(0xFF);
            Ok(())
        }

        fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
            check_write(self, offset, bytes.len()).map_err(Self::map_error)?;
            let mut data = self.data.lock().map_err(|_| InMemoryFlashError::Poisoned)?;
            let offset = offset as usize;
            for (idx, byte) in bytes.iter().enumerate() {
                let current = data[offset + idx];
                if current & byte != *byte {
                    return Err(InMemoryFlashError::WriteRequiresErase);
                }
                data[offset + idx] = current & byte;
            }
            Ok(())
        }
    }

    impl<const SIZE: usize, const ERASE: usize, const WRITE: usize> embedded_storage_async::nor_flash::ReadNorFlash
        for InMemoryFlash<SIZE, ERASE, WRITE>
    {
        const READ_SIZE: usize = 1;

        async fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
            ReadNorFlash::read(self, offset, bytes)
        }

        fn capacity(&self) -> usize {
            SIZE
        }
    }

    impl<const SIZE: usize, const ERASE: usize, const WRITE: usize> embedded_storage_async::nor_flash::NorFlash
        for InMemoryFlash<SIZE, ERASE, WRITE>
    {
        const WRITE_SIZE: usize = WRITE;
        const ERASE_SIZE: usize = ERASE;

        async fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
            NorFlash::erase(self, from, to)
        }

        async fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
            NorFlash::write(self, offset, bytes)
        }
    }
}
