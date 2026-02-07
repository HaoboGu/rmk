//! Attribute parsers for controller macros.

use proc_macro2::TokenStream;
use syn::{Attribute, Meta};

use super::config::{ControllerConfig, ControllerEventChannelConfig};
use crate::utils::AttributeParser;

/// Parse controller config from attribute tokens.
/// Extracts `subscribe = [...]` and optional `poll_interval = N`.
pub fn parse_controller_config(tokens: impl Into<TokenStream>) -> ControllerConfig {
    let parser = AttributeParser::new(tokens).unwrap_or_else(|_| AttributeParser::empty());

    ControllerConfig {
        event_types: parser.get_path_array("subscribe"),
        poll_interval_ms: parser.get_int("poll_interval"),
    }
}

/// Parse controller_event parameters from a TokenStream.
/// Extracts `channel_size`, `subs`, `pubs`.
pub fn parse_controller_event_channel_config(
    tokens: impl Into<TokenStream>,
) -> ControllerEventChannelConfig {
    let parser = AttributeParser::new(tokens).unwrap_or_else(|_| AttributeParser::empty());

    ControllerEventChannelConfig {
        channel_size: parser.get_expr_tokens("channel_size"),
        subs: parser.get_expr_tokens("subs"),
        pubs: parser.get_expr_tokens("pubs"),
    }
}

/// Parse controller_event parameters from an Attribute.
pub fn parse_controller_event_channel_config_from_attr(
    attr: &Attribute,
) -> ControllerEventChannelConfig {
    if let Meta::List(meta_list) = &attr.meta {
        parse_controller_event_channel_config(meta_list.tokens.clone())
    } else {
        ControllerEventChannelConfig {
            channel_size: None,
            subs: None,
            pubs: None,
        }
    }
}
