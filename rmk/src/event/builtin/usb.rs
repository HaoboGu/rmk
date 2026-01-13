//! USB connection events

use rmk_macro::controller_event;

use super::connection::ConnectionType;

/// Connection type changed event
#[controller_event(subs = 2)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ConnectionTypeEvent {
    pub connection_type: ConnectionType,
}
