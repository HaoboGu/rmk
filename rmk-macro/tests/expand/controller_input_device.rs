use rmk_macro::{controller, input_device};

#[derive(Clone, Copy, Debug)]
pub struct SensorEvent {
    pub value: u16,
}

#[derive(Clone, Copy, Debug)]
pub struct ConfigEvent {
    pub threshold: u16,
}

#[derive(Clone, Copy, Debug)]
pub struct ModeEvent {
    pub enabled: bool,
}

mod basic {
    use super::{ConfigEvent, SensorEvent, controller, input_device};

    #[input_device(publish = SensorEvent)]
    #[controller(subscribe = [ConfigEvent])]
    pub struct SensorController {
        pub threshold: u16,
    }
}

mod polling {
    use super::{ConfigEvent, SensorEvent, controller, input_device};

    #[input_device(publish = SensorEvent)]
    #[controller(subscribe = [ConfigEvent], poll_interval = 50)]
    pub struct PollingSensorController {
        pub threshold: u16,
        pub last_value: u16,
    }
}

mod reversed {
    use super::{ConfigEvent, SensorEvent, controller, input_device};

    #[controller(subscribe = [ConfigEvent])]
    #[input_device(publish = SensorEvent)]
    pub struct ReversedSensorController {
        pub threshold: u16,
    }
}

mod reversed_polling {
    use super::{ConfigEvent, SensorEvent, controller, input_device};

    #[controller(subscribe = [ConfigEvent], poll_interval = 50)]
    #[input_device(publish = SensorEvent)]
    pub struct ReversedPollingSensorController {
        pub threshold: u16,
        pub last_value: u16,
    }
}

mod multi_event {
    use super::{ConfigEvent, ModeEvent, SensorEvent, controller, input_device};

    #[input_device(publish = SensorEvent)]
    #[controller(subscribe = [ConfigEvent, ModeEvent])]
    pub struct MultiEventSensorController {
        pub threshold: u16,
        pub mode: bool,
    }
}

mod multi_event_polling {
    use super::{ConfigEvent, ModeEvent, SensorEvent, controller, input_device};

    #[input_device(publish = SensorEvent)]
    #[controller(subscribe = [ConfigEvent, ModeEvent], poll_interval = 40)]
    pub struct PollingMultiEventSensorController {
        pub threshold: u16,
        pub mode: bool,
        pub last_value: u16,
    }
}

mod multi_event_reversed {
    use super::{ConfigEvent, ModeEvent, SensorEvent, controller, input_device};

    #[controller(subscribe = [ConfigEvent, ModeEvent])]
    #[input_device(publish = SensorEvent)]
    pub struct ReversedMultiEventSensorController {
        pub threshold: u16,
        pub mode: bool,
    }
}

mod multi_event_reversed_polling {
    use super::{ConfigEvent, ModeEvent, SensorEvent, controller, input_device};

    #[controller(subscribe = [ConfigEvent, ModeEvent], poll_interval = 40)]
    #[input_device(publish = SensorEvent)]
    pub struct ReversedPollingMultiEventSensorController {
        pub threshold: u16,
        pub mode: bool,
        pub last_value: u16,
    }
}
