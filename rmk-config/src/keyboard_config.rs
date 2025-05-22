use crate::{
    Basic, BehaviorConfig, BoardConfig, ChipModel, CommunicationConfig, DependencyConfig, LayoutConfig, LightConfig,
    StorageConfig,
};

#[derive(Clone, Debug, Default)]
pub struct KeyboardConfig {
    pub basic: Basic,
    pub communication: CommunicationConfig,
    pub chip: ChipModel,
    pub board: BoardConfig,
    pub layout: LayoutConfig,
    pub behavior: BehaviorConfig,
    pub light: LightConfig,
    pub storage: StorageConfig,
    pub dependency: DependencyConfig,
}
