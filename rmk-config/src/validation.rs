//! Centralized validation for keyboard configuration
//!
//! All validation logic is collected here to ensure:
//! 1. Consistent error messages
//! 2. Single point of validation
//! 3. Clear validation order

use crate::defaults;
use crate::error::{ConfigError, ConfigResult};
use crate::{KeyboardTomlConfig, MacroOperation, MatrixType};

/// Validates the entire keyboard configuration
pub fn validate_config(config: &KeyboardTomlConfig) -> ConfigResult<()> {
    validate_keyboard_section(config)?;
    validate_rmk_constants(config)?;
    validate_board_section(config)?;
    validate_layout_section(config)?;
    validate_behavior_section(config)?;
    Ok(())
}

/// Validates the [keyboard] section
fn validate_keyboard_section(config: &KeyboardTomlConfig) -> ConfigResult<()> {
    let keyboard = config
        .keyboard
        .as_ref()
        .ok_or(ConfigError::MissingField {
            field: "keyboard".to_string(),
        })?;

    // Either board or chip must be set, but not both
    if keyboard.board.is_none() == keyboard.chip.is_none() {
        return Err(ConfigError::Validation {
            field: "keyboard.board/chip".to_string(),
            message: "Either 'board' or 'chip' should be set, but not both".to_string(),
        });
    }

    Ok(())
}

/// Validates the [rmk] constants section
fn validate_rmk_constants(config: &KeyboardTomlConfig) -> ConfigResult<()> {
    let rmk = &config.rmk;

    if rmk.combo_max_num > defaults::COMBO_MAX_NUM_LIMIT {
        return Err(ConfigError::InvalidValue {
            field: "rmk.combo_max_num".to_string(),
            value: rmk.combo_max_num.to_string(),
            expected: format!("0 to {}", defaults::COMBO_MAX_NUM_LIMIT),
        });
    }

    if rmk.fork_max_num > defaults::FORK_MAX_NUM_LIMIT {
        return Err(ConfigError::InvalidValue {
            field: "rmk.fork_max_num".to_string(),
            value: rmk.fork_max_num.to_string(),
            expected: format!("0 to {}", defaults::FORK_MAX_NUM_LIMIT),
        });
    }

    if rmk.morse_max_num > defaults::MORSE_MAX_NUM_LIMIT {
        return Err(ConfigError::InvalidValue {
            field: "rmk.morse_max_num".to_string(),
            value: rmk.morse_max_num.to_string(),
            expected: format!("0 to {}", defaults::MORSE_MAX_NUM_LIMIT),
        });
    }

    if !(defaults::MAX_PATTERNS_PER_KEY_MIN..=defaults::MAX_PATTERNS_PER_KEY_MAX)
        .contains(&rmk.max_patterns_per_key)
    {
        return Err(ConfigError::InvalidValue {
            field: "rmk.max_patterns_per_key".to_string(),
            value: rmk.max_patterns_per_key.to_string(),
            expected: format!(
                "{} to {}",
                defaults::MAX_PATTERNS_PER_KEY_MIN,
                defaults::MAX_PATTERNS_PER_KEY_MAX
            ),
        });
    }

    Ok(())
}

/// Validates the [matrix] or [split] section
fn validate_board_section(config: &KeyboardTomlConfig) -> ConfigResult<()> {
    let matrix = config.matrix.as_ref();
    let split = config.split.as_ref();

    match (matrix, split) {
        (None, Some(_)) => {
            // Split keyboard - valid
            Ok(())
        }
        (Some(m), None) => {
            // Unibody keyboard - validate matrix pins
            match m.matrix_type {
                MatrixType::normal => {
                    if m.row_pins.is_none() || m.col_pins.is_none() {
                        return Err(ConfigError::Validation {
                            field: "matrix".to_string(),
                            message: "`row_pins` and `col_pins` are required for normal matrix"
                                .to_string(),
                        });
                    }
                }
                MatrixType::direct_pin => {
                    if m.direct_pins.is_none() {
                        return Err(ConfigError::Validation {
                            field: "matrix".to_string(),
                            message: "`direct_pins` is required for direct pin matrix".to_string(),
                        });
                    }
                }
            }
            Ok(())
        }
        (None, None) => Err(ConfigError::Validation {
            field: "matrix/split".to_string(),
            message: "[matrix] section is required for non-split keyboard".to_string(),
        }),
        (Some(_), Some(_)) => Err(ConfigError::Validation {
            field: "matrix/split".to_string(),
            message: "Use at most one of [matrix] or [split] in your keyboard.toml!\n\
                -> [matrix] is used to define a normal matrix of non-split keyboard\n\
                -> [split] is used to define a split keyboard"
                .to_string(),
        }),
    }
}

/// Validates the [layout] section
fn validate_layout_section(config: &KeyboardTomlConfig) -> ConfigResult<()> {
    let layout = config.layout.as_ref().ok_or(ConfigError::MissingField {
        field: "layout".to_string(),
    })?;

    // Validate layer count
    if let Some(layers) = &config.layer {
        if layers.len() > layout.layers as usize {
            return Err(ConfigError::Validation {
                field: "layout.layers".to_string(),
                message: format!(
                    "Number of [[layer]] entries ({}) exceeds layout.layers ({})",
                    layers.len(),
                    layout.layers
                ),
            });
        }

        // matrix_map is required for [[layer]] format
        if !layers.is_empty() && layout.matrix_map.is_none() {
            return Err(ConfigError::Validation {
                field: "layout.matrix_map".to_string(),
                message: "layout.matrix_map is required when using [[layer]] sections".to_string(),
            });
        }
    }

    // Validate alias keys for whitespace
    if let Some(aliases) = &config.aliases {
        for key in aliases.keys() {
            if key.chars().any(char::is_whitespace) {
                return Err(ConfigError::Validation {
                    field: "aliases".to_string(),
                    message: format!(
                        "Alias key '{}' must not contain whitespace characters",
                        key
                    ),
                });
            }
        }
    }

    Ok(())
}

/// Validates the [behavior] section
fn validate_behavior_section(config: &KeyboardTomlConfig) -> ConfigResult<()> {
    let behavior = match &config.behavior {
        Some(b) => b,
        None => return Ok(()),
    };

    let layout = config.layout.as_ref().ok_or(ConfigError::MissingField {
        field: "layout".to_string(),
    })?;

    // Validate tri_layer
    if let Some(tri_layer) = &behavior.tri_layer {
        if tri_layer.upper >= layout.layers {
            return Err(ConfigError::Validation {
                field: "behavior.tri_layer.upper".to_string(),
                message: format!(
                    "tri_layer.upper ({}) must be less than layout.layers ({})",
                    tri_layer.upper, layout.layers
                ),
            });
        }
        if tri_layer.lower >= layout.layers {
            return Err(ConfigError::Validation {
                field: "behavior.tri_layer.lower".to_string(),
                message: format!(
                    "tri_layer.lower ({}) must be less than layout.layers ({})",
                    tri_layer.lower, layout.layers
                ),
            });
        }
        if tri_layer.adjust >= layout.layers {
            return Err(ConfigError::Validation {
                field: "behavior.tri_layer.adjust".to_string(),
                message: format!(
                    "tri_layer.adjust ({}) must be less than layout.layers ({})",
                    tri_layer.adjust, layout.layers
                ),
            });
        }
    }

    // Validate combos
    if let Some(combo) = &behavior.combo {
        if combo.combos.len() > config.rmk.combo_max_num {
            return Err(ConfigError::Validation {
                field: "behavior.combo.combos".to_string(),
                message: format!(
                    "Number of combos ({}) exceeds rmk.combo_max_num ({})",
                    combo.combos.len(),
                    config.rmk.combo_max_num
                ),
            });
        }
        for (i, c) in combo.combos.iter().enumerate() {
            if c.actions.len() > config.rmk.combo_max_length {
                return Err(ConfigError::Validation {
                    field: format!("behavior.combo.combos[{}]", i),
                    message: format!(
                        "Number of keys in combo ({}) exceeds rmk.combo_max_length ({})",
                        c.actions.len(),
                        config.rmk.combo_max_length
                    ),
                });
            }
            if let Some(layer) = c.layer {
                if layer >= layout.layers {
                    return Err(ConfigError::Validation {
                        field: format!("behavior.combo.combos[{}].layer", i),
                        message: format!(
                            "Combo layer ({}) must be less than layout.layers ({})",
                            layer, layout.layers
                        ),
                    });
                }
            }
        }
    }

    // Validate macros
    if let Some(macros) = &behavior.macros {
        let macros_size: usize = macros
            .macros
            .iter()
            .map(|m| {
                m.operations
                    .iter()
                    .map(|op| match op {
                        MacroOperation::Tap { .. }
                        | MacroOperation::Down { .. }
                        | MacroOperation::Up { .. } => 3,
                        MacroOperation::Delay { .. } => 4,
                        MacroOperation::Text { text } => text.len(),
                    })
                    .sum::<usize>()
            })
            .sum();

        if macros_size > config.rmk.macro_space_size {
            return Err(ConfigError::Validation {
                field: "behavior.macros".to_string(),
                message: format!(
                    "Total size of macros ({}) exceeds rmk.macro_space_size ({})",
                    macros_size, config.rmk.macro_space_size
                ),
            });
        }
    }

    // Validate forks
    if let Some(fork) = &behavior.fork {
        if fork.forks.len() > config.rmk.fork_max_num {
            return Err(ConfigError::Validation {
                field: "behavior.fork.forks".to_string(),
                message: format!(
                    "Number of forks ({}) exceeds rmk.fork_max_num ({})",
                    fork.forks.len(),
                    config.rmk.fork_max_num
                ),
            });
        }
    }

    // Validate morses
    if let Some(morse) = &behavior.morse {
        if let Some(morses) = &morse.morses {
            if morses.len() > config.rmk.morse_max_num {
                return Err(ConfigError::Validation {
                    field: "behavior.morse.morses".to_string(),
                    message: format!(
                        "Number of morses ({}) exceeds rmk.morse_max_num ({})",
                        morses.len(),
                        config.rmk.morse_max_num
                    ),
                });
            }

            // Validate max taps per morse
            for (i, morse_config) in morses.iter().enumerate() {
                let tap_actions_len = morse_config
                    .tap_actions
                    .as_ref()
                    .map(|v| v.len())
                    .unwrap_or(0);
                let hold_actions_len = morse_config
                    .hold_actions
                    .as_ref()
                    .map(|v| v.len())
                    .unwrap_or(0);
                let n = tap_actions_len.max(hold_actions_len);

                if n > defaults::MAX_TAPS_PER_MORSE {
                    return Err(ConfigError::Validation {
                        field: format!("behavior.morse.morses[{}]", i),
                        message: format!(
                            "Number of taps per morse ({}) exceeds maximum ({})",
                            n,
                            defaults::MAX_TAPS_PER_MORSE
                        ),
                    });
                }
            }
        }
    }

    Ok(())
}
