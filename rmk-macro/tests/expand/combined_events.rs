use rmk_macro::{controller_event, input_event};

mod basic {
    use super::{controller_event, input_event};

    #[input_event(channel_size = 8)]
    #[controller_event(subs = 2)]
    #[derive(Clone, Copy, Debug)]
    pub struct DualChannelEvent {
        pub data: u16,
    }
}

mod reversed {
    use super::{controller_event, input_event};

    #[controller_event(subs = 2)]
    #[input_event(channel_size = 8)]
    #[derive(Clone, Copy, Debug)]
    pub struct DualChannelEventReversed {
        pub data: u16,
    }
}
