// Code-generated from rmk-types by tsify — DO NOT EDIT.
// Rust↔TS wire-type contract for the Rynk protocol.
// Regenerate: cd rynk/rynk-wasm && ./scripts/gen-types.sh

/* tslint:disable */
/* eslint-disable */
/**
 * A KeyAction is the action at a keyboard position, stored in keymap.
 * It can be a single action like triggering a key, or a composite keyboard action like tap/hold
 */
export type KeyAction = "No" | "Transparent" | { Single: Action } | { Tap: Action } | { TapHold: [Action, Action, MorseProfile] } | { Morse: number };

/**
 * A decoded topic push (server → host), one variant per row of the
 * topic table above — generated from it. `Serialize` lets the host
 * re-emit a decoded topic as JSON (every payload is already a wire type).
 */
export type TopicEvent = { LayerChange: number } | { WpmUpdate: number } | { ConnectionChange: ConnectionStatus } | { SleepState: boolean } | { LedIndicatorChange: LedIndicator } | { BatteryStatusChange: BatteryStatus };

/**
 * A single basic action that a keyboard can execute.
 */
export type Action = "No" | { Key: KeyCode } | { Modifier: ModifierCombination } | { KeyWithModifier: [KeyCode, ModifierCombination] } | { LayerOn: number } | { LayerOnWithModifier: [number, ModifierCombination] } | { LayerOff: number } | { LayerToggle: number } | { DefaultLayer: number } | { LayerToggleOnly: number } | "TriLayerLower" | "TriLayerUpper" | { TriggerMacro: number } | { OneShotLayer: number } | { OneShotModifier: ModifierCombination } | { OneShotKey: KeyCode } | { Light: LightAction } | { KeyboardControl: KeyboardAction } | { Special: SpecialKey } | { User: number } | { PersistentDefaultLayer: number } | { Steno: StenoKey };

/**
 * A single steno key, identified by its position (0..=63) in the canonical
 * Plover HID key chart. See module docs for the full list.
 */
export type StenoKey = number;

/**
 * Action for a rotary encoder position, stored in the encoder map.
 *
 * Both fields default to `KeyAction::No` (no action).
 */
export interface EncoderAction {
    /**
     * Action triggered when the encoder is rotated clockwise.
     */
    clockwise: KeyAction;
    /**
     * Action triggered when the encoder is rotated counter-clockwise.
     */
    counter_clockwise: KeyAction;
}

/**
 * Actions for controlling lights
 */
export type LightAction = "BacklightOn" | "BacklightOff" | "BacklightToggle" | "BacklightDown" | "BacklightUp" | "BacklightStep" | "BacklightToggleBreathing" | "RgbTog" | "RgbModeForward" | "RgbModeReverse" | "RgbHui" | "RgbHud" | "RgbSai" | "RgbSad" | "RgbVai" | "RgbVad" | "RgbSpi" | "RgbSpd" | "RgbModePlain" | "RgbModeBreathe" | "RgbModeRainbow" | "RgbModeSwirl" | "RgbModeSnake" | "RgbModeKnight" | "RgbModeXmas" | "RgbModeGradient" | "RgbModeRgbtest" | "RgbModeTwinkle";

/**
 * Actions for controlling the keyboard or changing the keyboard\'s state, for example, enable/disable a particular function
 */
export type KeyboardAction = "Bootloader" | "Reboot" | "DebugToggle" | "ClearEeprom" | "OutputAuto" | "OutputUsb" | "OutputBluetooth" | "ComboOn" | "ComboOff" | "ComboToggle" | "CapsWordToggle";

/**
 * BLE state (what the BLE subsystem is currently doing).
 */
export type BleState = "Advertising" | "Connected" | "Inactive";

/**
 * Battery status used for both status queries and event notifications.
 */
export type BatteryStatus = "Unavailable" | { Available: { charge_state: ChargeState; level: number | undefined } };

/**
 * Bitset state used by fork matching logic.
 *
 * A zero (default) value means \"match nothing\" — no modifiers, LEDs, or mouse buttons.
 */
export interface StateBits {
    /**
     * Active modifier combination to match.
     */
    modifiers: ModifierCombination;
    /**
     * LED indicator state to match (Num/Caps/Scroll Lock, etc.).
     */
    leds: LedIndicator;
    /**
     * Mouse button state to match.
     */
    mouse: MouseButtons;
}

/**
 * Bulk request payload for setting multiple combos at once.
 */
export interface SetComboBulkRequest {
    start_index: number;
    configs: Combo[];
}

/**
 * Bulk request payload for setting multiple morse configs at once.
 */
export interface SetMorseBulkRequest {
    start_index: number;
    configs: Morse[];
}

/**
 * Bulk response for getting multiple combos at once.
 */
export interface GetComboBulkResponse {
    configs: Combo[];
}

/**
 * Bulk response for getting multiple key actions at once.
 */
export interface GetKeymapBulkResponse {
    actions: KeyAction[];
}

/**
 * Bulk response for getting multiple morse configs at once.
 */
export interface GetMorseBulkResponse {
    configs: Morse[];
}

/**
 * Challenge returned by the Unlock endpoint containing physical key positions to press.
 */
export interface UnlockChallenge {
    key_positions: [number, number][];
}

/**
 * Charge state of the battery.
 */
export type ChargeState = "Charging" | "Discharging" | "Unknown";

/**
 * Configuration data for a combo.
 *
 * A combo triggers an output action when a set of keys are pressed simultaneously.
 * The maximum number of trigger keys is determined by `COMBO_SIZE` (from `constants.rs`,
 * generated at build time from `keyboard.toml` on firmware or fixed upper bound on host).
 * Actions are stored in a Vec — only meaningful keys are present (no `KeyAction::No` padding).
 *
 * Note: `COMBO_SIZE` is a **wire-format** capacity — on firmware it equals
 * `COMBO_MAX_LENGTH` (from `keyboard.toml`), on host it\'s a fixed upper bound.
 */
export interface Combo {
    actions: KeyAction[];
    output: KeyAction;
    layer: number | undefined;
}

/**
 * Connection type for the keyboard.
 */
export type ConnectionType = "Usb" | "Ble";

/**
 * Current lock/unlock state of the device.
 */
export interface LockStatus {
    locked: boolean;
    awaiting_keys: boolean;
    remaining_keys: number;
}

/**
 * Current matrix key-press state as a bitmap.
 * Bit ordering: row-major, bit 0 = col 0, bit 1 = col 1, etc.
 * Total meaningful bytes = num_rows * ceil(num_cols / 8).
 */
export interface MatrixState {
    pressed_bitmap: number[];
}

/**
 * Definition of a morse key.
 *
 * A morse key is a key that behaves differently according to the pattern of a tap/hold sequence.
 * The maximum number of taps is limited to 15 by the internal u16 representation of MorsePattern.
 * There is a list of (pattern, corresponding action) pairs for each morse key:
 * The number of pairs is limited by `MORSE_SIZE` (from `constants.rs`, generated at build time).
 *
 * Note: `MORSE_SIZE` is a **wire-format** capacity — on firmware it equals
 * `MAX_PATTERNS_PER_KEY` (from `keyboard.toml`), on host it\'s a fixed upper bound.
 */
export interface Morse {
    /**
     * The profile of this morse key, which defines the timing parameters, etc.
     * If some of its fields are filled with None, the global default value will be used.
     */
    profile: MorseProfile;
    /**
     * The list of pattern -> action pairs, which can be triggered
     */
    actions: [number, Action][];
}

/**
 * Device capabilities discovered during the connection handshake.
 *
 * The host reads this once after connecting to learn the firmware\'s layout,
 * feature set, and protocol limits.
 */
export interface DeviceCapabilities {
    num_layers: number;
    num_rows: number;
    num_cols: number;
    num_encoders: number;
    max_combos: number;
    max_combo_keys: number;
    /**
     * Number of macros supported, set by the user at compile time. `0`
     * disables macros: the host MUST NOT use them or consult `macro_space_size`.
     */
    max_macros: number;
    /**
     * Byte size of the flat macro region; only meaningful when `max_macros > 0`.
     */
    macro_space_size: number;
    max_morse: number;
    max_patterns_per_key: number;
    max_forks: number;
    storage_enabled: boolean;
    lighting_enabled: boolean;
    is_split: boolean;
    num_split_peripherals: number;
    ble_enabled: boolean;
    num_ble_profiles: number;
    max_payload_size: number;
    max_bulk_keys: number;
    macro_chunk_size: number;
    bulk_transfer_supported: boolean;
}

/**
 * Fork (key override) configuration.
 *
 * A fork overrides a key\'s output based on the current modifier/LED/mouse state.
 * When the trigger key is pressed, the fork checks current state against `match_any`
 * and `match_none` to decide between `positive_output` and `negative_output`.
 */
export interface Fork {
    /**
     * The key action that activates this fork. Should not be `KeyAction::Transparent`.
     */
    trigger: KeyAction;
    /**
     * Output when the state condition is NOT met.
     */
    negative_output: KeyAction;
    /**
     * Output when the state condition IS met.
     */
    positive_output: KeyAction;
    /**
     * If any of these state bits are active, the positive branch is taken.
     */
    match_any: StateBits;
    /**
     * If any of these state bits are active, the fork is suppressed.
     */
    match_none: StateBits;
    /**
     * Modifiers to keep active when the fork fires.
     */
    kept_modifiers: ModifierCombination;
    /**
     * Whether this fork can be rebound via protocol.
     * This is a firmware-enforced policy — the protocol itself does not
     * reject writes to non-bindable forks; enforcement happens in the
     * firmware\'s SetFork handler.
     */
    bindable: boolean;
}

/**
 * Identifies a specific key position in the keymap.
 */
export interface KeyPosition {
    layer: number;
    row: number;
    col: number;
}

/**
 * Key codes which are not in the HID spec, but still commonly used
 */
export type SpecialKey = "GraveEscape" | "Repeat";

/**
 * Keys in `Generic Desktop Page`, generally used for system control
 * Ref: <https://www.usb.org/sites/default/files/documents/hut1_12v2.pdf#page=26>
 */
export type SystemControlKey = "No" | "PowerDown" | "Sleep" | "WakeUp" | "Restart";

/**
 * Keys in consumer page
 * Ref: <https://www.usb.org/sites/default/files/documents/hut1_12v2.pdf#page=75>
 */
export type ConsumerKey = "No" | "SnapShot" | "BrightnessUp" | "BrightnessDown" | "Play" | "Pause" | "Record" | "FastForward" | "Rewind" | "NextTrack" | "PrevTrack" | "StopPlay" | "Eject" | "RandomPlay" | "Repeat" | "StopEject" | "PlayPause" | "Mute" | "VolumeIncrement" | "VolumeDecrement" | "Reserved" | "Email" | "Calculator" | "LocalBrowser" | "Lock" | "ControlPanel" | "Assistant" | "New" | "Open" | "Close" | "Exit" | "Maximize" | "Minimize" | "Save" | "Print" | "Properties" | "Undo" | "Copy" | "Cut" | "Paste" | "SelectAll" | "Find" | "Search" | "Home" | "Back" | "Forward" | "Stop" | "Refresh" | "Bookmarks" | "NextKeyboardLayoutSelect" | "DesktopShowAllWindows" | "AcSoftKeyLeft";

/**
 * Mode for morse key behavior
 */
export type MorseMode = "PermissiveHold" | "HoldOnOtherPress" | "Normal";

/**
 * MorsePattern is a sequence of maximum 15 taps or holds that can be encoded into an u16:
 * 0x1 when empty, then 0 for tap or 1 for hold shifted from the right
 */
export type MorsePattern = number;

/**
 * Protocol version advertised during the connection handshake.
 */
export interface ProtocolVersion {
    major: number;
    minor: number;
}

/**
 * Protocol-facing behavior configuration (global timing settings).
 */
export interface BehaviorConfig {
    combo_timeout_ms: number;
    oneshot_timeout_ms: number;
    tap_interval_ms: number;
    tap_capslock_interval_ms: number;
}

/**
 * Protocol-level error returned in response payload.
 */
export type RynkError = "Malformed" | "NotReady" | "StorageFault" | "Internal" | "Unimplemented" | "Invalid" | "UnknownCmd";

/**
 * Raw macro data for a single macro chunk.
 */
export interface MacroData {
    data: number[];
}

/**
 * Request payload for `GetComboBulk`.
 */
export interface GetComboBulkRequest {
    start_index: number;
    count: number;
}

/**
 * Request payload for `GetEncoderAction`.
 */
export interface GetEncoderRequest {
    encoder_id: number;
    layer: number;
}

/**
 * Request payload for `GetKeymapBulk` endpoint.
 *
 * Keys are linearized in row-major order starting from `(start_row, start_col)`.
 * `count` is the number of keys to read; iteration wraps to subsequent
 * rows when the end of a row is reached.
 */
export interface GetKeymapBulkRequest {
    layer: number;
    start_row: number;
    start_col: number;
    count: number;
}

/**
 * Request payload for `GetMacro`.
 */
export interface GetMacroRequest {
    index: number;
    offset: number;
}

/**
 * Request payload for `GetMorseBulk`.
 */
export interface GetMorseBulkRequest {
    start_index: number;
    count: number;
}

/**
 * Request payload for `SetCombo`.
 */
export interface SetComboRequest {
    index: number;
    config: Combo;
}

/**
 * Request payload for `SetEncoderAction`.
 */
export interface SetEncoderRequest {
    encoder_id: number;
    layer: number;
    action: EncoderAction;
}

/**
 * Request payload for `SetFork`.
 */
export interface SetForkRequest {
    index: number;
    config: Fork;
}

/**
 * Request payload for `SetKeyAction` endpoint.
 */
export interface SetKeyRequest {
    position: KeyPosition;
    action: KeyAction;
}

/**
 * Request payload for `SetKeymapBulk` endpoint.
 *
 * Keys are linearized in row-major order starting from `(start_row, start_col)`.
 * Iteration wraps to subsequent rows when the end of a row is reached.
 * The number of keys to write is derived from `actions.len()`.
 */
export interface SetKeymapBulkRequest {
    layer: number;
    start_row: number;
    start_col: number;
    actions: KeyAction[];
}

/**
 * Request payload for `SetMacro`.
 *
 * Writes are by `offset`; chunk length carries no end-of-macro meaning, and
 * writes past the macro region are truncated by the firmware.
 */
export interface SetMacroRequest {
    index: number;
    offset: number;
    data: MacroData;
}

/**
 * Request payload for `SetMorse`.
 */
export interface SetMorseRequest {
    index: number;
    config: Morse;
}

/**
 * Status of a single split peripheral.
 */
export interface PeripheralStatus {
    connected: boolean;
    battery: BatteryStatus;
}

/**
 * Storage reset mode for the `StorageReset` endpoint.
 */
export type StorageResetMode = "Full" | "LayoutOnly";

/**
 * USB device lifecycle. `Suspended` is distinct from `Configured` because
 * the bus is enumerated but transmission is gated on remote wakeup — the
 * first key still needs to reach the USB writer to trigger that wakeup.
 */
export type UsbState = "Disabled" | "Enabled" | "Configured" | "Suspended";

/**
 * Unified BLE status: which profile is active and what the BLE is doing.
 */
export interface BleStatus {
    profile: number;
    state: BleState;
}

/**
 * Unified connection status: the single source of truth for transport
 * availability and routing. The active transport is derived on demand via
 * [`Self::decide_active`] from the input fields below.
 */
export interface ConnectionStatus {
    usb: UsbState;
    ble: BleStatus;
    /**
     * Tiebreaker when both transports are ready.
     */
    preferred: ConnectionType;
}

export type HidKeyCode = "No" | "ErrorRollover" | "PostFail" | "ErrorUndefined" | "A" | "B" | "C" | "D" | "E" | "F" | "G" | "H" | "I" | "J" | "K" | "L" | "M" | "N" | "O" | "P" | "Q" | "R" | "S" | "T" | "U" | "V" | "W" | "X" | "Y" | "Z" | "Kc1" | "Kc2" | "Kc3" | "Kc4" | "Kc5" | "Kc6" | "Kc7" | "Kc8" | "Kc9" | "Kc0" | "Enter" | "Escape" | "Backspace" | "Tab" | "Space" | "Minus" | "Equal" | "LeftBracket" | "RightBracket" | "Backslash" | "NonusHash" | "Semicolon" | "Quote" | "Grave" | "Comma" | "Dot" | "Slash" | "CapsLock" | "F1" | "F2" | "F3" | "F4" | "F5" | "F6" | "F7" | "F8" | "F9" | "F10" | "F11" | "F12" | "PrintScreen" | "ScrollLock" | "Pause" | "Insert" | "Home" | "PageUp" | "Delete" | "End" | "PageDown" | "Right" | "Left" | "Down" | "Up" | "NumLock" | "KpSlash" | "KpAsterisk" | "KpMinus" | "KpPlus" | "KpEnter" | "Kp1" | "Kp2" | "Kp3" | "Kp4" | "Kp5" | "Kp6" | "Kp7" | "Kp8" | "Kp9" | "Kp0" | "KpDot" | "NonusBackslash" | "Application" | "KbPower" | "KpEqual" | "F13" | "F14" | "F15" | "F16" | "F17" | "F18" | "F19" | "F20" | "F21" | "F22" | "F23" | "F24" | "Execute" | "Help" | "Menu" | "Select" | "Stop" | "Again" | "Undo" | "Cut" | "Copy" | "Paste" | "Find" | "KbMute" | "KbVolumeUp" | "KbVolumeDown" | "LockingCapsLock" | "LockingNumLock" | "LockingScrollLock" | "KpComma" | "KpEqualAs400" | "International1" | "International2" | "International3" | "International4" | "International5" | "International6" | "International7" | "International8" | "International9" | "Language1" | "Language2" | "Language3" | "Language4" | "Language5" | "Language6" | "Language7" | "Language8" | "Language9" | "AlternateErase" | "SystemRequest" | "Cancel" | "Clear" | "Prior" | "Return" | "Separator" | "Out" | "Oper" | "ClearAgain" | "Crsel" | "Exsel" | "SystemPower" | "SystemSleep" | "SystemWake" | "AudioMute" | "AudioVolUp" | "AudioVolDown" | "MediaNextTrack" | "MediaPrevTrack" | "MediaStop" | "MediaPlayPause" | "MediaSelect" | "MediaEject" | "Mail" | "Calculator" | "MyComputer" | "WwwSearch" | "WwwHome" | "WwwBack" | "WwwForward" | "WwwStop" | "WwwRefresh" | "WwwFavorites" | "MediaFastForward" | "MediaRewind" | "BrightnessUp" | "BrightnessDown" | "ControlPanel" | "Assistant" | "MissionControl" | "Launchpad" | "MouseUp" | "MouseDown" | "MouseLeft" | "MouseRight" | "MouseBtn1" | "MouseBtn2" | "MouseBtn3" | "MouseBtn4" | "MouseBtn5" | "MouseBtn6" | "MouseBtn7" | "MouseBtn8" | "MouseWheelUp" | "MouseWheelDown" | "MouseWheelLeft" | "MouseWheelRight" | "MouseAccel0" | "MouseAccel1" | "MouseAccel2" | "LCtrl" | "LShift" | "LAlt" | "LGui" | "RCtrl" | "RShift" | "RAlt" | "RGui";

export type KeyCode = { Hid: HidKeyCode } | { Consumer: ConsumerKey } | { SystemControl: SystemControlKey };

export type LedIndicator = { num_lock: boolean; caps_lock: boolean; scroll_lock: boolean; compose: boolean; kana: boolean; };

export type ModifierCombination = { left_ctrl: boolean; left_shift: boolean; left_alt: boolean; left_gui: boolean; right_ctrl: boolean; right_shift: boolean; right_alt: boolean; right_gui: boolean; };

export type MorseProfile = { unilateral_tap: boolean | undefined; mode: MorseMode | undefined; hold_timeout_ms: number | undefined; gap_timeout_ms: number | undefined; };

export type MouseButtons = { button1: boolean; button2: boolean; button3: boolean; button4: boolean; button5: boolean; button6: boolean; button7: boolean; button8: boolean; };
