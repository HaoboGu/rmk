use crate::HostConfig;

impl crate::KeyboardTomlConfig {
    pub fn get_host_config(&self) -> HostConfig {
        self.host.clone().unwrap_or_default()
    }
}
