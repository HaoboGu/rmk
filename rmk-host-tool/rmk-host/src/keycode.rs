//! Friendly parser for `KeyAction` strings in the CLI.
//!
//! Accepts a small set of common forms:
//! * `no` → `KeyAction::No`
//! * `trans` / `transparent` → `KeyAction::Transparent`
//! * a single ASCII letter `a`-`z` / `A`-`Z` → corresponding HID letter
//! * a single digit `0`-`9` → corresponding HID digit
//! * `kc_<NAME>` (case-insensitive) → matching HID variant for a curated list
//!   of common keys (letters, digits, modifiers, navigation, F-keys, etc.)
//!
//! Anything outside this set returns `Err`. The CLI is a development tool, not
//! a full keymap editor — bumping the alias table below is the way to extend it.

use rmk_types::action::{Action, KeyAction};
use rmk_types::keycode::{HidKeyCode, KeyCode};

/// Parse a CLI-friendly key-action string into a `KeyAction`.
pub fn parse_key_action(input: &str) -> Result<KeyAction, String> {
    let s = input.trim();
    let lower = s.to_ascii_lowercase();

    if lower == "no" || lower == "kc_no" {
        return Ok(KeyAction::No);
    }
    if lower == "trans" || lower == "transparent" || lower == "kc_trns" {
        return Ok(KeyAction::Transparent);
    }
    // Single-char alpha shortcut.
    if s.len() == 1 {
        if let Some(hid) = ascii_to_hid(s.chars().next().unwrap()) {
            return Ok(KeyAction::Single(Action::Key(KeyCode::Hid(hid))));
        }
    }
    // `kc_<name>` form.
    if let Some(rest) = lower.strip_prefix("kc_") {
        if let Some(hid) = name_to_hid(rest) {
            return Ok(KeyAction::Single(Action::Key(KeyCode::Hid(hid))));
        }
    }
    Err(format!(
        "unrecognized key-action `{input}`. Accepted: `no`, `trans`, single letter/digit, `KC_<name>`."
    ))
}

fn ascii_to_hid(c: char) -> Option<HidKeyCode> {
    match c {
        'a'..='z' => name_to_hid(&c.to_string()),
        'A'..='Z' => name_to_hid(&c.to_ascii_lowercase().to_string()),
        '0' => Some(HidKeyCode::Kc0),
        '1' => Some(HidKeyCode::Kc1),
        '2' => Some(HidKeyCode::Kc2),
        '3' => Some(HidKeyCode::Kc3),
        '4' => Some(HidKeyCode::Kc4),
        '5' => Some(HidKeyCode::Kc5),
        '6' => Some(HidKeyCode::Kc6),
        '7' => Some(HidKeyCode::Kc7),
        '8' => Some(HidKeyCode::Kc8),
        '9' => Some(HidKeyCode::Kc9),
        _ => None,
    }
}

fn name_to_hid(name: &str) -> Option<HidKeyCode> {
    Some(match name {
        "a" => HidKeyCode::A,
        "b" => HidKeyCode::B,
        "c" => HidKeyCode::C,
        "d" => HidKeyCode::D,
        "e" => HidKeyCode::E,
        "f" => HidKeyCode::F,
        "g" => HidKeyCode::G,
        "h" => HidKeyCode::H,
        "i" => HidKeyCode::I,
        "j" => HidKeyCode::J,
        "k" => HidKeyCode::K,
        "l" => HidKeyCode::L,
        "m" => HidKeyCode::M,
        "n" => HidKeyCode::N,
        "o" => HidKeyCode::O,
        "p" => HidKeyCode::P,
        "q" => HidKeyCode::Q,
        "r" => HidKeyCode::R,
        "s" => HidKeyCode::S,
        "t" => HidKeyCode::T,
        "u" => HidKeyCode::U,
        "v" => HidKeyCode::V,
        "w" => HidKeyCode::W,
        "x" => HidKeyCode::X,
        "y" => HidKeyCode::Y,
        "z" => HidKeyCode::Z,
        "0" => HidKeyCode::Kc0,
        "1" => HidKeyCode::Kc1,
        "2" => HidKeyCode::Kc2,
        "3" => HidKeyCode::Kc3,
        "4" => HidKeyCode::Kc4,
        "5" => HidKeyCode::Kc5,
        "6" => HidKeyCode::Kc6,
        "7" => HidKeyCode::Kc7,
        "8" => HidKeyCode::Kc8,
        "9" => HidKeyCode::Kc9,
        "enter" | "ent" | "return" => HidKeyCode::Enter,
        "esc" | "escape" => HidKeyCode::Escape,
        "bspc" | "backspace" => HidKeyCode::Backspace,
        "tab" => HidKeyCode::Tab,
        "spc" | "space" => HidKeyCode::Space,
        "minus" | "mins" => HidKeyCode::Minus,
        "equal" | "eql" => HidKeyCode::Equal,
        "lbrc" | "lbracket" => HidKeyCode::LeftBracket,
        "rbrc" | "rbracket" => HidKeyCode::RightBracket,
        "bsls" | "backslash" => HidKeyCode::Backslash,
        "scln" | "semicolon" => HidKeyCode::Semicolon,
        "quot" | "quote" => HidKeyCode::Quote,
        "grv" | "grave" => HidKeyCode::Grave,
        "comm" | "comma" => HidKeyCode::Comma,
        "dot" | "period" => HidKeyCode::Dot,
        "slsh" | "slash" => HidKeyCode::Slash,
        "caps" | "capslock" => HidKeyCode::CapsLock,
        "f1" => HidKeyCode::F1,
        "f2" => HidKeyCode::F2,
        "f3" => HidKeyCode::F3,
        "f4" => HidKeyCode::F4,
        "f5" => HidKeyCode::F5,
        "f6" => HidKeyCode::F6,
        "f7" => HidKeyCode::F7,
        "f8" => HidKeyCode::F8,
        "f9" => HidKeyCode::F9,
        "f10" => HidKeyCode::F10,
        "f11" => HidKeyCode::F11,
        "f12" => HidKeyCode::F12,
        "left" => HidKeyCode::Left,
        "right" => HidKeyCode::Right,
        "up" => HidKeyCode::Up,
        "down" => HidKeyCode::Down,
        "home" => HidKeyCode::Home,
        "end" => HidKeyCode::End,
        "pgup" | "pageup" => HidKeyCode::PageUp,
        "pgdn" | "pagedown" => HidKeyCode::PageDown,
        "ins" | "insert" => HidKeyCode::Insert,
        "del" | "delete" => HidKeyCode::Delete,
        "lctl" | "lctrl" => HidKeyCode::LCtrl,
        "lsft" | "lshift" => HidKeyCode::LShift,
        "lalt" => HidKeyCode::LAlt,
        "lgui" | "lwin" | "lcmd" => HidKeyCode::LGui,
        "rctl" | "rctrl" => HidKeyCode::RCtrl,
        "rsft" | "rshift" => HidKeyCode::RShift,
        "ralt" => HidKeyCode::RAlt,
        "rgui" | "rwin" | "rcmd" => HidKeyCode::RGui,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_no_and_trans() {
        assert!(matches!(parse_key_action("no").unwrap(), KeyAction::No));
        assert!(matches!(parse_key_action("trans").unwrap(), KeyAction::Transparent));
    }

    #[test]
    fn parses_single_letter_and_kc_form() {
        assert!(matches!(
            parse_key_action("a").unwrap(),
            KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::A)))
        ));
        assert!(matches!(
            parse_key_action("KC_A").unwrap(),
            KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::A)))
        ));
        assert!(matches!(
            parse_key_action("kc_lshift").unwrap(),
            KeyAction::Single(Action::Key(KeyCode::Hid(HidKeyCode::LShift)))
        ));
    }

    #[test]
    fn rejects_unknown_keycode() {
        assert!(parse_key_action("KC_FOOBAR").is_err());
    }
}
