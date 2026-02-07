//! Attribute parsers for input device and processor macros.

use proc_macro2::TokenStream;
use syn::{Attribute, Meta};

use super::config::{InputDeviceConfig, InputProcessorConfig};
use crate::utils::AttributeParser;

/// Parse input_device config from attribute tokens.
/// Extracts `publish = EventType`.
pub fn parse_input_device_config(tokens: impl Into<TokenStream>) -> Option<InputDeviceConfig> {
    let parser = AttributeParser::new(tokens).ok()?;
    parser
        .get_path("publish")
        .map(|event_type| InputDeviceConfig { event_type })
}

/// Parse input_processor config from attribute tokens.
/// Extracts `subscribe = [...]`.
pub fn parse_input_processor_config(tokens: impl Into<TokenStream>) -> InputProcessorConfig {
    let parser = AttributeParser::new(tokens).unwrap_or_else(|_| AttributeParser::empty());

    InputProcessorConfig {
        event_types: parser.get_path_array("subscribe"),
    }
}

/// Parse input_event channel_size from a TokenStream.
pub fn parse_input_event_channel_size(tokens: impl Into<TokenStream>) -> Option<TokenStream> {
    let parser = AttributeParser::new(tokens).ok()?;
    parser.get_expr_tokens("channel_size")
}

/// Parse input_event channel_size from an Attribute.
pub fn parse_input_event_channel_size_from_attr(attr: &Attribute) -> Option<TokenStream> {
    if let Meta::List(meta_list) = &attr.meta {
        parse_input_event_channel_size(meta_list.tokens.clone())
    } else {
        None
    }
}
