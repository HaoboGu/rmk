use crate::KeyInfo;

/// Resolved layout configuration for keymap generation.
pub struct Layout {
    pub rows: u8,
    pub cols: u8,
    pub layers: u8,
    pub keymap: Vec<Vec<Vec<String>>>,
    pub encoder_map: Vec<Vec<[String; 2]>>,
    pub key_info: Vec<Vec<KeyInfo>>,
    pub encoder_counts: Vec<usize>,
}

impl crate::KeyboardTomlConfig {
    /// Resolve layout configuration from TOML config.
    pub fn layout(&self) -> Result<Layout, String> {
        let (layout_config, key_info) = self.get_layout_config()?;
        let board = self.get_board_config()?;
        Ok(Layout {
            rows: layout_config.rows,
            cols: layout_config.cols,
            layers: layout_config.layers,
            keymap: layout_config.keymap,
            encoder_map: layout_config.encoder_map,
            key_info,
            encoder_counts: board.get_num_encoder(),
        })
    }
}
