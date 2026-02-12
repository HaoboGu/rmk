//! Implementation of the `keymap!` proc-macro
//!
//! This module provides a proc-macro for defining keymaps directly in Rust code,
//! reusing the same parsing logic as `keyboard.toml`.

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
        let mut layers = Vec::new();

        // Parse the macro input
        while !input.is_empty() {
            let ident: syn::Ident = input.parse()?;
            input.parse::<Token![:]>()?;

            match ident.to_string().as_str() {
                "matrix_map" => {
                    let lit: LitStr = input.parse()?;
                    matrix_map = Some(lit.value());
                }
                "layers" => {
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
                                    let lit: LitInt = layer_content.parse()?;
                                    layer_num = Some(lit.base10_parse::<usize>()?);
                                }
                                "name" => {
                                    let lit: LitStr = layer_content.parse()?;
                                    layer_name = Some(lit.value());
                                }
                                "layout" => {
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
    use std::collections::HashMap;

    // Parse matrix_map
    let matrix_coords = KeyboardTomlConfig::parse_matrix_map(&input.matrix_map)?;
    let total_keys = matrix_coords.len();

    // Build layer name map and convert to u32 for keymap_parser
    let mut layer_names = HashMap::new();
    for layer in &input.layers {
        if let Some(ref name) = layer.name {
            layer_names.insert(name.clone(), layer.layer as u32);
        }
    }

    // Parse each layer
    let mut layers_output = Vec::new();
    for layer_def in &input.layers {
        // Parse the key actions - keymap_parser handles:
        // 1. Alias resolution
        // 2. Layer name resolution
        // 3. Key action parsing
        let key_actions = KeyboardTomlConfig::keymap_parser(
            &layer_def.layout,
            &HashMap::new(), // No aliases in macro for now
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

        layers_output.push((layer_def.layer, key_actions));
    }

    // Sort layers by layer number
    layers_output.sort_by_key(|(layer_num, _)| *layer_num);

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
    use super::action_parser::parse_key;

    // Calculate dimensions from matrix_coords
    let max_row = matrix_coords.iter().map(|(r, _, _)| *r).max().unwrap_or(0) as usize + 1;
    let max_col = matrix_coords.iter().map(|(_, c, _)| *c).max().unwrap_or(0) as usize + 1;

    let mut all_layers = Vec::new();

    for (_layer_num, keys) in layers {
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
            .map(|row| {
                let keys_code: Vec<TokenStream2> = row
                    .iter()
                    .map(|key| parse_key(key.clone(), &None))
                    .collect();
                quote! { [#(#keys_code),*] }
            })
            .collect();

        all_layers.push(quote! { [#(#rows_code),*] });
    }

    Ok(quote! {
        [#(#all_layers),*]
    })
}
