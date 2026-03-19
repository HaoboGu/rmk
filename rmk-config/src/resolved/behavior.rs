use std::collections::HashMap;

/// Resolved behavioral configuration.
pub struct Behavior {
    pub tri_layer: Option<[u8; 3]>,
    pub one_shot_timeout_ms: Option<u64>,
    pub combos: Option<Combos>,
    pub macros: Option<Macros>,
    pub forks: Option<Forks>,
    pub morse: Option<Morse>,
}

pub struct Combos {
    pub combos: Vec<Combo>,
    pub timeout_ms: Option<u64>,
}

pub struct Combo {
    pub actions: Vec<String>,
    pub output: String,
    pub layer: Option<u8>,
}

pub struct Macros {
    pub macros: Vec<Macro>,
}

pub struct Macro {
    pub operations: Vec<MacroOperation>,
}

/// Resolved macro operation — all durations are plain milliseconds.
#[derive(Clone, Debug)]
pub enum MacroOperation {
    Tap { keycode: String },
    Down { keycode: String },
    Up { keycode: String },
    Delay { duration_ms: u64 },
    Text { text: String },
}

pub struct Forks {
    pub forks: Vec<Fork>,
}

pub struct Fork {
    pub trigger: String,
    pub negative_output: String,
    pub positive_output: String,
    pub match_any: Option<String>,
    pub match_none: Option<String>,
    pub kept_modifiers: Option<String>,
    pub bindable: bool,
}

pub struct Morse {
    pub enable_flow_tap: bool,
    pub prior_idle_time_ms: u64,
    pub default_profile: MorseProfile,
    pub profiles: HashMap<String, MorseProfile>,
    pub morses: Vec<MorseKey>,
}

#[derive(Clone)]
pub struct MorseProfile {
    pub unilateral_tap: Option<bool>,
    pub permissive_hold: Option<bool>,
    pub hold_on_other_press: Option<bool>,
    pub normal_mode: Option<bool>,
    pub hold_timeout_ms: Option<u64>,
    pub gap_timeout_ms: Option<u64>,
}

pub struct MorseKey {
    pub profile: Option<String>,
    pub tap: Option<String>,
    pub hold: Option<String>,
    pub hold_after_tap: Option<String>,
    pub double_tap: Option<String>,
    pub tap_actions: Option<Vec<String>>,
    pub hold_actions: Option<Vec<String>>,
    pub morse_actions: Option<Vec<MorseActionPair>>,
}

pub struct MorseActionPair {
    pub pattern: String,
    pub action: String,
}

impl crate::KeyboardTomlConfig {
    /// Resolve behavioral configuration from TOML config.
    pub fn behavior(&self) -> Result<Behavior, String> {
        let toml_behavior = self.get_behavior_config()?;

        let tri_layer = toml_behavior.tri_layer.map(|t| [t.upper, t.lower, t.adjust]);

        let one_shot_timeout_ms = toml_behavior.one_shot.and_then(|o| o.timeout.map(|t| t.0));

        let combos = toml_behavior.combo.map(|c| Combos {
            combos: c
                .combos
                .into_iter()
                .map(|combo| Combo {
                    actions: combo.actions,
                    output: combo.output,
                    layer: combo.layer,
                })
                .collect(),
            timeout_ms: c.timeout.map(|t| t.0),
        });

        let macros = toml_behavior.macros.map(|m| Macros {
            macros: m
                .macros
                .into_iter()
                .map(|mc| Macro {
                    operations: mc.operations.into_iter().map(resolve_macro_operation).collect(),
                })
                .collect(),
        });

        let forks = toml_behavior.fork.map(|f| Forks {
            forks: f
                .forks
                .into_iter()
                .map(|fork| Fork {
                    trigger: fork.trigger,
                    negative_output: fork.negative_output,
                    positive_output: fork.positive_output,
                    match_any: fork.match_any,
                    match_none: fork.match_none,
                    kept_modifiers: fork.kept_modifiers,
                    bindable: fork.bindable.unwrap_or(false),
                })
                .collect(),
        });

        let morse = toml_behavior.morse.map(|m| {
            let profiles = m
                .profiles
                .as_ref()
                .map(|p| {
                    p.iter()
                        .map(|(name, p)| (name.clone(), resolve_morse_profile(p)))
                        .collect()
                })
                .unwrap_or_default();

            let default_profile = MorseProfile {
                unilateral_tap: m.unilateral_tap,
                permissive_hold: m.permissive_hold,
                hold_on_other_press: m.hold_on_other_press,
                normal_mode: m.normal_mode,
                hold_timeout_ms: Some(m.hold_timeout.as_ref().map(|t| t.0).unwrap_or(250)),
                gap_timeout_ms: Some(m.gap_timeout.as_ref().map(|t| t.0).unwrap_or(250)),
            };

            let morses = m
                .morses
                .unwrap_or_default()
                .into_iter()
                .map(|mk| MorseKey {
                    profile: mk.profile,
                    tap: mk.tap,
                    hold: mk.hold,
                    hold_after_tap: mk.hold_after_tap,
                    double_tap: mk.double_tap,
                    tap_actions: mk.tap_actions,
                    hold_actions: mk.hold_actions,
                    morse_actions: mk.morse_actions.map(|pairs| {
                        pairs
                            .into_iter()
                            .map(|p| MorseActionPair {
                                pattern: p.pattern,
                                action: p.action,
                            })
                            .collect()
                    }),
                })
                .collect();

            Morse {
                enable_flow_tap: m.enable_flow_tap.unwrap_or(false),
                prior_idle_time_ms: m.prior_idle_time.map(|t| t.0).unwrap_or(120),
                default_profile,
                profiles,
                morses,
            }
        });

        Ok(Behavior {
            tri_layer,
            one_shot_timeout_ms,
            combos,
            macros,
            forks,
            morse,
        })
    }
}

fn resolve_macro_operation(op: crate::MacroOperation) -> MacroOperation {
    match op {
        crate::MacroOperation::Tap { keycode } => MacroOperation::Tap { keycode },
        crate::MacroOperation::Down { keycode } => MacroOperation::Down { keycode },
        crate::MacroOperation::Up { keycode } => MacroOperation::Up { keycode },
        crate::MacroOperation::Delay { duration } => MacroOperation::Delay {
            duration_ms: duration.0,
        },
        crate::MacroOperation::Text { text } => MacroOperation::Text { text },
    }
}

fn resolve_morse_profile(p: &crate::MorseProfile) -> MorseProfile {
    MorseProfile {
        unilateral_tap: p.unilateral_tap,
        permissive_hold: p.permissive_hold,
        hold_on_other_press: p.hold_on_other_press,
        normal_mode: p.normal_mode,
        hold_timeout_ms: p.hold_timeout.as_ref().map(|t| t.0),
        gap_timeout_ms: p.gap_timeout.as_ref().map(|t| t.0),
    }
}
