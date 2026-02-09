//! Attribute parsers for input device macros.

use proc_macro2::TokenStream;

use super::config::InputDeviceConfig;
use crate::utils::AttributeParser;

/// Parse input_device config from attribute tokens.
/// Extracts `publish = EventType`.
pub fn parse_input_device_config(tokens: impl Into<TokenStream>) -> Result<Option<InputDeviceConfig>, TokenStream> {
    let parser = match AttributeParser::new(tokens) {
        Ok(p) => p,
        Err(_) => return Ok(None),
    };
    parser.validate_keys(&["publish"])?;
    Ok(parser
        .get_path("publish")
        .map(|event_type| InputDeviceConfig { event_type }))
}
