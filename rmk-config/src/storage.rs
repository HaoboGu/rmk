use crate::StorageConfig;

impl crate::KeyboardTomlConfig {
    pub fn get_storage_config(&self) -> StorageConfig {
        let default = self.get_default_config().unwrap().storage;
        if let Some(mut storage) = self.storage.clone() {
            storage.start_addr = storage.start_addr.or(default.start_addr);
            storage.num_sectors = storage.num_sectors.or(default.num_sectors);
            storage.clear_storage = storage.clear_storage.or(default.clear_storage);
            storage
        } else {
            default
        }
    }
}
