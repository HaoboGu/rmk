use pest::Parser;
use pest_derive::Parser;

use crate::{KeyboardTomlConfig, LayoutConfig};
use std::collections::HashMap;

// Pest parser using the grammar files
#[derive(Parser)]
#[grammar = "keymap.pest"]
struct ConfigParser;

// Max alias resolution depth to prevent infinite loops
const MAX_ALIAS_RESOLUTION_DEPTH: usize = 10;

impl KeyboardTomlConfig {
    /// Layout is a mandatory field in toml, so we mainly check the sizes
    pub fn get_layout_config(&self) -> Result<LayoutConfig, String> {
        let aliases = self.aliases.clone().unwrap_or_default();
        let layers = self.layer.clone().unwrap_or_default();
        let mut layout = self.layout.clone().expect("layout config is required");
        // temporarily allow both matrix_map and keymap to be set and append the obsolete layout.keymap based layer configurations
        // to the new [[layer]] based layer configurations in the resulting LayoutConfig

        // Check alias keys for whitespace
        for key in aliases.keys() {
            if key.chars().any(char::is_whitespace) {
                return Err(format!(
                    "keyboard.toml: Alias key '{}' must not contain whitespace characters",
                    key
                ));
            }
        }
        let mut final_layers = Vec::<Vec<Vec<String>>>::new();
        let mut sequence_to_grid: Option<Vec<(u8, u8)>> = None;
        if let Some(matrix_map) = &layout.matrix_map {
            // process matrix_map first to build mapping between the electronic grid and the configuration sequence of keys
            let mut sequence_number = 0u32;
            let mut grid_to_sequence: Vec<Vec<Option<u32>>> =
                vec![vec![None; layout.cols as usize]; layout.rows as usize];
            match Self::parse_matrix_map(matrix_map) {
                Ok(coords) => {
                    for (row, col) in &coords {
                        if *row >= layout.rows || *col >= layout.cols {
                            return Err(format!(
                                "keyboard.toml: Coordinate ({},{}) in `layout.matrix_map` is out of bounds: ([0..{}], [0..{}]) is the expected range",
                                row, col, layout.rows-1, layout.cols-1
                            ));
                        }
                        if grid_to_sequence[*row as usize][*col as usize].is_some() {
                            return Err(format!(
                                "keyboard.toml: Duplicate coordinate ({},{}) found in `layout.matrix_map`",
                                row, col
                            ));
                        } else {
                            grid_to_sequence[*row as usize][*col as usize] = Some(sequence_number);
                        }
                        sequence_number += 1;
                    }
                    sequence_to_grid = Some(coords);
                }
                Err(parse_err) => {
                    // Pest error already includes details about the invalid format
                    return Err(format!("keyboard.toml: Error in `layout.matrix_map`: {}", parse_err));
                }
            }
        } else if !layers.is_empty() {
            return Err("layout.matrix_map is need to be defined to process [[layer]] based key maps".to_string());
        }
        if let Some(sequence_to_grid) = &sequence_to_grid {
            // collect layer names first
            let mut layer_names = HashMap::<String, u32>::new();
            for (layer_number, layer) in layers.iter().enumerate() {
                if let Some(name) = &layer.name {
                    if layer_names.contains_key(name) {
                        return Err(format!(
                            "keyboard.toml: Duplicate layer name '{}' found in `layout.keymap`",
                            name
                        ));
                    }
                    layer_names.insert(name.clone(), layer_number as u32);
                }
            }
            if layers.len() > layout.layers as usize {
                return Err("keyboard.toml: Number of [[layer]] entries is larger than layout.layers".to_string());
            }
            // Parse each explicitly defined [[layer]] with pest into the final_layers vector
            // using the previously defined sequence_to_grid mapping to fill in the
            // grid shaped classic keymaps
            let layer_names = layer_names;
            for (layer_number, layer) in layers.iter().enumerate() {
                // each layer should contain a sequence of keymap entries
                // their number and order should match the number and order of the above parsed matrix map
                match Self::keymap_parser(&layer.keys, &aliases, &layer_names) {
                    Ok(key_action_sequence) => {
                        let mut legacy_keymap =
                            vec![vec!["No".to_string(); layout.cols as usize]; layout.rows as usize];
                        for (sequence_number, key_action) in key_action_sequence.into_iter().enumerate() {
                            if sequence_number >= sequence_to_grid.len() {
                                return Err(format!(
                                    "keyboard.toml: {} layer #{} contains too many entries (must match layout.matrix_map)", &layer.name.clone().unwrap_or_default(), layer_number));
                            }
                            let (row, col) = sequence_to_grid[sequence_number];
                            legacy_keymap[row as usize][col as usize] = key_action.clone();
                        }
                        final_layers.push(legacy_keymap);
                    }
                    Err(parse_err) => {
                        return Err(format!("keyboard.toml: Error in `layout.keymap`: {}", parse_err));
                    }
                }
            }
        }
        // Handle the deprecated `keymap` field if present
        if let Some(keymap) = &mut layout.keymap {
            final_layers.append(keymap);
        }
        // The required number of layers is less than what's set in keymap
        // Fill the rest with empty keys
        if final_layers.len() <= layout.layers as usize {
            for _ in final_layers.len()..layout.layers as usize {
                // Add 2D vector of empty keys
                final_layers.push(vec![vec!["_".to_string(); layout.cols as usize]; layout.rows as usize]);
            }
        } else {
            return Err(format!(
                "keyboard.toml: The actual number of layers is larger than {} [layout.layers]: {} [[Layer]] entries + {} layers in layout.keymap",
                layout.layers, layers.len(), layout.keymap.as_ref().map(|keymap| keymap.len()).unwrap_or_default()
            ));
        }
        // Row
        if final_layers.iter().any(|r| r.len() as u8 != layout.rows) {
            return Err("keyboard.toml: Row number in keymap doesn't match with [layout.row]".to_string());
        }
        // Col
        if final_layers
            .iter()
            .any(|r| r.iter().any(|c| c.len() as u8 != layout.cols))
        {
            return Err("keyboard.toml: Col number in keymap doesn't match with [layout.col]".to_string());
        }
        Ok(LayoutConfig {
            rows: layout.rows,
            cols: layout.cols,
            layers: layout.layers,
            keymap: final_layers,
        })
    }

    /// Parses and validates a matrix_map string using Pest.
    /// Ensures the string contains only valid coordinates and whitespace.
    fn parse_matrix_map(matrix_map: &str) -> Result<Vec<(u8, u8)>, String> {
        match ConfigParser::parse(Rule::matrix_map, matrix_map) {
            Ok(pairs) => {
                let mut coordinates = Vec::new();
                // The top-level pair is 'matrix_map'. We need to iterate its inner content.
                for pair in pairs {
                    // Should only be one pair matching Rule::matrix_map
                    if pair.as_rule() == Rule::matrix_map {
                        for inner_pair in pair.into_inner() {
                            match inner_pair.as_rule() {
                                Rule::coordinate => {
                                    let mut coord_parts = inner_pair.into_inner(); // Should contain two 'number' pairs

                                    let row_str = coord_parts.next().ok_or("Missing row coordinate")?.as_str();
                                    let col_str = coord_parts.next().ok_or("Missing col coordinate")?.as_str();

                                    let row = row_str
                                        .parse::<u8>()
                                        .map_err(|e| format!("Failed to parse row '{}': {}", row_str, e))?;
                                    let col = col_str
                                        .parse::<u8>()
                                        .map_err(|e| format!("Failed to parse col '{}': {}", col_str, e))?;

                                    coordinates.push((row, col));
                                }
                                Rule::EOI | Rule::WHITESPACE => {
                                    // Ignore End Of Input marker
                                }
                                _ => {
                                    // This case should not be reached
                                    return Err(format!(
                                        "Unexpected rule encountered during layout.matrix_map processing: {:?}",
                                        inner_pair.as_rule()
                                    ));
                                }
                            }
                        }
                    }
                }
                Ok(coordinates)
            }
            Err(e) => Err(format!("Invalid layout.matrix_map format: {}", e)),
        }
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

    fn layer_name_resolver(
        prefix: &str,
        pair: pest::iterators::Pair<Rule>,
        layer_names: &HashMap<String, u32>,
    ) -> Result<String, String> {
        let mut action = prefix.to_string() + "(";

        for inner_pair in pair.into_inner() {
            match inner_pair.as_rule() {
                //the first argument is the layer name or layer number
                Rule::layer_name => {
                    // Check if the layer name is valid
                    let layer_name = inner_pair.as_str().to_string();
                    if let Some(layer_number) = layer_names.get(&layer_name) {
                        action += layer_number.to_string().as_str();
                    } else {
                        return Err(format!("Invalid layer name: {}", layer_name));
                    }
                }
                Rule::layer_number => {
                    action += inner_pair.as_str();
                }
                _ => {
                    // the second argument is not processed, just forwarded
                    action += ", ";
                    action += inner_pair.as_str();
                }
            }
        }
        action += ")";

        Ok(action)
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
                                Rule::no_action => {
                                    let action = inner_pair.as_str().to_string();
                                    key_action_sequence.push(action);
                                }

                                Rule::transparent_action => {
                                    let action = inner_pair.as_str().to_string();
                                    key_action_sequence.push(action);
                                }

                                Rule::simple_keycode => {
                                    let action = inner_pair.as_str().to_string();
                                    key_action_sequence.push(action);
                                }

                                Rule::shifted_action => {
                                    let action = inner_pair.as_str().to_string();
                                    key_action_sequence.push(action);
                                }

                                Rule::osm_action => {
                                    let action = inner_pair.as_str().to_string();
                                    key_action_sequence.push(action);
                                }

                                Rule::wm_action => {
                                    let action = inner_pair.as_str().to_string();
                                    key_action_sequence.push(action);
                                }

                                //layer actions:
                                Rule::df_action => {
                                    key_action_sequence.push(Self::layer_name_resolver("DF", inner_pair, layer_names)?);
                                }
                                Rule::mo_action => {
                                    key_action_sequence.push(Self::layer_name_resolver("MO", inner_pair, layer_names)?);
                                }
                                Rule::lm_action => {
                                    key_action_sequence.push(Self::layer_name_resolver("LM", inner_pair, layer_names)?);
                                }
                                Rule::lt_action => {
                                    key_action_sequence.push(Self::layer_name_resolver("LT", inner_pair, layer_names)?);
                                    //"LT(".to_owned() + &Self::layer_name_resolver(inner_pair, layer_names)? + ")");
                                }
                                Rule::osl_action => {
                                    key_action_sequence.push(Self::layer_name_resolver(
                                        "OSL",
                                        inner_pair,
                                        layer_names,
                                    )?);
                                }
                                Rule::tt_action => {
                                    key_action_sequence.push(Self::layer_name_resolver("TT", inner_pair, layer_names)?);
                                }
                                Rule::tg_action => {
                                    key_action_sequence.push(Self::layer_name_resolver("TG", inner_pair, layer_names)?);
                                }
                                Rule::to_action => {
                                    key_action_sequence.push(Self::layer_name_resolver("TO", inner_pair, layer_names)?);
                                }

                                //tap-hold actions:
                                Rule::mt_action => {
                                    let action = inner_pair.as_str().to_string();
                                    key_action_sequence.push(action);
                                }
                                Rule::th_action => {
                                    let action = inner_pair.as_str().to_string();
                                    key_action_sequence.push(action);
                                }

                                Rule::EOI | Rule::WHITESPACE => {
                                    // Ignore End of input marker
                                }
                                _ => {
                                    // This case should not be reached
                                    return Err(format!(
                                        "Unexpected rule encountered during layer.keys processing:{:?}",
                                        inner_pair.as_rule()
                                    ));
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
