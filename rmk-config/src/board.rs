use crate::{InputDeviceConfig, KeyboardTomlConfig, MatrixConfig, MatrixType, SplitConfig};

#[derive(Clone, Debug)]
pub enum BoardConfig {
    Split(SplitConfig),
    UniBody(UniBodyConfig),
}

#[derive(Clone, Debug, Default)]
pub struct UniBodyConfig {
    pub matrix: MatrixConfig,
    pub input_device: InputDeviceConfig,
}

impl Default for BoardConfig {
    fn default() -> Self {
        BoardConfig::UniBody(UniBodyConfig::default())
    }
}

impl BoardConfig {
    /// Get the number of encoders for each board
    ///
    /// - If the board is the unibody board, the returned vector has only one element.
    /// - If the board is the split board, the number of elements is the number of peripherals + 1 (central),
    ///   where the first element is the number of encoders on the central.
    pub fn get_num_encoder(&self) -> Vec<usize> {
        let mut num_encoder = Vec::new();
        match self {
            BoardConfig::Split(split) => {
                // Central's encoders
                num_encoder.push(
                    split
                        .central
                        .input_device
                        .clone()
                        .unwrap_or_default()
                        .encoder
                        .unwrap_or(Vec::new())
                        .len(),
                );

                // Peripheral's encoders
                for peri in &split.peripheral {
                    num_encoder.push(
                        peri.input_device
                            .clone()
                            .unwrap_or_default()
                            .encoder
                            .unwrap_or(Vec::new())
                            .len(),
                    );
                }
            }
            BoardConfig::UniBody(uni_body_config) => {
                num_encoder.push(uni_body_config.input_device.encoder.clone().unwrap_or(Vec::new()).len());
            }
        };
        num_encoder
    }
}

impl KeyboardTomlConfig {
    pub fn get_board_config(&self) -> Result<BoardConfig, String> {
        let matrix = self.matrix.clone();
        let split = self.split.clone();
        let input_device = self.input_device.clone();
        match (matrix, split) {
            (None, Some(s)) => {
                Ok(BoardConfig::Split(s))
            },
            (Some(m), None) => {
                match m.matrix_type {
                    MatrixType::normal => {
                        if m.input_pins.is_none() || m.output_pins.is_none() {
                            return Err("`input_pins` and `output_pins` is required for normal matrix".to_string());
                        }
                    },
                    MatrixType::direct_pin => {
                        if m.direct_pins.is_none() {
                            return Err("`direct_pins` is required for direct pin matrix".to_string());
                        }
                    },
                }
                // FIXME: input device for split keyboard is not supported yet
                Ok(BoardConfig::UniBody(UniBodyConfig{matrix: m, input_device: input_device.unwrap_or_default()}))
            },
            (None, None) => Err("[matrix] section in keyboard.toml is required for non-split keyboard".to_string()),
            _ => Err("Use at most one of [matrix] or [split] in your keyboard.toml!\n-> [matrix] is used to define a normal matrix of non-split keyboard\n-> [split] is used to define a split keyboard\n".to_string()),
        }
    }
}
