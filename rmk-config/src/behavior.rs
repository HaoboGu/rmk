use crate::{BehaviorConfig, LayoutConfig, MacroOperation};

impl crate::KeyboardTomlConfig {
    pub fn get_behavior_config(&self) -> Result<(BehaviorConfig, LayoutConfig), String> {
        let default = self.behavior.clone().unwrap_or_default();
        let (layout, key_info) = self.get_layout_config().unwrap();
        match self.behavior.clone() {
            Some(mut behavior) => {
                behavior.key_info = if key_info.is_empty()
                    || key_info.iter().all(|row| {
                        row.iter().all(|key| {
                            key.hand != 'L'
                                && key.hand != 'l'
                                && key.hand != 'R'
                                && key.hand != 'r'
                                && key.profile.is_none()
                        })
                    }) {
                    None
                } else {
                    Some(key_info)
                };
                behavior.tri_layer = match behavior.tri_layer {
                    Some(tri_layer) => {
                        if tri_layer.upper >= layout.layers {
                            return Err("keyboard.toml: Tri layer upper is larger than [layout.layers]".to_string());
                        } else if tri_layer.lower >= layout.layers {
                            return Err("keyboard.toml: Tri layer lower is larger than [layout.layers]".to_string());
                        } else if tri_layer.adjust >= layout.layers {
                            return Err("keyboard.toml: Tri layer adjust is larger than [layout.layers]".to_string());
                        }
                        Some(tri_layer)
                    }
                    None => default.tri_layer,
                };
                behavior.one_shot = behavior.one_shot.or(default.one_shot);
                behavior.combo = behavior.combo.or(default.combo);
                if let Some(combo) = &behavior.combo {
                    if combo.combos.len() > self.rmk.combo_max_num {
                        return Err("keyboard.toml: number of combos is greater than combo_max_num configured under [rmk] section".to_string());
                    }
                    for (i, c) in combo.combos.iter().enumerate() {
                        if c.actions.len() > self.rmk.combo_max_length {
                            return Err(format!(
                                "keyboard.toml: number of keys in combo #{} is greater than combo_max_length configured under [rmk] section",
                                i
                            ));
                        }
                        if let Some(layer) = c.layer {
                            if layer >= layout.layers {
                                return Err(format!(
                                    "keyboard.toml: layer in combo #{} is greater than [layout.layers]",
                                    i
                                ));
                            }
                        }
                    }
                }
                behavior.macros = behavior.macros.or(default.macros);
                if let Some(macros) = &behavior.macros {
                    let macros_size = macros
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
                        .sum::<usize>();

                    if macros_size > self.rmk.macro_space_size {
                        return Err(format!(
                            "keyboard.toml: total size of macros ({}) is greater than macro_space_size configured under [rmk] section",
                            macros_size
                        ));
                    }
                }
                behavior.fork = behavior.fork.or(default.fork);
                if let Some(fork) = &behavior.fork
                    && fork.forks.len() > self.rmk.fork_max_num
                {
                    return Err(
                        "keyboard.toml: number of forks is greater than fork_max_num configured under [rmk] section"
                            .to_string(),
                    );
                }
                behavior.morse = behavior.morse.or(default.morse);
                if let Some(morse) = &behavior.morse
                    && let Some(morses) = &morse.morses
                    && morses.len() > self.rmk.morse_max_num
                {
                    return Err(
                        "keyboard.toml: number of morses is greater than morse_max_num configured under [rmk] section"
                            .to_string(),
                    );
                }
                Ok((behavior, layout))
            }
            None => Ok((default, layout)),
        }
    }
}
