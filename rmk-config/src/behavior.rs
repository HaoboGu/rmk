use crate::error::ConfigResult;
use crate::BehaviorConfig;

impl crate::KeyboardTomlConfig {
    pub fn get_behavior_config(&self) -> ConfigResult<BehaviorConfig> {
        let default = self.behavior.clone().unwrap_or_default();
        match self.behavior.clone() {
            Some(mut behavior) => {
                // Merge with defaults
                behavior.tri_layer = behavior.tri_layer.or(default.tri_layer);
                behavior.one_shot = behavior.one_shot.or(default.one_shot);
                behavior.combo = behavior.combo.or(default.combo);
                behavior.macros = behavior.macros.or(default.macros);
                behavior.fork = behavior.fork.or(default.fork);
                behavior.morse = behavior.morse.or(default.morse);
                Ok(behavior)
            }
            None => Ok(default),
        }
    }
}
