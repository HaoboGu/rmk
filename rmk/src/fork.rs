pub use rmk_types::fork::Fork;
pub use rmk_types::fork::ForkStateBits as StateBits;

use rmk_types::action::KeyAction;
use rmk_types::modifier::ModifierCombination;

#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ActiveFork {
    pub(crate) replacement: KeyAction, // the final replacement decision of the full fork chain
    pub(crate) suppress: ModifierCombination, // aggregate the chain's match_any modifiers here
}
