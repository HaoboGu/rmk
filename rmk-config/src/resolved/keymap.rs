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
    pub encoder_counts: Vec<usize>,
}

impl crate::KeyboardTomlConfig {
    /// Resolve the keymap configuration from TOML config.
    pub fn keymap(&self) -> Result<Keymap, String> {
        let (keymap_config, key_info) = self.get_keymap_config()?;
        let board = self.get_board_config()?;
        let encoder_counts = board.get_num_encoder();

        // A layer may list fewer encoders than the hardware has (the rest default to No),
        // but listing more is an unambiguous mistake that codegen would otherwise silently drop.
        let total_encoders: usize = encoder_counts.iter().sum();
        for (i, encoders) in keymap_config.encoder_map.iter().enumerate() {
            if encoders.len() > total_encoders {
                return Err(format!(
                    "keyboard.toml: [[keymap.layer]] #{i} lists {} encoders but the board has {total_encoders}",
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
            encoder_counts,
        })
    }
}
