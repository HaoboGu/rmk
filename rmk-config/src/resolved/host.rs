/// Resolved host-tool configuration.
pub struct Host {
    pub vial_enabled: bool,
    pub rynk_enabled: bool,
    pub unlock_keys: Vec<[u8; 2]>,
    pub insecure: bool,
    pub write_requires_unlock: bool,
}

impl crate::KeyboardTomlConfig {
    /// Resolve host-tool configuration from TOML config.
    pub fn host(&self) -> Host {
        let host_toml = self.get_host_config();
        let unlock_keys = host_toml.unlock_keys.unwrap_or_default();

        // Must match `rmk_types::protocol::rynk::UNLOCK_KEYS_SIZE` (rmk-config
        // can't depend on rmk-types — rmk-types build-depends on rmk-config).
        const UNLOCK_KEYS_MAX: usize = 4;
        if unlock_keys.len() > UNLOCK_KEYS_MAX {
            panic!(
                "❌ Parse `keyboard.toml` error: [host].unlock_keys has {} entries, the max is {UNLOCK_KEYS_MAX}",
                unlock_keys.len()
            );
        }
        // Every position must be inside the matrix. Skip when the layout can't
        // resolve — that error is surfaced by the layout resolver itself.
        if let Ok((layout, _)) = self.get_layout_config() {
            for key in &unlock_keys {
                let (row, col) = (key[0], key[1]);
                if row >= layout.rows || col >= layout.cols {
                    panic!(
                        "❌ Parse `keyboard.toml` error: [host].unlock_keys position ({row}, {col}) is outside the {}×{} matrix",
                        layout.rows, layout.cols
                    );
                }
            }
        }

        Host {
            vial_enabled: host_toml.vial_enabled,
            rynk_enabled: host_toml.rynk_enabled,
            unlock_keys,
            insecure: host_toml.insecure,
            write_requires_unlock: host_toml.write_requires_unlock,
        }
    }
}
