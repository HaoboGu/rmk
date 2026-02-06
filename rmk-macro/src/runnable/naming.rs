//! Event type naming utilities for Runnable generation.
//!
//! Converts event type names to method names and variant names.

use quote::format_ident;
use syn::Path;

use crate::utils::to_snake_case;

/// Convert event type to read method name (BatteryEvent -> read_battery_event).
pub fn event_type_to_read_method_name(path: &Path) -> syn::Ident {
    let type_name = path.segments.last().unwrap().ident.to_string();

    // Strip "Event" suffix if present.
    let base_name = type_name.strip_suffix("Event").unwrap_or(&type_name);

    // CamelCase -> snake_case.
    let snake_case = to_snake_case(base_name);

    // Add "read_" prefix and "_event" suffix.
    format_ident!("read_{}_event", snake_case)
}

/// Convert event type to handler method name (BatteryEvent -> on_battery_event).
pub fn event_type_to_handler_method_name(path: &Path) -> syn::Ident {
    let type_name = path.segments.last().unwrap().ident.to_string();

    // Strip "Event" suffix if present.
    let base_name = type_name.strip_suffix("Event").unwrap_or(&type_name);

    // CamelCase -> snake_case.
    let snake_case = to_snake_case(base_name);

    // Add "on_" prefix and "_event" suffix.
    format_ident!("on_{}_event", snake_case)
}

/// Convert event type to enum variant name (BatteryEvent -> Battery).
///
/// Strips "Event" suffix and returns the base name as the variant.
pub fn event_type_to_variant_name(path: &Path) -> syn::Ident {
    let type_name = path.segments.last().unwrap().ident.to_string();

    // Strip "Event" suffix if present.
    let base_name = type_name.strip_suffix("Event").unwrap_or(&type_name);

    format_ident!("{}", base_name)
}

/// Generate unique variant names from event types, handling collisions.
///
/// If two event types would produce the same variant name (e.g., FooEvent and FooData
/// both mapping to "Foo"), append numeric suffixes to disambiguate.
pub fn generate_unique_variant_names(event_types: &[Path]) -> Vec<syn::Ident> {
    use std::collections::HashMap;

    let base_names: Vec<syn::Ident> = event_types.iter().map(event_type_to_variant_name).collect();

    // Count occurrences of each name
    let mut counts: HashMap<String, usize> = HashMap::new();
    for name in &base_names {
        *counts.entry(name.to_string()).or_insert(0) += 1;
    }

    // Generate unique names with suffixes for duplicates
    let mut seen: HashMap<String, usize> = HashMap::new();
    base_names
        .iter()
        .map(|name| {
            let name_str = name.to_string();
            if counts[&name_str] > 1 {
                let idx = seen.entry(name_str.clone()).or_insert(0);
                *idx += 1;
                format_ident!("{}{}", name_str, idx)
            } else {
                name.clone()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_event_type_to_read_method_name() {
        let path: Path = parse_quote!(BatteryEvent);
        assert_eq!(event_type_to_read_method_name(&path).to_string(), "read_battery_event");

        let path: Path = parse_quote!(ChargingStateEvent);
        assert_eq!(
            event_type_to_read_method_name(&path).to_string(),
            "read_charging_state_event"
        );
    }

    #[test]
    fn test_event_type_to_variant_name() {
        let path: Path = parse_quote!(BatteryEvent);
        assert_eq!(event_type_to_variant_name(&path).to_string(), "Battery");

        let path: Path = parse_quote!(ChargingStateEvent);
        assert_eq!(event_type_to_variant_name(&path).to_string(), "ChargingState");

        // Without Event suffix
        let path: Path = parse_quote!(KeyPress);
        assert_eq!(event_type_to_variant_name(&path).to_string(), "KeyPress");
    }

    #[test]
    fn test_generate_unique_variant_names() {
        // No collisions
        let paths: Vec<Path> = vec![parse_quote!(BatteryEvent), parse_quote!(ChargingEvent)];
        let names = generate_unique_variant_names(&paths);
        assert_eq!(names[0].to_string(), "Battery");
        assert_eq!(names[1].to_string(), "Charging");

        // With collision (both map to same base name)
        let paths: Vec<Path> = vec![
            parse_quote!(FooEvent),
            parse_quote!(FooEvent), // Same type twice
        ];
        let names = generate_unique_variant_names(&paths);
        assert_eq!(names[0].to_string(), "Foo1");
        assert_eq!(names[1].to_string(), "Foo2");
    }
}
