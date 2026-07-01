//! DFU events

use rmk_macro::event;
use rmk_types::dfu::DfuStatus;

/// DFU status changed event
#[event(
    channel_size = crate::DFU_STATUS_EVENT_CHANNEL_SIZE,
    pubs = crate::DFU_STATUS_EVENT_PUB_SIZE,
    subs = crate::DFU_STATUS_EVENT_SUB_SIZE
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct DfuStatusEvent(pub DfuStatus);

impl DfuStatusEvent {
    pub fn new(status: DfuStatus) -> Self {
        Self(status)
    }
}

impl_payload_wrapper!(DfuStatusEvent, DfuStatus);
