//! Attribute parsers for event system macros.

use proc_macro2::TokenStream;

use super::config::InputDeviceConfig;
use super::utils::AttributeParser;

/// Parse input_device config from attribute tokens.
/// Extracts `publish = EventType`.
pub fn parse_input_device_config(
    tokens: impl Into<TokenStream>,
) -> Result<Option<InputDeviceConfig>, TokenStream> {
    let parser = AttributeParser::new(tokens).map_err(|e| e.to_compile_error())?;
    parser.validate_keys(&["publish"])?;
    Ok(parser
        .get_path("publish")
        .map(|event_type| InputDeviceConfig { event_type }))
}
