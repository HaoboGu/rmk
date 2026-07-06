use crate::DfuTomlConfig;

impl crate::KeyboardTomlConfig {
    pub(crate) fn get_dfu_config(&self) -> Option<DfuTomlConfig> {
        self.dfu.clone()
    }
}
