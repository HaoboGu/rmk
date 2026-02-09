use rmk_macro::{controller, input_processor};

#[derive(Clone, Copy, Debug)]
pub struct KeyEvent {
    pub row: u8,
    pub col: u8,
    pub pressed: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct EncoderEvent {
    pub index: u8,
    pub direction: i8,
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
    use super::{ConfigEvent, EncoderEvent, KeyEvent, controller, input_processor};

    #[input_processor(subscribe = [KeyEvent, EncoderEvent])]
    #[controller(subscribe = [ConfigEvent])]
    pub struct HybridProcessorController;
}

mod polling {
    use super::{ConfigEvent, EncoderEvent, KeyEvent, controller, input_processor};

    #[input_processor(subscribe = [KeyEvent, EncoderEvent])]
    #[controller(subscribe = [ConfigEvent], poll_interval = 20)]
    pub struct PollingHybridProcessorController;
}

mod reversed {
    use super::{ConfigEvent, EncoderEvent, KeyEvent, controller, input_processor};

    #[controller(subscribe = [ConfigEvent])]
    #[input_processor(subscribe = [KeyEvent, EncoderEvent])]
    pub struct ReversedHybridProcessorController;
}

mod reversed_polling {
    use super::{ConfigEvent, EncoderEvent, KeyEvent, controller, input_processor};

    #[controller(subscribe = [ConfigEvent], poll_interval = 20)]
    #[input_processor(subscribe = [KeyEvent, EncoderEvent])]
    pub struct ReversedPollingHybridProcessorController;
}

mod multi_event {
    use super::{ConfigEvent, EncoderEvent, KeyEvent, ModeEvent, controller, input_processor};

    #[input_processor(subscribe = [KeyEvent, EncoderEvent])]
    #[controller(subscribe = [ConfigEvent, ModeEvent])]
    pub struct MultiControllerHybridProcessor;
}

mod multi_event_polling {
    use super::{ConfigEvent, EncoderEvent, KeyEvent, ModeEvent, controller, input_processor};

    #[input_processor(subscribe = [KeyEvent, EncoderEvent])]
    #[controller(subscribe = [ConfigEvent, ModeEvent], poll_interval = 20)]
    pub struct PollingMultiControllerHybridProcessor;
}

mod multi_event_reversed {
    use super::{ConfigEvent, EncoderEvent, KeyEvent, ModeEvent, controller, input_processor};

    #[controller(subscribe = [ConfigEvent, ModeEvent])]
    #[input_processor(subscribe = [KeyEvent, EncoderEvent])]
    pub struct ReversedMultiControllerHybridProcessor;
}

mod multi_event_reversed_polling {
    use super::{ConfigEvent, EncoderEvent, KeyEvent, ModeEvent, controller, input_processor};

    #[controller(subscribe = [ConfigEvent, ModeEvent], poll_interval = 20)]
    #[input_processor(subscribe = [KeyEvent, EncoderEvent])]
    pub struct ReversedPollingMultiControllerHybridProcessor;
}
