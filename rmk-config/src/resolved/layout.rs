/// Resolved physical layout: the compressed, opaque blob the firmware streams
/// verbatim over `GetLayout`. Empty when there's no `[layout].map`.
pub struct Layout {
    pub blob: Vec<u8>,
}

impl crate::KeyboardTomlConfig {
    /// Resolve the physical layout blob from the `[layout]` section.
    pub fn layout(&self) -> Result<Layout, String> {
        let blob = match &self.layout {
            Some(l) => {
                let encoder_counts = self.get_board_config()?.get_num_encoder();
                crate::layout::build_layout_blob(l, Some(encoder_counts.iter().sum()))?
            }
            None => Vec::new(),
        };
        Ok(Layout { blob })
    }
}
