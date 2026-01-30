// Layout configuration types

use serde::Deserialize;

/// Configurations for keyboard layout (internal TOML representation)
/// This is renamed from LayoutTomlConfig to LayoutDefinition
#[derive(Clone, Debug, Deserialize)]
#[allow(unused)]
pub struct LayoutDefinition {
    pub rows: u8,
    pub cols: u8,
    pub layers: u8,
    pub keymap: Option<Vec<Vec<Vec<String>>>>, // Will be deprecated in the future
    pub matrix_map: Option<String>,            // Temporarily allow both matrix_map and keymap to be set
    pub encoder_map: Option<Vec<Vec<[String; 2]>>>, // Will be deprecated together with keymap
}

/// Layer definition (internal TOML representation)
/// This is renamed from LayerTomlConfig to LayerDefinition
#[derive(Clone, Debug, Deserialize)]
#[allow(unused)]
pub struct LayerDefinition {
    pub name: Option<String>,
    pub keys: String,
    pub encoders: Option<Vec<[String; 2]>>,
}

/// Configurations for keyboard layout (public API type)
/// This is renamed from LayoutConfig to Layout
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    pub rows: u8,
    pub cols: u8,
    pub layers: u8,
    pub keymap: Vec<Vec<Vec<String>>>,
    pub encoder_map: Vec<Vec<[String; 2]>>, // Empty if there are no encoders or not configured
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyInfo {
    pub hand: char, // 'L' or 'R' or other chars
}
