/// User-defined split event example.
///
/// This event is automatically forwarded between split keyboard halves.
/// When published on either half, it is delivered both locally and to the remote half.
use rmk::macros::event;

#[event(split = 0, subs = 4, pubs = 2)]
#[derive(
    Clone,
    Copy,
    Debug,
    serde::Serialize,
    serde::Deserialize,
    postcard::experimental::max_size::MaxSize,
)]
pub struct CustomSplitEvent {
    pub sensor_value: i16,
}
