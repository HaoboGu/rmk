//! Shared utility functions for rmk-macro.
//!
//! Contains only cross-domain utilities used by both codegen and event_macros.

/// Internal case conversion function.
fn convert_case_internal(s: &str, to_upper: bool) -> String {
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();

    for i in 0..chars.len() {
        let c = chars[i];

        if c.is_uppercase() {
            // Add underscore before uppercase when:
            // 1) not at start
            // 2) previous is lowercase
            // 3) next is lowercase (acronym end)
            let add_underscore = i > 0
                && (chars[i - 1].is_lowercase()
                    || (i + 1 < chars.len() && chars[i + 1].is_lowercase()));

            if add_underscore {
                result.push('_');
            }
            result.push(if to_upper { c } else { c.to_ascii_lowercase() });
        } else {
            result.push(if to_upper { c.to_ascii_uppercase() } else { c });
        }
    }

    result
}

/// Convert CamelCase to snake_case.
pub fn to_snake_case(s: &str) -> String {
    convert_case_internal(s, false)
}

/// Convert CamelCase to UPPER_SNAKE_CASE.
pub fn to_upper_snake_case(s: &str) -> String {
    convert_case_internal(s, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("Battery"), "battery");
        assert_eq!(to_snake_case("ChargingState"), "charging_state");
        assert_eq!(to_snake_case("USB"), "usb");
        assert_eq!(to_snake_case("USBKey"), "usb_key");
        assert_eq!(to_snake_case("HTMLParser"), "html_parser");
    }

    #[test]
    fn test_to_upper_snake_case() {
        assert_eq!(to_upper_snake_case("KeyEvent"), "KEY_EVENT");
        assert_eq!(to_upper_snake_case("ModifierEvent"), "MODIFIER_EVENT");
        assert_eq!(to_upper_snake_case("TouchpadEvent"), "TOUCHPAD_EVENT");
        assert_eq!(to_upper_snake_case("USBEvent"), "USB_EVENT");
        assert_eq!(to_upper_snake_case("HIDDevice"), "HID_DEVICE");
    }
}
