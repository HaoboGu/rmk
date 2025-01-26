use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct StaticConfig {
    pub num_macros: u8
}
