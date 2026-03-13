//! Shared configuration types used by firmware storage and protocol layers.

use crate::action::MorseProfile;

/// Behavior timing configuration stored in flash and exchanged over the protocol.
///
/// Field order is load-bearing: postcard serializes by position, so existing
/// flash data remains compatible as long as the order is preserved.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    postcard::experimental::max_size::MaxSize,
)]
#[cfg_attr(feature = "protocol", derive(postcard_schema::Schema))]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct BehaviorConfig {
    /// The prior-idle-time in ms used for in-flow tap.
    pub prior_idle_time: u16,
    /// Default morse profile containing mode, timeouts, and unilateral_tap settings.
    pub morse_default_profile: MorseProfile,
    /// Timeout time for combos in ms.
    pub combo_timeout: u16,
    /// Timeout time for one-shot keys in ms.
    pub one_shot_timeout: u16,
    /// Interval for tap actions in ms.
    pub tap_interval: u16,
    /// Interval for tapping capslock in ms.
    ///
    /// macOS has special processing of capslock; when tapping capslock the
    /// interval should be a different value.
    pub tap_capslock_interval: u16,
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            prior_idle_time: 0,
            morse_default_profile: MorseProfile::const_default(),
            combo_timeout: 50,
            one_shot_timeout: 500,
            tap_interval: 200,
            tap_capslock_interval: 200,
        }
    }
}
