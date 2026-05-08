use core::cell::RefCell;

use embassy_time::Duration;
use rmk_types::action::{EncoderAction, KeyAction};
use rmk_types::fork::Fork;
use rmk_types::morse::{Morse, MorseProfile};
#[cfg(all(feature = "storage", feature = "host"))]
use {
    crate::{boot::reboot_keyboard, storage::Storage},
    embedded_storage_async::nor_flash::NorFlash,
};

use crate::MACRO_SPACE_SIZE;
use crate::config::{BehaviorConfig, Hand, MouseKeyConfig, OneShotModifiersConfig, PositionalConfig};
use crate::event::{KeyboardEvent, KeyboardEventPos, LayerChangeEvent, publish_event};
use crate::input_device::rotary_encoder::Direction;
use crate::keyboard::combo::Combo;
use crate::keyboard_macros::MacroOperation;
#[cfg(feature = "host_security")]
use crate::matrix::MatrixState;

pub(crate) const HOLD_BUFFER_SIZE: usize = 16;

/// All allocated data needed to build a [`KeyMap`].
pub struct KeymapData<const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize = 0> {
    /// Per-layer key actions
    pub(crate) keymap: [[[KeyAction; COL]; ROW]; NUM_LAYER],
    /// Per-layer encoder actions
    pub(crate) encoder_map: [[EncoderAction; NUM_ENCODER]; NUM_LAYER],
    /// Per-layer activation flags
    layer_state: [bool; NUM_LAYER],
    /// Layer cache for key positions
    layer_cache: [[u8; COL]; ROW],
    /// Layer cache for encoder directions
    encoder_layer_cache: [[u8; 2]; NUM_ENCODER],
}

impl<const ROW: usize, const COL: usize, const NUM_LAYER: usize> KeymapData<ROW, COL, NUM_LAYER, 0> {
    /// Create keymap data for a keyboard without encoders.
    pub const fn new(keymap: [[[KeyAction; COL]; ROW]; NUM_LAYER]) -> Self {
        Self {
            keymap,
            encoder_map: [const { [] }; NUM_LAYER],
            layer_state: [false; NUM_LAYER],
            layer_cache: [[0; COL]; ROW],
            encoder_layer_cache: [],
        }
    }
}

impl<const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    KeymapData<ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    /// Create keymap data for a keyboard with encoders.
    pub const fn new_with_encoder(
        keymap: [[[KeyAction; COL]; ROW]; NUM_LAYER],
        encoder_map: [[EncoderAction; NUM_ENCODER]; NUM_LAYER],
    ) -> Self {
        Self {
            keymap,
            encoder_map,
            layer_state: [false; NUM_LAYER],
            layer_cache: [[0; COL]; ROW],
            encoder_layer_cache: [[0u8; 2]; NUM_ENCODER],
        }
    }
}

/// fills up the vector to its capacity
pub(crate) fn fill_vec<T: Default + Clone, const N: usize>(vector: &mut heapless::Vec<T, N>) {
    vector
        .resize(vector.capacity(), T::default())
        .expect("impossible error, as we resize to the capacity of the vector!");
}

/// KeyMap with hidden interior mutability.
///
/// Consumers use `&KeyMap` with plain method calls — no generics needed.
/// All const generic parameters are erased at construction time.
pub struct KeyMap<'a> {
    inner: RefCell<KeyMapInner<'a>>,
}

struct KeyMapInner<'a> {
    row: usize,
    col: usize,
    num_layer: usize,
    num_encoder: usize,
    /// Flat layer data: num_layer * row * col
    layers: &'a mut [KeyAction],
    /// Flat encoder data: num_layer * num_encoder (None if no encoders)
    encoders: Option<&'a mut [EncoderAction]>,
    /// Per-layer activation state
    layer_state: &'a mut [bool],
    /// Default layer number
    default_layer: u8,
    /// Layer cache for keys: row * col
    layer_cache: &'a mut [u8],
    /// Layer cache for encoders: num_encoder * 2
    encoder_layer_cache: &'a mut [u8],
    /// Behavior configuration
    behavior: &'a mut BehaviorConfig,
    /// Hand info: row * col (read-only)
    hand: &'a [Hand],
    /// Mouse button state
    mouse_buttons: u8,
    /// Matrix state for vial lock
    #[cfg(feature = "host_security")]
    matrix_state: MatrixState,
}

// ── Flat indexing helpers ──────────────────────────────────────────────

impl KeyMapInner<'_> {
    #[inline]
    fn layer_index(&self, layer: usize, row: usize, col: usize) -> usize {
        layer * self.row * self.col + row * self.col + col
    }

    #[inline]
    fn encoder_index(&self, layer: usize, id: usize) -> usize {
        layer * self.num_encoder + id
    }

    #[inline]
    fn cache_index(&self, row: usize, col: usize) -> usize {
        row * self.col + col
    }

    #[inline]
    fn encoder_cache_index(&self, id: usize, direction: usize) -> usize {
        id * 2 + direction
    }
}

// ── KeyMapInner methods (all take &mut self or &self) ─────────────────

impl KeyMapInner<'_> {
    fn get_keymap_config(&self) -> (usize, usize, usize) {
        (self.row, self.col, self.num_layer)
    }

    fn get_default_layer(&self) -> u8 {
        self.default_layer
    }

    fn set_default_layer(&mut self, layer_num: u8) {
        self.default_layer = layer_num;
    }

    fn get_action_at(&self, pos: KeyboardEventPos, layer_num: usize) -> KeyAction {
        match pos {
            KeyboardEventPos::Key(key_pos) => {
                let row = key_pos.row as usize;
                let col = key_pos.col as usize;
                self.layers[self.layer_index(layer_num, row, col)]
            }
            KeyboardEventPos::RotaryEncoder(encoder_pos) => {
                if let Some(encoders) = &self.encoders
                    && encoder_pos.direction != Direction::None
                {
                    let idx = self.encoder_index(layer_num, encoder_pos.id as usize);
                    if let Some(encoder_action) = encoders.get(idx) {
                        return match encoder_pos.direction {
                            Direction::Clockwise => encoder_action.clockwise,
                            Direction::CounterClockwise => encoder_action.counter_clockwise,
                            Direction::None => KeyAction::No,
                        };
                    }
                }
                KeyAction::No
            }
        }
    }

    fn set_action_at(&mut self, pos: KeyboardEventPos, layer_num: usize, action: KeyAction) {
        match pos {
            KeyboardEventPos::Key(key_pos) => {
                let row = key_pos.row as usize;
                let col = key_pos.col as usize;
                let idx = self.layer_index(layer_num, row, col);
                self.layers[idx] = action;
            }
            KeyboardEventPos::RotaryEncoder(encoder_pos) => {
                let idx = self.encoder_index(layer_num, encoder_pos.id as usize);
                if let Some(encoders) = &mut self.encoders
                    && let Some(encoder_action) = encoders.get_mut(idx)
                {
                    match encoder_pos.direction {
                        Direction::Clockwise => encoder_action.clockwise = action,
                        Direction::CounterClockwise => encoder_action.counter_clockwise = action,
                        Direction::None => {}
                    }
                }
            }
        }
    }

    fn get_action_with_layer_cache(&mut self, event: KeyboardEvent) -> KeyAction {
        if !event.pressed {
            let layer = self.pop_layer_from_cache(event.pos);
            return self.get_action_at(event.pos, layer as usize);
        }

        for layer_idx in (0..self.num_layer).rev() {
            if self.layer_state[layer_idx] || layer_idx as u8 == self.default_layer {
                let action = self.get_action_at(event.pos, layer_idx);
                if action == KeyAction::Transparent {
                    continue;
                }
                self.save_layer_cache(event.pos, layer_idx as u8);
                return action;
            }
            if layer_idx as u8 == self.default_layer {
                break;
            }
        }

        KeyAction::No
    }

    fn get_activated_layer(&self) -> u8 {
        for layer_idx in (0..self.num_layer).rev() {
            if self.layer_state[layer_idx] || layer_idx as u8 == self.default_layer {
                return layer_idx as u8;
            }
        }
        self.default_layer
    }

    fn pop_layer_from_cache(&mut self, pos: KeyboardEventPos) -> u8 {
        match pos {
            KeyboardEventPos::Key(key_pos) => {
                let row = key_pos.row as usize;
                let col = key_pos.col as usize;
                let ci = self.cache_index(row, col);
                let layer = self.layer_cache[ci];
                self.layer_cache[ci] = self.default_layer;
                layer
            }
            KeyboardEventPos::RotaryEncoder(encoder_pos) => {
                if encoder_pos.direction != Direction::None {
                    let ci = self.encoder_cache_index(encoder_pos.id as usize, encoder_pos.direction as usize);
                    if let Some(cache) = self.encoder_layer_cache.get_mut(ci) {
                        let layer = *cache;
                        *cache = self.default_layer;
                        return layer;
                    }
                }
                self.default_layer
            }
        }
    }

    fn save_layer_cache(&mut self, pos: KeyboardEventPos, layer_num: u8) {
        match pos {
            KeyboardEventPos::Key(key_pos) => {
                let row = key_pos.row as usize;
                let col = key_pos.col as usize;
                let ci = self.cache_index(row, col);
                self.layer_cache[ci] = layer_num;
            }
            KeyboardEventPos::RotaryEncoder(encoder_pos) => {
                if encoder_pos.direction != Direction::None {
                    let ci = self.encoder_cache_index(encoder_pos.id as usize, encoder_pos.direction as usize);
                    if let Some(cache) = self.encoder_layer_cache.get_mut(ci) {
                        *cache = layer_num;
                    }
                }
            }
        }
    }

    fn update_fn_layer_state(&mut self) {
        if self.num_layer > 3 {
            self.layer_state[3] = self.layer_state[1] && self.layer_state[2];
            let layer = self.get_activated_layer();
            publish_event(LayerChangeEvent::new(layer));
        }
    }

    fn update_tri_layer(&mut self) {
        if let Some(ref tri_layer) = self.behavior.tri_layer {
            self.layer_state[tri_layer[2] as usize] =
                self.layer_state[tri_layer[0] as usize] && self.layer_state[tri_layer[1] as usize];
        }
        let layer = self.get_activated_layer();
        publish_event(LayerChangeEvent::new(layer));
    }

    fn activate_layer(&mut self, layer_num: u8) {
        if layer_num as usize >= self.num_layer {
            warn!(
                "Not a valid layer {}, keyboard supports only {} layers",
                layer_num, self.num_layer
            );
            return;
        }
        self.layer_state[layer_num as usize] = true;
        self.update_tri_layer();
    }

    fn deactivate_layer(&mut self, layer_num: u8) {
        if layer_num as usize >= self.num_layer {
            warn!(
                "Not a valid layer {}, keyboard supports only {} layers",
                layer_num, self.num_layer
            );
            return;
        }
        self.layer_state[layer_num as usize] = false;
        self.update_tri_layer();
    }

    fn toggle_layer(&mut self, layer_num: u8) {
        if layer_num as usize >= self.num_layer {
            warn!(
                "Not a valid layer {}, keyboard supports only {} layers",
                layer_num, self.num_layer
            );
            return;
        }
        self.layer_state[layer_num as usize] = !self.layer_state[layer_num as usize];
        self.update_tri_layer();
    }
}

// ── Public KeyMap API (interior borrow hidden) ────────────────────────

impl<'a> KeyMap<'a> {
    /// Flatten [`KeymapData`] and build the `KeyMap`.
    ///
    /// This is the shared construction logic used by both `new` and `new_from_storage`.
    /// Uses `as_flattened_mut()` / `as_flattened()` (Rust 1.85+, no unsafe).
    fn build<const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>(
        data: &'a mut KeymapData<ROW, COL, NUM_LAYER, NUM_ENCODER>,
        behavior: &'a mut BehaviorConfig,
        positional_config: &'a PositionalConfig<ROW, COL>,
    ) -> Self {
        let layers = data.keymap.as_mut_slice().as_flattened_mut().as_flattened_mut();
        let encoders = if NUM_ENCODER > 0 {
            Some(data.encoder_map.as_mut_slice().as_flattened_mut())
        } else {
            None
        };
        let layer_state = &mut data.layer_state;
        let layer_cache = data.layer_cache.as_mut_slice().as_flattened_mut();
        let encoder_layer_cache = data.encoder_layer_cache.as_mut_slice().as_flattened_mut();
        let hand = positional_config.hand.as_slice().as_flattened();

        KeyMap {
            inner: RefCell::new(KeyMapInner {
                row: ROW,
                col: COL,
                num_layer: NUM_LAYER,
                num_encoder: NUM_ENCODER,
                layers,
                encoders,
                layer_state,
                default_layer: 0,
                layer_cache,
                encoder_layer_cache,
                behavior,
                hand,
                mouse_buttons: 0,
                #[cfg(feature = "host_security")]
                matrix_state: MatrixState::new(ROW, COL),
            }),
        }
    }

    /// Generic constructor — const generics stop here.
    pub async fn new<const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>(
        data: &'a mut KeymapData<ROW, COL, NUM_LAYER, NUM_ENCODER>,
        behavior: &'a mut BehaviorConfig,
        positional_config: &'a PositionalConfig<ROW, COL>,
    ) -> Self {
        fill_vec(&mut behavior.fork.forks);
        fill_vec(&mut behavior.morse.morses);
        Self::build(data, behavior, positional_config)
    }

    #[cfg(all(feature = "storage", feature = "host"))]
    pub async fn new_from_storage<
        F: NorFlash,
        const ROW: usize,
        const COL: usize,
        const NUM_LAYER: usize,
        const NUM_ENCODER: usize,
    >(
        data: &'a mut KeymapData<ROW, COL, NUM_LAYER, NUM_ENCODER>,
        storage: Option<&mut Storage<F, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
        behavior: &'a mut BehaviorConfig,
        positional_config: &'a PositionalConfig<ROW, COL>,
    ) -> Self {
        fill_vec(&mut behavior.fork.forks);
        fill_vec(&mut behavior.morse.morses);

        // Read from storage BEFORE flattening (storage expects typed arrays)
        if let Some(storage) = storage
            && {
                Ok(())
                    .and(storage.read_keymap(data).await)
                    .and(storage.read_behavior_config(behavior).await)
                    .and(
                        storage
                            .read_macro_cache(&mut behavior.keyboard_macros.macro_sequences)
                            .await,
                    )
                    .and(storage.read_combos(&mut behavior.combo.combos).await)
                    .and(storage.read_forks(&mut behavior.fork.forks).await)
                    .and(storage.read_morses(&mut behavior.morse.morses).await)
            }
            .is_err()
        {
            error!("Failed to read from storage, clearing...");
            storage.flash.erase_all().await.ok();
            reboot_keyboard();
        }

        Self::build(data, behavior, positional_config)
    }

    // ── Action resolution ──

    pub(crate) fn get_action_with_layer_cache(&self, event: KeyboardEvent) -> KeyAction {
        self.inner.borrow_mut().get_action_with_layer_cache(event)
    }

    pub(crate) fn get_action_at(&self, pos: KeyboardEventPos, layer: usize) -> KeyAction {
        self.inner.borrow().get_action_at(pos, layer)
    }

    /// Read the action currently bound to a `(layer, row, col)` position.
    ///
    /// This is the post-storage, post-Vial state — i.e. what the keyboard will
    /// actually emit when that key fires, not the compile-time default. Useful
    /// for accessory displays / status surfaces that want to mirror the live
    /// keymap.
    pub fn action_at_pos(&self, layer: usize, row: u8, col: u8) -> KeyAction {
        self.inner
            .borrow()
            .get_action_at(KeyboardEventPos::key_pos(col, row), layer)
    }

    /// Active layer index (after layer-toggle/momentary updates).
    pub fn active_layer(&self) -> u8 {
        self.inner.borrow().get_activated_layer()
    }

    pub(crate) fn set_action_at(&self, pos: KeyboardEventPos, layer: usize, action: KeyAction) {
        self.inner.borrow_mut().set_action_at(pos, layer, action);
    }

    // ── Layers ──

    pub(crate) fn activate_layer(&self, layer_num: u8) {
        self.inner.borrow_mut().activate_layer(layer_num);
    }

    pub(crate) fn deactivate_layer(&self, layer_num: u8) {
        self.inner.borrow_mut().deactivate_layer(layer_num);
    }

    pub(crate) fn toggle_layer(&self, layer_num: u8) {
        self.inner.borrow_mut().toggle_layer(layer_num);
    }

    pub(crate) fn get_activated_layer(&self) -> u8 {
        self.inner.borrow().get_activated_layer()
    }

    pub(crate) fn get_default_layer(&self) -> u8 {
        self.inner.borrow().get_default_layer()
    }

    pub(crate) fn set_default_layer(&self, layer_num: u8) {
        self.inner.borrow_mut().set_default_layer(layer_num);
    }

    pub(crate) fn update_fn_layer_state(&self) {
        self.inner.borrow_mut().update_fn_layer_state();
    }

    // ── Config ──

    pub(crate) fn get_keymap_config(&self) -> (usize, usize, usize) {
        self.inner.borrow().get_keymap_config()
    }

    pub(crate) fn num_encoders(&self) -> usize {
        self.inner.borrow().num_encoder
    }

    pub(crate) fn hand_at(&self, row: usize, col: usize) -> Hand {
        let inner = self.inner.borrow();
        let idx = inner.cache_index(row, col);
        if idx < inner.hand.len() {
            inner.hand[idx]
        } else {
            Hand::Unknown
        }
    }

    // ── Behavior getters (borrow scoped inside each method) ──

    pub(crate) fn combo_timeout(&self) -> Duration {
        self.inner.borrow().behavior.combo.timeout
    }

    pub(crate) fn one_shot_timeout(&self) -> Duration {
        self.inner.borrow().behavior.one_shot.timeout
    }

    pub(crate) fn one_shot_modifiers_config(&self) -> OneShotModifiersConfig {
        self.inner.borrow().behavior.one_shot_modifiers
    }

    pub(crate) fn tap_interval(&self) -> u16 {
        self.inner.borrow().behavior.tap.tap_interval
    }

    pub(crate) fn tap_capslock_interval(&self) -> u16 {
        self.inner.borrow().behavior.tap.tap_capslock_interval
    }

    pub(crate) fn morse_enable_flow_tap(&self) -> bool {
        self.inner.borrow().behavior.morse.enable_flow_tap
    }

    pub(crate) fn morse_prior_idle_time(&self) -> Duration {
        self.inner.borrow().behavior.morse.prior_idle_time
    }

    pub(crate) fn morse_default_profile(&self) -> MorseProfile {
        self.inner.borrow().behavior.morse.default_profile
    }

    pub(crate) fn mouse_key_config(&self) -> MouseKeyConfig {
        self.inner.borrow().behavior.mouse_key
    }

    pub(crate) fn forks_is_empty(&self) -> bool {
        self.inner.borrow().behavior.fork.forks.is_empty()
    }

    pub(crate) fn morses_len(&self) -> usize {
        self.inner.borrow().behavior.morse.morses.len()
    }

    // ── Behavior setters ──

    pub(crate) fn set_combo_timeout(&self, timeout: Duration) {
        self.inner.borrow_mut().behavior.combo.timeout = timeout;
    }

    pub(crate) fn set_one_shot_timeout(&self, timeout: Duration) {
        self.inner.borrow_mut().behavior.one_shot.timeout = timeout;
    }

    pub(crate) fn set_tap_interval(&self, interval: u16) {
        self.inner.borrow_mut().behavior.tap.tap_interval = interval;
    }

    pub(crate) fn set_tap_capslock_interval(&self, interval: u16) {
        self.inner.borrow_mut().behavior.tap.tap_capslock_interval = interval;
    }

    pub(crate) fn set_morse_default_profile(&self, profile: MorseProfile) {
        self.inner.borrow_mut().behavior.morse.default_profile = profile;
    }

    pub(crate) fn set_morse_prior_idle_time(&self, time: Duration) {
        self.inner.borrow_mut().behavior.morse.prior_idle_time = time;
    }

    // ── Per-element morse ──

    pub(crate) fn get_morse(&self, idx: usize) -> Option<Morse> {
        self.inner.borrow().behavior.morse.morses.get(idx).cloned()
    }

    pub(crate) fn with_morse_mut<R>(&self, idx: usize, f: impl FnOnce(&mut Morse) -> R) -> Option<R> {
        self.inner.borrow_mut().behavior.morse.morses.get_mut(idx).map(f)
    }

    // ── Collection closures ──

    pub(crate) fn with_forks<R>(&self, f: impl FnOnce(&[Fork]) -> R) -> R {
        let inner = self.inner.borrow();
        f(&inner.behavior.fork.forks)
    }

    pub(crate) fn with_combos<R>(&self, f: impl FnOnce(&[Option<Combo>]) -> R) -> R {
        let inner = self.inner.borrow();
        f(&inner.behavior.combo.combos)
    }

    pub(crate) fn with_combos_mut<R>(&self, f: impl FnOnce(&mut [Option<Combo>]) -> R) -> R {
        let mut inner = self.inner.borrow_mut();
        f(&mut inner.behavior.combo.combos)
    }

    // ── Macros ──

    pub(crate) fn get_macro_sequence_start(&self, idx: u8) -> Option<usize> {
        MacroOperation::get_macro_sequence_start(&self.inner.borrow().behavior.keyboard_macros.macro_sequences, idx)
    }

    pub(crate) fn get_next_macro_operation(&self, start: usize, offset: usize) -> (MacroOperation, usize) {
        MacroOperation::get_next_macro_operation(
            &self.inner.borrow().behavior.keyboard_macros.macro_sequences,
            start,
            offset,
        )
    }

    // ── Mouse ──

    pub(crate) fn mouse_buttons(&self) -> u8 {
        self.inner.borrow().mouse_buttons
    }

    pub(crate) fn set_mouse_buttons(&self, buttons: u8) {
        self.inner.borrow_mut().mouse_buttons = buttons;
    }

    // ── Bulk flat access (for Vial DynamicKeymapGetBuffer/SetBuffer) ──

    pub(crate) fn get_action_by_flat_index(&self, index: usize) -> KeyAction {
        let inner = self.inner.borrow();
        if index < inner.layers.len() {
            inner.layers[index]
        } else {
            KeyAction::No
        }
    }

    pub(crate) fn set_action_by_flat_index(&self, index: usize, action: KeyAction) {
        let mut inner = self.inner.borrow_mut();
        if index < inner.layers.len() {
            inner.layers[index] = action;
        }
    }

    // ── Encoder access (for Vial GetEncoder/SetEncoder) ──

    pub(crate) fn get_encoder_action(&self, layer: usize, id: usize) -> Option<EncoderAction> {
        let inner = self.inner.borrow();
        inner.encoders.as_ref().and_then(|encoders| {
            let idx = inner.encoder_index(layer, id);
            encoders.get(idx).copied()
        })
    }

    pub(crate) fn set_encoder_clockwise(&self, layer: usize, id: usize, action: KeyAction) -> Option<EncoderAction> {
        let mut inner = self.inner.borrow_mut();
        let idx = inner.encoder_index(layer, id);
        if let Some(encoders) = &mut inner.encoders
            && let Some(encoder_action) = encoders.get_mut(idx)
        {
            encoder_action.clockwise = action;
            return Some(*encoder_action);
        }
        None
    }

    pub(crate) fn set_encoder_counter_clockwise(
        &self,
        layer: usize,
        id: usize,
        action: KeyAction,
    ) -> Option<EncoderAction> {
        let mut inner = self.inner.borrow_mut();
        let idx = inner.encoder_index(layer, id);
        if let Some(encoders) = &mut inner.encoders
            && let Some(encoder_action) = encoders.get_mut(idx)
        {
            encoder_action.counter_clockwise = action;
            return Some(*encoder_action);
        }
        None
    }

    // ── Macro buffer (for Vial DynamicKeymapMacroGetBuffer/SetBuffer) ──

    pub(crate) fn read_macro_buffer(&self, offset: usize, target: &mut [u8]) {
        let inner = self.inner.borrow();
        let src = &inner.behavior.keyboard_macros.macro_sequences;
        let end = (offset + target.len()).min(src.len());
        if offset < end {
            target[..end - offset].copy_from_slice(&src[offset..end]);
        }
    }

    pub(crate) fn write_macro_buffer(&self, offset: usize, data: &[u8]) {
        let mut inner = self.inner.borrow_mut();
        let dst = &mut inner.behavior.keyboard_macros.macro_sequences;
        let end = (offset + data.len()).min(dst.len());
        if offset < end {
            dst[offset..end].copy_from_slice(&data[..end - offset]);
        }
    }

    pub(crate) fn reset_macro_buffer(&self) {
        self.inner.borrow_mut().behavior.keyboard_macros.macro_sequences = [0; MACRO_SPACE_SIZE];
    }

    pub(crate) fn get_macro_sequences(&self) -> [u8; MACRO_SPACE_SIZE] {
        self.inner.borrow().behavior.keyboard_macros.macro_sequences
    }

    // ── Matrix state (host_security) ──

    #[cfg(feature = "host_security")]
    pub(crate) fn update_matrix_state(&self, event: &KeyboardEvent) {
        self.inner.borrow_mut().matrix_state.update(event);
    }

    #[cfg(feature = "host_security")]
    pub(crate) fn read_matrix_state(&self, target: &mut [u8]) {
        self.inner.borrow().matrix_state.read_all(target);
    }

    #[cfg(feature = "host_security")]
    pub(crate) fn read_matrix_key(&self, row: u8, col: u8) -> bool {
        self.inner.borrow().matrix_state.read(row, col)
    }
}

#[cfg(test)]
mod test {
    use rmk_types::fork::{Fork, StateBits};
    use rmk_types::modifier::ModifierCombination;

    use crate::keyboard::combo::{Combo, ComboConfig};
    use crate::keymap::fill_vec;
    use crate::{COMBO_MAX_NUM, FORK_MAX_NUM, k};

    #[test]
    fn test_fill_vec() {
        let mut combos: heapless::Vec<_, COMBO_MAX_NUM> = heapless::Vec::from_slice(&[
            Combo::new(ComboConfig::new([k!(A), k!(B), k!(C), k!(D)], k!(Z), None)),
            Combo::new(ComboConfig::new([k!(A), k!(B)], k!(X), None)),
            Combo::new(ComboConfig::new([k!(A), k!(B), k!(C)], k!(Y), None)),
        ])
        .unwrap();

        fill_vec(&mut combos);

        assert_eq!(combos.len(), COMBO_MAX_NUM);

        let mut forks: heapless::Vec<_, FORK_MAX_NUM> = heapless::Vec::from_slice(&[
            Fork::new(
                k!(A),
                k!(Y),
                k!(F),
                StateBits::default(),
                StateBits::default(),
                ModifierCombination::new(),
                false,
            ),
            Fork::new(
                k!(B),
                k!(B),
                k!(F),
                StateBits::default(),
                StateBits::default(),
                ModifierCombination::new(),
                false,
            ),
            Fork::new(
                k!(C),
                k!(Y),
                k!(Y),
                StateBits::default(),
                StateBits::default(),
                ModifierCombination::new(),
                false,
            ),
        ])
        .unwrap();

        fill_vec(&mut forks);

        assert_eq!(forks.len(), FORK_MAX_NUM);
    }
}
