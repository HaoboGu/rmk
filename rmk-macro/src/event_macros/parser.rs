//! Attribute parsers for event system macros.

use proc_macro2::TokenStream;

use super::config::InputDeviceConfig;
use super::utils::AttributeParser;

/// Parse input_device config from attribute tokens.
/// Extracts `publish = EventType`.
pub fn parse_input_device_config(
    tokens: impl Into<TokenStream>,
) -> Result<InputDeviceConfig, TokenStream> {
    let parser = AttributeParser::new_validated(tokens, &["publish"])?;

    parser
        .get_path("publish")
        .map(|event_type| InputDeviceConfig {
            event_type,
        })
        .ok_or_else(|| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                "#[input_device] requires `publish` attribute. Use `#[input_device(publish = EventType)]`",
            )
            .to_compile_error()
        })
}
