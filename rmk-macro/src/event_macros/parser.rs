//! Attribute parsers for event system macros.
//!
//! Merged from input/parser.rs and controller/parser.rs.

use proc_macro2::TokenStream;
use syn::{Attribute, Meta};

use super::config::{
    ControllerConfig, ControllerEventChannelConfig, InputDeviceConfig, InputProcessorConfig,
};
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

/// Parse input_processor config from attribute tokens.
/// Extracts `subscribe = [...]`.
pub fn parse_input_processor_config(
    tokens: impl Into<TokenStream>,
) -> Result<InputProcessorConfig, TokenStream> {
    let parser = AttributeParser::new(tokens).map_err(|e| e.to_compile_error())?;
    parser.validate_keys(&["subscribe"])?;

    Ok(InputProcessorConfig {
        event_types: parser.get_path_array("subscribe")?,
    })
}

/// Parse input_event channel_size from a TokenStream.
pub fn parse_input_event_channel_size(
    tokens: impl Into<TokenStream>,
) -> Result<Option<TokenStream>, TokenStream> {
    let parser = AttributeParser::new(tokens).map_err(|e| e.to_compile_error())?;
    parser.validate_keys(&["channel_size"])?;
    Ok(parser.get_expr_tokens("channel_size"))
}

/// Parse input_event channel_size from an Attribute.
pub fn parse_input_event_channel_size_from_attr(
    attr: &Attribute,
) -> Result<Option<TokenStream>, TokenStream> {
    if let Meta::List(meta_list) = &attr.meta {
        parse_input_event_channel_size(meta_list.tokens.clone())
    } else {
        Ok(None)
    }
}

/// Parse controller config from attribute tokens.
/// Extracts `subscribe = [...]` and optional `poll_interval = N`.
pub fn parse_controller_config(
    tokens: impl Into<TokenStream>,
) -> Result<ControllerConfig, TokenStream> {
    let parser = AttributeParser::new(tokens).map_err(|e| e.to_compile_error())?;
    parser.validate_keys(&["subscribe", "poll_interval"])?;

    Ok(ControllerConfig {
        event_types: parser.get_path_array("subscribe")?,
        poll_interval_ms: parser.get_int("poll_interval")?,
    })
}

/// Parse controller_event parameters from a TokenStream.
/// Extracts `channel_size`, `subs`, `pubs`.
pub fn parse_controller_event_channel_config(
    tokens: impl Into<TokenStream>,
) -> Result<ControllerEventChannelConfig, TokenStream> {
    let parser = AttributeParser::new(tokens).map_err(|e| e.to_compile_error())?;
    parser.validate_keys(&["channel_size", "subs", "pubs"])?;

    Ok(ControllerEventChannelConfig {
        channel_size: parser.get_expr_tokens("channel_size"),
        subs: parser.get_expr_tokens("subs"),
        pubs: parser.get_expr_tokens("pubs"),
    })
}

/// Parse controller_event parameters from an Attribute.
pub fn parse_controller_event_channel_config_from_attr(
    attr: &Attribute,
) -> Result<ControllerEventChannelConfig, TokenStream> {
    if let Meta::List(meta_list) = &attr.meta {
        parse_controller_event_channel_config(meta_list.tokens.clone())
    } else {
        Ok(ControllerEventChannelConfig {
            channel_size: None,
            subs: None,
            pubs: None,
        })
    }
}
