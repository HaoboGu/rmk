/// Resolved host-tool configuration.
pub struct Host {
    pub vial_enabled: bool,
    pub rynk_enabled: bool,
    pub unlock_keys: Vec<[u8; 2]>,
    pub vial_insecure: bool,
}

impl crate::KeyboardTomlConfig {
    /// Resolve host-tool configuration from TOML config.
    pub fn host(&self) -> Host {
        let host_toml = self.get_host_config();
        Host {
            vial_enabled: host_toml.vial_enabled,
            rynk_enabled: host_toml.rynk_enabled,
            unlock_keys: host_toml.unlock_keys.unwrap_or_default(),
            vial_insecure: host_toml.vial_insecure,
        }
    }
}
