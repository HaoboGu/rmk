//! Implementation of the `keymap!` proc-macro
//!
//! This module provides a proc-macro for defining keymaps directly in Rust code,
//! reusing the same parsing logic as `keyboard.toml`.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    panic::{AssertUnwindSafe, catch_unwind},
};

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    LitInt, LitStr, Token,
};

/// Main struct representing the keymap macro input
struct KeymapInput {
    matrix_map: String,
    aliases: HashMap<String, String>,
    layers: Vec<LayerDef>,
}

/// Represents a single layer definition
struct LayerDef {
    layer: usize,
    name: Option<String>,
    layout: String,
}

impl Parse for KeymapInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut matrix_map = None;
        let mut aliases = HashMap::new();
        let mut layers = Vec::new();
        let mut aliases_defined = false;
        let mut layers_defined = false;

        // Parse the macro input
        while !input.is_empty() {
            let ident: syn::Ident = input.parse()?;
            input.parse::<Token![:]>()?;

            match ident.to_string().as_str() {
                "matrix_map" => {
                    if matrix_map.is_some() {
                        return Err(syn::Error::new_spanned(
                            &ident,
                            "duplicate field: matrix_map",
                        ));
                    }
                    let lit: LitStr = input.parse()?;
                    matrix_map = Some(lit.value());
                }
                "aliases" => {
                    if aliases_defined {
                        return Err(syn::Error::new_spanned(
                            &ident,
                            "duplicate field: aliases",
                        ));
                    }
                    aliases_defined = true;

                    let content;
                    syn::braced!(content in input);

                    // Parse alias definitions: name = "value", ...
                    while !content.is_empty() {
                        let alias_name: syn::Ident = content.parse()?;
                        content.parse::<Token![=]>()?;
                        let alias_value: LitStr = content.parse()?;
                        if aliases
                            .insert(alias_name.to_string(), alias_value.value())
                            .is_some()
                        {
                            return Err(syn::Error::new_spanned(
                                alias_name,
                                "duplicate alias name is not allowed",
                            ));
                        }

                        // Parse optional comma
                        let _ = content.parse::<Token![,]>();
                    }
                }
                "layers" => {
                    if layers_defined {
                        return Err(syn::Error::new_spanned(
                            &ident,
                            "duplicate field: layers",
                        ));
                    }
                    layers_defined = true;

                    let content;
                    syn::bracketed!(content in input);

                    // Parse each layer
                    while !content.is_empty() {
                        let layer_content;
                        syn::braced!(layer_content in content);

                        let mut layer_num = None;
                        let mut layer_name = None;
                        let mut layer_layout = None;

                        // Parse layer fields
                        while !layer_content.is_empty() {
                            let field_ident: syn::Ident = layer_content.parse()?;
                            layer_content.parse::<Token![:]>()?;

                            match field_ident.to_string().as_str() {
                                "layer" => {
                                    if layer_num.is_some() {
                                        return Err(syn::Error::new_spanned(
                                            &field_ident,
                                            "duplicate field in layer definition: layer",
                                        ));
                                    }
                                    let lit: LitInt = layer_content.parse()?;
                                    layer_num = Some(lit.base10_parse::<usize>()?);
                                }
                                "name" => {
                                    if layer_name.is_some() {
                                        return Err(syn::Error::new_spanned(
                                            &field_ident,
                                            "duplicate field in layer definition: name",
                                        ));
                                    }
                                    let lit: LitStr = layer_content.parse()?;
                                    layer_name = Some(lit.value());
                                }
                                "layout" => {
                                    if layer_layout.is_some() {
                                        return Err(syn::Error::new_spanned(
                                            &field_ident,
                                            "duplicate field in layer definition: layout",
                                        ));
                                    }
                                    let lit: LitStr = layer_content.parse()?;
                                    layer_layout = Some(lit.value());
                                }
                                _ => {
                                    return Err(syn::Error::new_spanned(
                                        &field_ident,
                                        format!("unexpected field: {}", field_ident),
                                    ));
                                }
                            }

                            // Parse optional comma
                            let _ = layer_content.parse::<Token![,]>();
                        }

                        let layer = LayerDef {
                            layer: layer_num.ok_or_else(|| {
                                syn::Error::new(
                                    layer_content.span(),
                                    "layer field is required",
                                )
                            })?,
                            name: layer_name,
                            layout: layer_layout.ok_or_else(|| {
                                syn::Error::new(
                                    layer_content.span(),
                                    "layout field is required",
                                )
                            })?,
                        };

                        layers.push(layer);

                        // Parse optional comma between layers
                        let _ = content.parse::<Token![,]>();
                    }
                }
                _ => {
                    return Err(syn::Error::new_spanned(
                        &ident,
                        format!("unexpected field: {}", ident),
                    ));
                }
            }

            // Parse optional comma
            let _ = input.parse::<Token![,]>();
        }

        let matrix_map = matrix_map.ok_or_else(|| {
            syn::Error::new(input.span(), "matrix_map field is required")
        })?;

        if layers.is_empty() {
            return Err(syn::Error::new(
                input.span(),
                "at least one layer is required",
            ));
        }

        Ok(KeymapInput {
            matrix_map,
            aliases,
            layers,
        })
    }
}

/// Main implementation of the keymap! macro
pub fn keymap_impl(input: TokenStream) -> TokenStream {
    let keymap_input = match syn::parse::<KeymapInput>(input) {
        Ok(input) => input,
        Err(e) => return e.to_compile_error().into(),
    };

    match generate_keymap(&keymap_input) {
        Ok(tokens) => tokens.into(),
        Err(e) => syn::Error::new(proc_macro2::Span::call_site(), e)
            .to_compile_error()
            .into(),
    }
}

/// Generate the keymap TokenStream from the parsed input
fn generate_keymap(input: &KeymapInput) -> Result<TokenStream2, String> {
    use rmk_config::KeyboardTomlConfig;

    // Parse matrix_map
    let matrix_coords = KeyboardTomlConfig::parse_matrix_map(&input.matrix_map)?;
    let total_keys = matrix_coords.len();
    validate_unique_matrix_coords(&matrix_coords)?;

    // Validate layer IDs and build layer name map for keymap_parser.
    let mut layer_names = HashMap::new();
    let mut layer_ids = BTreeSet::new();
    for layer in &input.layers {
        if layer.layer > u8::MAX as usize {
            return Err(format!(
                "Layer {} is out of range. layer must be in [0..{}] because layer actions are u8",
                layer.layer,
                u8::MAX
            ));
        }

        if !layer_ids.insert(layer.layer) {
            return Err(format!(
                "duplicate layer id {} found; layer ids must be unique",
                layer.layer
            ));
        }

        if let Some(ref name) = layer.name
            && let Some(previous_layer) = layer_names.insert(name.clone(), layer.layer as u32)
        {
            return Err(format!(
                "duplicate layer name '{}' found (layers {} and {})",
                name, previous_layer, layer.layer
            ));
        }
    }

    let expected_max_layer = input.layers.len().saturating_sub(1);
    for (expected_layer, actual_layer) in layer_ids.iter().copied().enumerate() {
        if actual_layer != expected_layer {
            return Err(format!(
                "sparse layer id {} found; layer ids must be contiguous in 0..{}",
                actual_layer, expected_max_layer
            ));
        }
    }

    // Parse each layer and place in map by numeric ID.
    let mut layers_by_id = BTreeMap::new();
    for layer_def in &input.layers {
        // Parse the key actions - keymap_parser handles:
        // 1. Alias resolution
        // 2. Layer name resolution
        // 3. Key action parsing
        let key_actions = parse_layer_actions(
            layer_def.layer,
            &layer_def.layout,
            &input.aliases,
            &layer_names,
        )?;

        if key_actions.len() != total_keys {
            return Err(format!(
                "Layer {} has {} keys, but matrix_map defines {} positions",
                layer_def.layer,
                key_actions.len(),
                total_keys
            ));
        }

        validate_layer_references(
            layer_def.layer,
            &key_actions,
            &matrix_coords,
            &input.layers,
        )?;

        layers_by_id.insert(layer_def.layer, key_actions);
    }

    let layers_output: Vec<(usize, Vec<String>)> = layers_by_id.into_iter().collect();

    // Generate the output
    let layers_code = generate_layers_code(&layers_output, &matrix_coords)?;

    Ok(quote! {
        #layers_code
    })
}

/// Generate Rust code for all layers
fn generate_layers_code(
    layers: &[(usize, Vec<String>)],
    matrix_coords: &[(u8, u8, char)],
) -> Result<TokenStream2, String> {
    // Calculate dimensions from matrix_coords
    let max_row = matrix_coords.iter().map(|(r, _, _)| *r).max().unwrap_or(0) as usize + 1;
    let max_col = matrix_coords.iter().map(|(_, c, _)| *c).max().unwrap_or(0) as usize + 1;

    let mut all_layers = Vec::new();

    for (layer_num, keys) in layers {
        // Create a 2D grid initialized with "No" actions
        let mut grid: Vec<Vec<String>> = vec![vec!["No".to_string(); max_col]; max_row];

        // Fill the grid with actual key actions according to matrix_map
        for (i, key) in keys.iter().enumerate() {
            if i < matrix_coords.len() {
                let (row, col, _hand) = matrix_coords[i];
                grid[row as usize][col as usize] = key.clone();
            }
        }

        // Generate code for this layer
        let rows_code: Vec<TokenStream2> = grid
            .iter()
            .enumerate()
            .map(|(row_idx, row)| -> Result<TokenStream2, String> {
                let keys_code: Vec<TokenStream2> = row
                    .iter()
                    .enumerate()
                    .map(|(col_idx, key)| parse_key_action(*layer_num, row_idx, col_idx, key))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(quote! { [#(#keys_code),*] })
            })
            .collect::<Result<Vec<_>, _>>()?;

        all_layers.push(quote! { [#(#rows_code),*] });
    }

    Ok(quote! {
        [#(#all_layers),*]
    })
}

fn parse_layer_actions(
    layer_num: usize,
    layout: &str,
    aliases: &HashMap<String, String>,
    layer_names: &HashMap<String, u32>,
) -> Result<Vec<String>, String> {
    use rmk_config::KeyboardTomlConfig;

    let parsed = catch_unwind(AssertUnwindSafe(|| {
        KeyboardTomlConfig::keymap_parser(layout, aliases, layer_names)
    }));

    match parsed {
        Ok(result) => result.map_err(|e| format!("Layer {}: {}", layer_num, e)),
        Err(payload) => Err(format!(
            "Layer {}: keymap parser panicked: {}",
            layer_num,
            panic_payload_to_string(payload)
        )),
    }
}

fn parse_key_action(layer_num: usize, row: usize, col: usize, key: &str) -> Result<TokenStream2, String> {
    use super::action_parser::parse_key;

    let parsed = catch_unwind(AssertUnwindSafe(|| parse_key(key.to_string(), &None)));
    match parsed {
        Ok(tokens) => Ok(tokens),
        Err(payload) => Err(format!(
            "failed to parse key action '{}' at layer {}, row {}, col {}: {}",
            key,
            layer_num,
            row,
            col,
            panic_payload_to_string(payload)
        )),
    }
}

fn validate_unique_matrix_coords(matrix_coords: &[(u8, u8, char)]) -> Result<(), String> {
    let mut first_seen: HashMap<(u8, u8), usize> = HashMap::new();

    for (idx, (row, col, _hand)) in matrix_coords.iter().enumerate() {
        let coord = (*row, *col);
        if let Some(first_idx) = first_seen.get(&coord).copied() {
            return Err(format!(
                "matrix_map coordinate ({},{}) is duplicated at positions {} and {}",
                row, col, first_idx, idx
            ));
        } else {
            first_seen.insert(coord, idx);
        }
    }

    Ok(())
}

fn validate_layer_references(
    current_layer: usize,
    key_actions: &[String],
    matrix_coords: &[(u8, u8, char)],
    layers: &[LayerDef],
) -> Result<(), String> {
    let max_layer = layers.iter().map(|layer| layer.layer).max().unwrap_or(0);
    let layer_count = max_layer + 1;

    for (idx, action) in key_actions.iter().enumerate() {
        if let Some(referenced_layer) = parse_referenced_layer(action)?
            && referenced_layer >= layer_count
        {
            let position = matrix_coords
                .get(idx)
                .map(|(row, col, _)| format!("row {}, col {}", row, col))
                .unwrap_or_else(|| format!("key index {}", idx));
            return Err(format!(
                "layer reference {} in action '{}' at layer {} ({}) is out of range; valid layer range is 0..{}",
                referenced_layer,
                action,
                current_layer,
                position,
                layer_count - 1
            ));
        }
    }

    Ok(())
}

fn parse_referenced_layer(action: &str) -> Result<Option<usize>, String> {
    let lower = action.to_ascii_lowercase();
    let prefixes = ["mo(", "to(", "tg(", "tt(", "df(", "osl(", "lm(", "lt("];

    let is_layer_action = prefixes.iter().any(|prefix| lower.starts_with(prefix));
    if !is_layer_action {
        return Ok(None);
    }

    let open = action
        .find('(')
        .ok_or_else(|| format!("invalid action format '{}'", action))?;
    let close = action
        .rfind(')')
        .ok_or_else(|| format!("invalid action format '{}'", action))?;
    if close <= open + 1 {
        return Err(format!("invalid action format '{}'", action));
    }

    let inner = &action[open + 1..close];
    let layer_str = inner
        .split(',')
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("missing layer argument in '{}'", action))?;

    let layer_num = layer_str
        .parse::<usize>()
        .map_err(|_| format!("invalid layer argument '{}' in '{}'", layer_str, action))?;

    Ok(Some(layer_num))
}

fn panic_payload_to_string(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic payload".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_input(input: &str) -> KeymapInput {
        syn::parse_str::<KeymapInput>(input).expect("test input should parse")
    }

    #[test]
    fn rejects_non_contiguous_layers() {
        let input = parse_input(
            r#"
            matrix_map: "(0,0) (0,1)",
            layers: [
                { layer: 1, name: "one", layout: "A B" },
                { layer: 3, name: "three", layout: "C D" }
            ]
            "#,
        );

        let err = generate_keymap(&input).expect_err("generation should fail");
        assert!(
            err.contains("sparse layer id 1 found"),
            "non-contiguous layer ids must be rejected"
        );
    }

    #[test]
    fn rejects_duplicate_matrix_coordinates() {
        let input = parse_input(
            r#"
            matrix_map: "(0,0) (0,0)",
            layers: [
                { layer: 0, name: "base", layout: "A B" }
            ]
            "#,
        );

        let err = generate_keymap(&input).expect_err("generation should fail");
        assert!(err.contains("matrix_map coordinate (0,0) is duplicated"));
    }

    #[test]
    fn rejects_duplicate_layer_names() {
        let input = parse_input(
            r#"
            matrix_map: "(0,0)",
            layers: [
                { layer: 0, name: "base", layout: "A" },
                { layer: 1, name: "base", layout: "B" }
            ]
            "#,
        );

        let err = generate_keymap(&input).expect_err("generation should fail");
        assert!(err.contains("duplicate layer name 'base'"));
    }

    #[test]
    fn rejects_duplicate_layer_ids() {
        let input = parse_input(
            r#"
            matrix_map: "(0,0)",
            layers: [
                { layer: 0, name: "base0", layout: "A" },
                { layer: 0, name: "base1", layout: "B" }
            ]
            "#,
        );

        let err = generate_keymap(&input).expect_err("generation should fail");
        assert!(err.contains("duplicate layer id 0 found"));
    }

    #[test]
    fn rejects_duplicate_alias_names() {
        let parse_result = syn::parse_str::<KeymapInput>(
            r#"
            matrix_map: "(0,0)",
            aliases: {
                dup = "A",
                dup = "B"
            },
            layers: [
                { layer: 0, layout: "A" }
            ]
            "#,
        );
        assert!(parse_result.is_err(), "duplicate aliases should be rejected");
    }

    #[test]
    fn rejects_duplicate_top_level_matrix_map() {
        let parse_result = syn::parse_str::<KeymapInput>(
            r#"
            matrix_map: "(0,0)",
            matrix_map: "(0,1)",
            layers: [
                { layer: 0, layout: "A" }
            ]
            "#,
        );
        assert!(
            parse_result.is_err(),
            "duplicate top-level matrix_map should be rejected"
        );
    }

    #[test]
    fn rejects_duplicate_top_level_aliases() {
        let parse_result = syn::parse_str::<KeymapInput>(
            r#"
            matrix_map: "(0,0)",
            aliases: { a = "A" },
            aliases: { b = "B" },
            layers: [
                { layer: 0, layout: "@a" }
            ]
            "#,
        );
        assert!(
            parse_result.is_err(),
            "duplicate top-level aliases should be rejected"
        );
    }

    #[test]
    fn rejects_duplicate_top_level_layers() {
        let parse_result = syn::parse_str::<KeymapInput>(
            r#"
            matrix_map: "(0,0)",
            layers: [
                { layer: 0, layout: "A" }
            ],
            layers: [
                { layer: 1, layout: "B" }
            ]
            "#,
        );
        assert!(
            parse_result.is_err(),
            "duplicate top-level layers should be rejected"
        );
    }

    #[test]
    fn rejects_duplicate_field_in_layer_definition() {
        let parse_result = syn::parse_str::<KeymapInput>(
            r#"
            matrix_map: "(0,0)",
            layers: [
                { layer: 0, layer: 1, layout: "A" }
            ]
            "#,
        );
        assert!(
            parse_result.is_err(),
            "duplicate fields in a layer definition should be rejected"
        );
    }

    #[test]
    fn rejects_layer_ids_out_of_u8_range() {
        let input = parse_input(
            r#"
            matrix_map: "(0,0)",
            layers: [
                { layer: 256, name: "overflow", layout: "A" }
            ]
            "#,
        );

        let err = generate_keymap(&input).expect_err("generation should fail");
        assert!(
            err.contains("out of range"),
            "error should explain layer id range"
        );
    }

    #[test]
    fn invalid_layer_actions_return_errors_instead_of_panicking() {
        let input = parse_input(
            r#"
            matrix_map: "(0,0)",
            layers: [
                { layer: 0, name: "base", layout: "MO(999)" }
            ]
            "#,
        );

        let err = generate_keymap(&input).expect_err("generation should fail");
        assert!(
            err.contains("out of range") || err.contains("failed to parse key action"),
            "should return validation or parse errors instead of panicking"
        );
    }

    #[test]
    fn rejects_out_of_range_layer_references() {
        let input = parse_input(
            r#"
            matrix_map: "(0,0)",
            layers: [
                { layer: 0, layout: "MO(3)" },
                { layer: 1, layout: "A" }
            ]
            "#,
        );

        let err = generate_keymap(&input).expect_err("generation should fail");
        assert!(
            err.contains("out of range"),
            "layer references beyond generated layer count must fail"
        );
    }
}
