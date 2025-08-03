use crate::{BehaviorConfig, MacroOperation};

impl crate::KeyboardTomlConfig {
    pub fn get_behavior_config(&self) -> Result<BehaviorConfig, String> {
        let default = self.behavior.clone().unwrap_or_default();
        let layout = self.get_layout_config().unwrap();
        match self.behavior.clone() {
            Some(mut behavior) => {
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
                behavior.tap_hold = behavior.tap_hold.or(default.tap_hold);
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
                if let Some(fork) = &behavior.fork {
                    if fork.forks.len() > self.rmk.fork_max_num {
                        return Err("keyboard.toml: number of forks is greater than fork_max_num configured under [rmk] section".to_string());
                    }
                }
                behavior.tap_dance = behavior.tap_dance.or(default.tap_dance);
                if let Some(tap_dance) = &behavior.tap_dance {
                    if tap_dance.tap_dances.len() > self.rmk.tap_dance_max_num {
                        return Err("keyboard.toml: number of tap dances is greater than tap_dance_max_num configured under [rmk] section".to_string());
                    }
                }
                Ok(behavior)
            }
            None => Ok(default),
        }
    }
}
