use crate::KeyInfo;

/// Resolved keymap for keymap generation: layer count, per-layer actions,
/// encoder map, plus the matrix-derived per-key info and grid dimensions.
pub struct Keymap {
    pub rows: u8,
    pub cols: u8,
    pub layers: u8,
    pub keymap: Vec<Vec<Vec<String>>>,
    pub encoder_map: Vec<Vec<[String; 2]>>,
    pub key_info: Vec<Vec<KeyInfo>>,
    /// Total number of encoders on the board.
    pub num_encoder: usize,
}

impl crate::KeyboardTomlConfig {
    /// Resolve the keymap configuration from TOML config.
    pub fn keymap(&self) -> Result<Keymap, String> {
        let (keymap_config, key_info) = self.get_keymap_config()?;
        // Encoders may be spread across split halves; only the board-wide total is used downstream.
        let num_encoder: usize = self.get_board_config()?.get_num_encoder().iter().sum();

        // Encoder maps are all-or-none; partial lists would leave encoders dead.
        for (i, encoders) in keymap_config.encoder_map.iter().enumerate() {
            if !encoders.is_empty() && encoders.len() != num_encoder {
                return Err(format!(
                    "keyboard.toml: [[keymap.layer]] #{i} lists {} encoders but the board has \
                     {num_encoder} (configure all {num_encoder} or none)",
                    encoders.len()
                ));
            }
        }

        Ok(Keymap {
            rows: keymap_config.rows,
            cols: keymap_config.cols,
            layers: keymap_config.layers,
            keymap: keymap_config.keymap,
            encoder_map: keymap_config.encoder_map,
            key_info,
            num_encoder,
        })
    }
}
