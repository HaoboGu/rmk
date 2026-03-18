//! Light control actions.

use postcard::experimental::max_size::MaxSize;
#[cfg(feature = "rmk_protocol")]
use postcard_schema::Schema;

/// Actions for controlling lights
#[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(MaxSize)]
#[cfg_attr(feature = "rmk_protocol", derive(Schema))]
#[cfg_attr(feature = "_codegen", derive(strum::VariantNames))]
pub enum LightAction {
    BacklightOn,
    BacklightOff,
    BacklightToggle,
    BacklightDown,
    BacklightUp,
    BacklightStep,
    BacklightToggleBreathing,
    RgbTog,
    RgbModeForward,
    RgbModeReverse,
    RgbHui,
    RgbHud,
    RgbSai,
    RgbSad,
    RgbVai,
    RgbVad,
    RgbSpi,
    RgbSpd,
    RgbModePlain,
    RgbModeBreathe,
    RgbModeRainbow,
    RgbModeSwirl,
    RgbModeSnake,
    RgbModeKnight,
    RgbModeXmas,
    RgbModeGradient,
    // Not in vial
    RgbModeRgbtest,
    RgbModeTwinkle,
}
