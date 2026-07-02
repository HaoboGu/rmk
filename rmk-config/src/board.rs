use crate::{InputDeviceConfig, KeyboardTomlConfig, MatrixConfig, MatrixType, SplitConfig};

#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)]
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
    /// Get number of peripherals
    pub fn get_num_periphreal(&self) -> usize {
        match self {
            BoardConfig::Split(split_config) => split_config.peripheral.len(),
            BoardConfig::UniBody(_) => 0,
        }
    }
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

/// Check a matrix's pin counts against its declared `rows`/`cols`. `row2col` only flips
/// scan direction (In/Out pins), never the pin-list lengths, so there is no swap here.
fn validate_matrix_dims(matrix: &MatrixConfig, rows: usize, cols: usize, ctx: &str) -> Result<(), String> {
    match matrix.matrix_type {
        MatrixType::Normal => {
            let row_pins = matrix.row_pins.as_ref().map_or(0, |v| v.len());
            let col_pins = matrix.col_pins.as_ref().map_or(0, |v| v.len());
            if row_pins != rows {
                return Err(format!(
                    "keyboard.toml: {ctx} has {row_pins} row_pins but rows = {rows}"
                ));
            }
            if col_pins != cols {
                return Err(format!(
                    "keyboard.toml: {ctx} has {col_pins} col_pins but cols = {cols}"
                ));
            }
        }
        MatrixType::DirectPin => {
            let direct = matrix.direct_pins.as_ref();
            let n_rows = direct.map_or(0, |v| v.len());
            if n_rows != rows {
                return Err(format!(
                    "keyboard.toml: {ctx} direct_pins has {n_rows} rows but rows = {rows}"
                ));
            }
            if let Some(direct) = direct {
                for (r, row) in direct.iter().enumerate() {
                    if row.len() != cols {
                        return Err(format!(
                            "keyboard.toml: {ctx} direct_pins row {r} has {} pins but cols = {cols}",
                            row.len()
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

impl KeyboardTomlConfig {
    pub(crate) fn get_board_config(&self) -> Result<BoardConfig, String> {
        let matrix = self.matrix.clone();
        let split = self.split.clone();
        let input_device = self.input_device.clone();
        match (matrix, split) {
            (None, Some(s)) => {
                validate_matrix_dims(&s.central.matrix, s.central.rows, s.central.cols, "[split.central]")?;
                for (i, peri) in s.peripheral.iter().enumerate() {
                    validate_matrix_dims(&peri.matrix, peri.rows, peri.cols, &format!("[[split.peripheral]] #{i}"))?;
                }
                Ok(BoardConfig::Split(s))
            },
            (Some(m), None) => {
                match m.matrix_type {
                    MatrixType::Normal => {
                        if m.row_pins.is_none() || m.col_pins.is_none() {
                            return Err("`row_pins` and `col_pins` is required for normal matrix".to_string());
                        }
                    },
                    MatrixType::DirectPin => {
                        if m.direct_pins.is_none() {
                            return Err("`direct_pins` is required for direct pin matrix".to_string());
                        }
                    },
                }
                if let Some(layout) = &self.layout {
                    validate_matrix_dims(&m, layout.rows as usize, layout.cols as usize, "[matrix]")?;
                }
                // Top-level input_device applies only to unibody configs.
                Ok(BoardConfig::UniBody(UniBodyConfig{matrix: m, input_device: input_device.unwrap_or_default()}))
            },
            (None, None) => Err("[matrix] section in keyboard.toml is required for non-split keyboard".to_string()),
            _ => Err("Use at most one of [matrix] or [split] in your keyboard.toml!\n-> [matrix] is used to define a normal matrix of non-split keyboard\n-> [split] is used to define a split keyboard\n".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::validate_matrix_dims;
    use crate::{MatrixConfig, MatrixType};

    fn normal(rows: &[&str], cols: &[&str]) -> MatrixConfig {
        MatrixConfig {
            matrix_type: MatrixType::Normal,
            row_pins: Some(rows.iter().map(|s| s.to_string()).collect()),
            col_pins: Some(cols.iter().map(|s| s.to_string()).collect()),
            ..Default::default()
        }
    }

    #[test]
    fn normal_matrix_dims_must_match_pins() {
        let m = normal(&["r0", "r1"], &["c0", "c1", "c2"]);
        assert!(validate_matrix_dims(&m, 2, 3, "[matrix]").is_ok());
        assert!(validate_matrix_dims(&m, 3, 3, "[matrix]").is_err()); // too many rows
        assert!(validate_matrix_dims(&m, 2, 2, "[matrix]").is_err()); // too few cols
    }

    #[test]
    fn direct_pin_dims_must_match_grid() {
        let m = MatrixConfig {
            matrix_type: MatrixType::DirectPin,
            direct_pins: Some(vec![vec!["a".into(), "b".into()], vec!["c".into(), "d".into()]]),
            ..Default::default()
        };
        assert!(validate_matrix_dims(&m, 2, 2, "[matrix]").is_ok());
        assert!(validate_matrix_dims(&m, 3, 2, "[matrix]").is_err());
        assert!(validate_matrix_dims(&m, 2, 3, "[matrix]").is_err());
    }

    #[test]
    fn direct_pin_rejects_jagged_rows() {
        let m = MatrixConfig {
            matrix_type: MatrixType::DirectPin,
            direct_pins: Some(vec![vec!["a".into(), "b".into()], vec!["c".into()]]),
            ..Default::default()
        };
        assert!(validate_matrix_dims(&m, 2, 2, "[matrix]").is_err());
    }
}
