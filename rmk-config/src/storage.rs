use crate::StorageConfig;

impl crate::KeyboardTomlConfig {
    pub fn get_storage_config(&self) -> StorageConfig {
        self.storage.unwrap_or_default()
    }
}
