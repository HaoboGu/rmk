use crate::DisplayConfig;

impl crate::KeyboardTomlConfig {
    pub(crate) fn get_display_config(&self) -> Option<DisplayConfig> {
        self.display.clone()
    }
}
