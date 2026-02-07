use rmk_macro::{controller, input_device};

#[derive(Clone, Copy, Debug)]
pub struct SensorEvent {
    pub value: u16,
}

#[derive(Clone, Copy, Debug)]
pub struct ConfigEvent {
    pub threshold: u16,
}

/// Test case: combined #[input_device] + #[controller] on the same struct.
/// This tests the runnable marker logic and select_biased! generation.
#[input_device(publish = SensorEvent)]
#[controller(subscribe = [ConfigEvent])]
pub struct SensorController {
    pub threshold: u16,
}

/// Test case: combined #[input_device] + #[controller] with polling.
/// This tests the timer arm placement in select_biased! (timer should be first).
#[input_device(publish = SensorEvent)]
#[controller(subscribe = [ConfigEvent], poll_interval = 50)]
pub struct PollingSensorController {
    pub threshold: u16,
    pub last_value: u16,
}
