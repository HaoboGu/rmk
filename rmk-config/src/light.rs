use crate::LightConfig;

impl crate::KeyboardTomlConfig {
    pub fn get_light_config(&self) -> LightConfig {
        let default = LightConfig::default();
        match self.light.clone() {
            Some(mut light_config) => {
                light_config.capslock = light_config.capslock.or(default.capslock);
                light_config.numslock = light_config.numslock.or(default.numslock);
                light_config.scrolllock = light_config.scrolllock.or(default.scrolllock);
                light_config
            }
            None => default,
        }
    }
}
