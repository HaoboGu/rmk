use std::collections::HashMap;

use pest::Parser;
use pest_derive::Parser;

use crate::layout::{MapToken, parse_map};
use crate::{KeyInfo, KeyboardTomlConfig, KeymapConfig, LayerTomlConfig};

// Pest parser using the grammar files
#[derive(Parser)]
#[grammar = "keymap.pest"]
pub(crate) struct ConfigParser;

// Max alias resolution depth to prevent infinite loops
const MAX_ALIAS_RESOLUTION_DEPTH: usize = 10;

impl KeyboardTomlConfig {
    /// Resolve `[keymap]` into a dense per-layer action grid plus per-key info.
    ///
    /// `[layout].map` fixes the write-order of keys; each `[[keymap.layer]]` lists its
    /// actions in that order, which we scatter back onto the `rows × cols` grid. Layers
    /// beyond those defined are padded transparent up to `[keymap].layers`.
    pub(crate) fn get_keymap_config(&self) -> Result<(KeymapConfig, Vec<Vec<KeyInfo>>), String> {
        let aliases = self.aliases.clone().unwrap_or_default();
        let keymap_cfg = self.keymap.clone().expect("keymap config is required");
        let layout = self.layout.clone().expect("layout config is required");
        let layers = &keymap_cfg.layer;

        // Aliases are referenced as `@name`, so a name with whitespace can't be matched.
        for key in aliases.keys() {
            if key.chars().any(char::is_whitespace) {
                return Err(format!(
                    "keyboard.toml: Alias key '{}' must not contain whitespace characters",
                    key
                ));
            }
        }

        // `layers` defaults to the number of `[[keymap.layer]]` blocks; an explicit value may
        // only *reserve extra* empty layers, never fewer than are defined.
        let num_layers = match keymap_cfg.layers {
            Some(n) if (layers.len() as u8) > n => {
                return Err(format!(
                    "keyboard.toml: {} [[keymap.layer]] entries exceed [keymap].layers = {}",
                    layers.len(),
                    n
                ));
            }
            Some(n) => n,
            None => layers.len() as u8,
        };

        // `[layout].map` is the sole source for placing keys, so it's required whenever a
        // layer is defined; with no layers the default keymap is simply transparent.
        let mut keymap = Vec::with_capacity(num_layers as usize);
        let key_info = match layout.map.as_deref() {
            Some(map) => {
                let (sequence_to_grid, key_info) = Self::build_key_grid(map, layout.rows, layout.cols)?;
                let layer_names = Self::collect_layer_names(layers)?;
                for layer in layers {
                    keymap.push(Self::build_layer_grid(
                        layer,
                        &sequence_to_grid,
                        &aliases,
                        &layer_names,
                        layout.rows,
                        layout.cols,
                    )?);
                }
                key_info
            }
            None if layers.is_empty() => {
                vec![vec![KeyInfo::default(); layout.cols as usize]; layout.rows as usize]
            }
            None => {
                return Err("keyboard.toml: `[layout].map` is required to place `[[keymap.layer]]` keys".to_string());
            }
        };

        // Pad undefined layers with transparent keys up to the configured count.
        for _ in keymap.len()..num_layers as usize {
            keymap.push(vec![vec!["_".to_string(); layout.cols as usize]; layout.rows as usize]);
        }

        let encoder_map = Self::resolve_encoders(layers, &aliases)?;

        Ok((
            KeymapConfig {
                rows: layout.rows,
                cols: layout.cols,
                layers: num_layers,
                keymap,
                encoder_map,
            },
            key_info,
        ))
    }

    /// Build the write-order → grid-coordinate sequence and per-key info (hand) from
    /// `[layout].map`. `parse_map` (shared with the geometry blob) already bounds-checks
    /// and de-dupes the coordinates, so here we just keep the `Key` tokens in order.
    fn build_key_grid(map: &str, rows: u8, cols: u8) -> Result<(Vec<(u8, u8)>, Vec<Vec<KeyInfo>>), String> {
        let mut key_info = vec![vec![KeyInfo::default(); cols as usize]; rows as usize];
        let mut sequence_to_grid = Vec::new();
        for token in parse_map(map, rows, cols)? {
            if let MapToken::Key { row, col, hand, .. } = token {
                key_info[row as usize][col as usize] = KeyInfo { hand };
                sequence_to_grid.push((row, col));
            }
        }
        Ok((sequence_to_grid, key_info))
    }

    /// Map each named layer to its index, rejecting duplicate names.
    fn collect_layer_names(layers: &[LayerTomlConfig]) -> Result<HashMap<String, u32>, String> {
        let mut layer_names = HashMap::new();
        for (index, layer) in layers.iter().enumerate() {
            if let Some(name) = &layer.name {
                if layer_names.insert(name.clone(), index as u32).is_some() {
                    return Err(format!(
                        "keyboard.toml: Duplicate layer name '{}' found in `[[keymap.layer]]`",
                        name
                    ));
                }
            }
        }
        Ok(layer_names)
    }

    /// Scatter one layer's alias-resolved key sequence onto a `rows × cols` grid in the
    /// order fixed by `[layout].map`. Positions without a key stay `"No"`.
    fn build_layer_grid(
        layer: &LayerTomlConfig,
        sequence_to_grid: &[(u8, u8)],
        aliases: &HashMap<String, String>,
        layer_names: &HashMap<String, u32>,
        rows: u8,
        cols: u8,
    ) -> Result<Vec<Vec<String>>, String> {
        let key_actions = Self::keymap_parser(&layer.keys, aliases, layer_names)
            .map_err(|e| format!("keyboard.toml: Error in `[[keymap.layer]]`: {}", e))?;
        if key_actions.len() > sequence_to_grid.len() {
            return Err(format!(
                "keyboard.toml: layer '{}' has {} keys but `layout.map` defines {} positions",
                layer.name.clone().unwrap_or_default(),
                key_actions.len(),
                sequence_to_grid.len()
            ));
        }

        let mut grid = vec![vec!["No".to_string(); cols as usize]; rows as usize];
        for ((row, col), action) in sequence_to_grid.iter().zip(key_actions) {
            grid[*row as usize][*col as usize] = action;
        }
        Ok(grid)
    }

    /// Resolve `[[keymap.layer]].encoders` (alias-expanded), one entry per layer.
    fn resolve_encoders(
        layers: &[LayerTomlConfig],
        aliases: &HashMap<String, String>,
    ) -> Result<Vec<Vec<[String; 2]>>, String> {
        let mut encoder_map = Vec::with_capacity(layers.len());
        for layer in layers {
            let mut encoders = layer.encoders.clone().unwrap_or_default();
            for [cw, ccw] in &mut encoders {
                *cw = Self::alias_resolver(cw, aliases)?;
                *ccw = Self::alias_resolver(ccw, aliases)?;
            }
            encoder_map.push(encoders);
        }
        Ok(encoder_map)
    }

    fn alias_resolver(keys: &str, aliases: &HashMap<String, String>) -> Result<String, String> {
        let mut current_keys = keys.to_string();

        let mut iterations = 0;

        loop {
            let mut next_keys = String::with_capacity(current_keys.capacity());
            let mut made_replacement = false;
            let mut last_index = 0; // Keep track of where we are in current_keys

            while let Some(at_index) = current_keys[last_index..].find('@') {
                let start_index = last_index + at_index;

                // Append the text before the '@'
                next_keys.push_str(&current_keys[last_index..start_index]);

                // Check if it's a valid alias start (@ followed by a non whitespace)
                if let Some(first_char) = current_keys.as_bytes().get(start_index + 1) {
                    if !first_char.is_ascii_whitespace() {
                        // Find the end of the alias identifier
                        let mut end_index = start_index + 2;
                        while let Some(c) = current_keys.as_bytes().get(end_index) {
                            if c.is_ascii_whitespace() {
                                break;
                            } else {
                                end_index += 1;
                            }
                        }

                        // Extract the alias key (except the starting '@')
                        let alias_key = &current_keys[start_index + 1..end_index];

                        // Look up and replace
                        match aliases.get(alias_key) {
                            Some(value) => {
                                next_keys.push_str(value);
                                made_replacement = true;
                            }
                            None => return Err(format!("Undefined alias: {}", alias_key)),
                        }
                        last_index = end_index; // Move past the processed alias
                    } else {
                        // Not a valid alias start, treat '@' literally
                        next_keys.push('@');
                        last_index = start_index + 1;
                    }
                } else {
                    // '@' was the last character, treat it literally
                    next_keys.push('@');
                    last_index = start_index + 1;
                    break; // No more characters after '@'
                }
            }

            // Append any remaining part of the string after the last '@' or if no '@' was found
            next_keys.push_str(&current_keys[last_index..]);

            // Check for termination conditions
            iterations += 1;
            if iterations >= MAX_ALIAS_RESOLUTION_DEPTH {
                return Err(format!(
                    "Alias resolution exceeded maximum depth ({}), potential infinite loop detected in '{}'",
                    MAX_ALIAS_RESOLUTION_DEPTH, keys
                )); // Show original keys for context
            }

            if !made_replacement {
                break; // No more replacements needed
            }

            // Prepare for the next iteration
            current_keys = next_keys;
        }

        Ok(current_keys)
    }

    /// Reconstruct an action string from a parsed pair, resolving every named
    /// layer reference (`MO(base)`) to its numeric index (`MO(0)`).
    ///
    /// Layer names may appear at any nesting depth (e.g. inside the tap slot of
    /// `TH(MO(nav), A)`), so this walks the whole subtree, collects the source
    /// span of each `layer_name`, and rewrites those spans in place. Actions
    /// without layer names are returned verbatim.
    fn resolve_layer_names(
        pair: &pest::iterators::Pair<Rule>,
        layer_names: &HashMap<String, u32>,
    ) -> Result<String, String> {
        let base = pair.as_span().start();
        let mut replacements: Vec<(usize, usize, String)> = Vec::new();
        Self::collect_layer_name_spans(pair.clone(), layer_names, &mut replacements)?;

        // Apply right-to-left so earlier byte offsets stay valid.
        replacements.sort_by_key(|(start, _, _)| *start);
        let mut result = pair.as_str().to_string();
        for (start, end, replacement) in replacements.into_iter().rev() {
            result.replace_range(start - base..end - base, &replacement);
        }
        Ok(result)
    }

    /// Recursively collect `(start, end, resolved_number)` for every `layer_name`
    /// in the subtree, validating each against the known layer names.
    fn collect_layer_name_spans(
        pair: pest::iterators::Pair<Rule>,
        layer_names: &HashMap<String, u32>,
        out: &mut Vec<(usize, usize, String)>,
    ) -> Result<(), String> {
        if pair.as_rule() == Rule::layer_name {
            let layer_name = pair.as_str();
            match layer_names.get(layer_name) {
                Some(layer_number) => {
                    let span = pair.as_span();
                    out.push((span.start(), span.end(), layer_number.to_string()));
                }
                None => return Err(format!("Invalid layer name: {}", layer_name)),
            }
            return Ok(());
        }
        for inner in pair.into_inner() {
            Self::collect_layer_name_spans(inner, layer_names, out)?;
        }
        Ok(())
    }

    fn keymap_parser(
        layer_keys: &str,
        aliases: &HashMap<String, String>,
        layer_names: &HashMap<String, u32>,
    ) -> Result<Vec<String>, String> {
        //resolve aliases first
        let layer_keys = Self::alias_resolver(layer_keys, aliases)?;

        let mut key_action_sequence = Vec::new();

        // Parse the keymap using Pest
        match ConfigParser::parse(Rule::key_map, &layer_keys) {
            Ok(pairs) => {
                // The top-level pair is 'key_map'. We need to iterate its inner content.
                for pair in pairs {
                    // Should only be one pair matching Rule::key_map
                    if pair.as_rule() == Rule::key_map {
                        for inner_pair in pair.into_inner() {
                            match inner_pair.as_rule() {
                                Rule::EOI | Rule::WHITESPACE => {
                                    // Ignore End of input marker
                                }
                                // Every key action is forwarded as its (alias-resolved) source
                                // text, with any named layer references resolved to indices.
                                // This handles layer names nested at any depth, e.g. the tap
                                // slot of `TH(MO(nav), A)`.
                                _ => {
                                    key_action_sequence.push(Self::resolve_layer_names(&inner_pair, layer_names)?);
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                return Err(format!("Invalid keymap format: {}", e));
            }
        }

        Ok(key_action_sequence)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_action_parsing() {
        // Test "No" followed by whitespace
        let test_cases = vec![
            ("No ", vec!["No"]),
            ("No\n", vec!["No"]),
            ("No\t", vec!["No"]),
            ("No  A", vec!["No", "A"]),
            ("A No B", vec!["A", "No", "B"]),
            ("No No No", vec!["No", "No", "No"]),
        ];

        for (input, expected) in test_cases {
            let result = ConfigParser::parse(Rule::key_map, input);
            assert!(result.is_ok(), "Failed to parse: {}", input);

            let mut actions = Vec::new();
            for pair in result.unwrap() {
                if pair.as_rule() == Rule::key_map {
                    for inner_pair in pair.into_inner() {
                        match inner_pair.as_rule() {
                            Rule::no_action | Rule::simple_keycode => {
                                actions.push(inner_pair.as_str().to_string());
                            }
                            Rule::EOI | Rule::WHITESPACE => {}
                            _ => {}
                        }
                    }
                }
            }

            assert_eq!(actions, expected, "Input: {}", input);
        }
    }

    #[test]
    fn test_no_vs_no_prefixed_keycodes() {
        // Test that "No" is parsed as no_action but "NoUsSlash" is parsed as simple_keycode
        let test_cases = vec![
            ("No", Rule::no_action),
            ("NoUsSlash", Rule::simple_keycode),
            ("NonUsSlash", Rule::simple_keycode),
            ("NoReturn", Rule::simple_keycode),
            ("NoBrake", Rule::simple_keycode),
        ];

        for (input, expected_rule) in test_cases {
            let result = ConfigParser::parse(Rule::key_map, input);
            assert!(result.is_ok(), "Failed to parse: {}", input);

            let mut found_rule = None;
            for pair in result.unwrap() {
                if pair.as_rule() == Rule::key_map {
                    for inner_pair in pair.into_inner() {
                        match inner_pair.as_rule() {
                            Rule::no_action | Rule::simple_keycode => {
                                found_rule = Some(inner_pair.as_rule());
                            }
                            _ => {}
                        }
                    }
                }
            }

            assert_eq!(
                found_rule,
                Some(expected_rule),
                "Input: {} should be parsed as {:?}",
                input,
                expected_rule
            );
        }
    }

    #[test]
    fn test_keymap_parser_with_no_actions() {
        let aliases = HashMap::new();
        let layer_names = HashMap::new();

        // Test parsing a keymap string with "No" actions
        let keymap = "A B No C No NoUsSlash NonUsSlash D No";
        let result = KeyboardTomlConfig::keymap_parser(keymap, &aliases, &layer_names);

        assert!(result.is_ok());
        let actions = result.unwrap();
        assert_eq!(
            actions,
            vec!["A", "B", "No", "C", "No", "NoUsSlash", "NonUsSlash", "D", "No"]
        );
    }

    #[test]
    fn test_keymap_parser_with_comma_alias() {
        let aliases = HashMap::new();
        let layer_names = HashMap::new();

        let keymap = "A , SHIFTED(,) B";
        let result = KeyboardTomlConfig::keymap_parser(keymap, &aliases, &layer_names);

        assert!(result.is_ok());
        let actions = result.unwrap();
        assert_eq!(actions, vec!["A", ",", "SHIFTED(,)", "B"]);
    }

    #[test]
    fn test_comma_separator_compatibility_in_multi_arg_actions() {
        let aliases = HashMap::new();
        let layer_names = HashMap::new();

        // Comma keeps working as argument separator in multi-argument actions.
        let keymap = "TH(A, B) TH(Comma, B) TH(A, Comma)";
        let result = KeyboardTomlConfig::keymap_parser(keymap, &aliases, &layer_names);

        assert!(result.is_ok());
        let actions = result.unwrap();
        assert_eq!(actions, vec!["TH(A, B)", "TH(Comma, B)", "TH(A, Comma)"]);
    }

    #[test]
    fn test_multi_arg_actions_reject_symbol_comma_as_key_argument() {
        let invalid_cases = ["TH(A, ,)", "TH(, ,)", "WM(, LShift)", "LT(1, ,)", "MT(, LShift)"];

        for input in invalid_cases {
            let result = ConfigParser::parse(Rule::key_map, input);
            assert!(result.is_err(), "Input should be rejected: {}", input);
        }
    }

    #[test]
    fn malformed_layer_returns_err_not_panic() {
        // A keymap the grammar can't parse returns Err rather than panicking the
        // build (keymap_parser used to `panic!` on the pest error).
        let aliases = HashMap::new();
        let layer_names = HashMap::new();
        let result = KeyboardTomlConfig::keymap_parser("TH(A, ,)", &aliases, &layer_names);
        assert!(result.is_err(), "unparseable keymap must be Err, not a panic");
    }

    #[test]
    fn test_single_key_arg_actions_accept_symbol_comma() {
        let valid_cases = ["SHIFTED(,)", "SHIFTED(Comma)", ","];

        for input in valid_cases {
            let result = ConfigParser::parse(Rule::key_map, input);
            assert!(result.is_ok(), "Input should be accepted: {}", input);
        }
    }

    #[test]
    fn test_morse_action_parsing() {
        let aliases = HashMap::new();
        let layer_names = HashMap::new();

        // Test parsing a keymap string with TD actions
        let keymap = "A TD(0) B TD(1) C TD(255)";
        let result = KeyboardTomlConfig::keymap_parser(keymap, &aliases, &layer_names);

        assert!(result.is_ok());
        let actions = result.unwrap();
        assert_eq!(actions, vec!["A", "TD(0)", "B", "TD(1)", "C", "TD(255)"]);
    }

    #[test]
    fn test_macro_trigger_action_parsing() {
        let aliases = std::collections::HashMap::new();
        let layer_names = std::collections::HashMap::new();

        // Test parsing a keymap string with macro trigger actions
        let keymap = "A Macro(0) B MACRO(1) C macro(255)";
        let result = KeyboardTomlConfig::keymap_parser(keymap, &aliases, &layer_names);

        assert!(result.is_ok());
        let actions = result.unwrap();
        assert_eq!(actions, vec!["A", "Macro(0)", "B", "MACRO(1)", "C", "macro(255)"]);
    }

    #[test]
    fn test_morse_action_grammar() {
        // Test that TD actions are parsed correctly by the grammar
        let test_cases = vec![
            ("TD(0)", Rule::morse_action),
            ("TD(1)", Rule::morse_action),
            ("TD(255)", Rule::morse_action),
            ("td(0)", Rule::morse_action), // Case insensitive
            ("td(1)", Rule::morse_action),
            ("MORSE(0)", Rule::morse_action),
            ("MORSE(1)", Rule::morse_action),
            ("MORSE(255)", Rule::morse_action),
            ("Morse(0)", Rule::morse_action), // Case insensitive
            ("morse(1)", Rule::morse_action),
        ];

        for (input, expected_rule) in test_cases {
            let result = ConfigParser::parse(Rule::key_map, input);
            assert!(result.is_ok(), "Failed to parse: {}", input);

            let mut found_rule = None;
            for pair in result.unwrap() {
                if pair.as_rule() == Rule::key_map {
                    for inner_pair in pair.into_inner() {
                        match inner_pair.as_rule() {
                            Rule::morse_action => {
                                found_rule = Some(inner_pair.as_rule());
                            }
                            _ => {}
                        }
                    }
                }
            }

            assert_eq!(
                found_rule,
                Some(expected_rule),
                "Input: {} should be parsed as {:?}",
                input,
                expected_rule
            );
        }
    }

    #[test]
    fn test_macro_grammar() {
        // Test that macro actions are parsed correctly by the grammar
        let test_cases = vec![
            ("Macro(0)", Rule::trigger_macro_action),
            ("Macro(1)", Rule::trigger_macro_action),
            ("Macro(255)", Rule::trigger_macro_action),
            ("MACRO(0)", Rule::trigger_macro_action), // Case insensitive
            ("MACRO(1)", Rule::trigger_macro_action),
            ("macro(0)", Rule::trigger_macro_action), // Case insensitive
            ("macro(1)", Rule::trigger_macro_action),
            ("macro(255)", Rule::trigger_macro_action),
        ];

        for (input, expected_rule) in test_cases {
            let result = ConfigParser::parse(Rule::key_map, input);
            assert!(result.is_ok(), "Failed to parse: {}", input);

            let mut found_rule = None;
            for pair in result.unwrap() {
                if pair.as_rule() == Rule::key_map {
                    for inner_pair in pair.into_inner() {
                        match inner_pair.as_rule() {
                            Rule::trigger_macro_action => {
                                found_rule = Some(inner_pair.as_rule());
                            }
                            _ => {}
                        }
                    }
                }
            }

            assert_eq!(
                found_rule,
                Some(expected_rule),
                "Input: {} should be parsed as {:?}",
                input,
                expected_rule
            );
        }
    }

    #[test]
    fn test_nested_actions_in_tap_hold_slots() {
        let aliases = HashMap::new();
        let layer_names = HashMap::new();

        // A single-action form (here WM) can appear in the tap/hold slots of
        // MT/TH/LT and is forwarded verbatim for the proc-macro to expand.
        let keymap = "MT(WM(P, RAlt), LShift, HRM) TH(WM(A, LShift), MO(2)) LT(1, WM(Q, LGui))";
        let result = KeyboardTomlConfig::keymap_parser(keymap, &aliases, &layer_names);

        assert!(result.is_ok(), "{:?}", result);
        assert_eq!(
            result.unwrap(),
            vec![
                "MT(WM(P, RAlt), LShift, HRM)",
                "TH(WM(A, LShift), MO(2))",
                "LT(1, WM(Q, LGui))",
            ]
        );
    }

    #[test]
    fn test_layer_name_resolution_nested() {
        let aliases = HashMap::new();
        let mut layer_names = HashMap::new();
        layer_names.insert("nav".to_string(), 3u32);

        // Layer names are resolved to indices even when nested inside a slot.
        let keymap = "MO(nav) TH(A, MO(nav))";
        let result = KeyboardTomlConfig::keymap_parser(keymap, &aliases, &layer_names);

        assert!(result.is_ok(), "{:?}", result);
        assert_eq!(result.unwrap(), vec!["MO(3)", "TH(A, MO(3))"]);
    }

    #[test]
    fn test_composite_actions_rejected_in_slots() {
        // Tap-hold / morse forms are not single `Action`s, so they cannot nest
        // inside a slot. The grammar must reject these.
        let invalid_cases = ["MT(MT(A, LCtrl), LShift)", "TH(TD(0), B)", "MT(LT(1, A), LShift)"];

        for input in invalid_cases {
            let result = ConfigParser::parse(Rule::key_map, input);
            assert!(result.is_err(), "Input should be rejected: {}", input);
        }
    }

    #[test]
    fn double_slash_is_not_a_comment() {
        let aliases = HashMap::new();
        let layer_names = HashMap::new();
        // `keys` is data-only now: `//` is no longer a comment, just (garbage) tokens.
        let result = KeyboardTomlConfig::keymap_parser("A // B", &aliases, &layer_names);
        assert_eq!(result.unwrap(), vec!["A", "//", "B"]);
    }

    fn config(toml: &str) -> KeyboardTomlConfig {
        toml::from_str(toml).expect("parse test config")
    }

    #[test]
    fn layers_defaults_to_block_count() {
        let cfg = config(
            "[layout]\nrows = 1\ncols = 2\nmap = \"(0,0) (0,1)\"\n\
             [keymap]\n[[keymap.layer]]\nkeys = \"A B\"\n[[keymap.layer]]\nkeys = \"C D\"\n",
        );
        let (km, _) = cfg.get_keymap_config().unwrap();
        assert_eq!(km.layers, 2);
        assert_eq!(km.keymap.len(), 2);
    }

    #[test]
    fn explicit_layers_reserves_extra_transparent_layers() {
        let cfg = config(
            "[layout]\nrows = 1\ncols = 1\nmap = \"(0,0)\"\n\
             [keymap]\nlayers = 4\n[[keymap.layer]]\nkeys = \"A\"\n",
        );
        let (km, _) = cfg.get_keymap_config().unwrap();
        assert_eq!(km.layers, 4);
        assert_eq!(km.keymap.len(), 4);
        assert_eq!(km.keymap[3][0][0], "_"); // reserved layers are transparent
    }

    #[test]
    fn explicit_layers_below_block_count_is_rejected() {
        let cfg = config(
            "[layout]\nrows = 1\ncols = 1\nmap = \"(0,0)\"\n\
             [keymap]\nlayers = 1\n[[keymap.layer]]\nkeys = \"A\"\n[[keymap.layer]]\nkeys = \"B\"\n",
        );
        assert!(cfg.get_keymap_config().is_err());
    }

    #[test]
    fn layer_with_more_encoders_than_hardware_is_rejected() {
        // The board declares no encoders, but a layer lists one → rejected (not silently dropped).
        let cfg = config(
            "[matrix]\nrow_pins = [\"r0\"]\ncol_pins = [\"c0\"]\n\
             [layout]\nrows = 1\ncols = 1\nmap = \"(0,0)\"\n\
             [keymap]\n[[keymap.layer]]\nkeys = \"A\"\nencoders = [[\"Up\", \"Down\"]]\n",
        );
        assert!(cfg.keymap().is_err());
    }
}
