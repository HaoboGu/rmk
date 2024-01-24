use usbd_hid::descriptor::generator_prelude::*;

#[gen_hid_descriptor(
    (collection = APPLICATION, usage_page = 0xFF60, usage = 0x61) = {
        (usage = 0x62, logical_min = 0x0) = {
            #[item_settings data,variable,absolute] input_data=input;
        };
        (usage = 0x63, logical_min = 0x0) = {
            #[item_settings data,variable,absolute] output_data=output;
        };
    }
)]
pub(crate) struct ViaReport {
    pub(crate) input_data: [u8; 32],
    pub(crate) output_data: [u8; 32],
}

// TODO: Composite hid report
// KeyboardReport describes a report and its companion descriptor that can be
// used to send keyboard button presses to a host and receive the status of the
// keyboard LEDs.
// #[gen_hid_descriptor(
//     (report_id = 0x01, collection = APPLICATION, usage_page = GENERIC_DESKTOP, usage = KEYBOARD) = {
//         (usage_page = KEYBOARD, usage_min = 0xE0, usage_max = 0xE7) = {
//             #[packed_bits 8] #[item_settings data,variable,absolute] modifier=input;
//         };
//         (usage_min = 0x00, usage_max = 0xFF) = {
//             #[item_settings constant,variable,absolute] reserved=input;
//         };
//         (usage_page = LEDS, usage_min = 0x01, usage_max = 0x05) = {
//             #[packed_bits 5] #[item_settings data,variable,absolute] leds=output;
//         };
//         (usage_page = KEYBOARD, usage_min = 0x00, usage_max = 0xDD) = {
//             #[item_settings data,array,absolute] keycodes=input;
//         };
//     },
//     (report_id = 0x02, collection = APPLICATION, usage_page = CONSUMER, usage = CONSUMER_CONTROL) = {
//         (usage_page = CONSUMER, usage_min = 0x00, usage_max = 0x514) = {
//             #[item_settings data,array,absolute,not_null] usage_id=input;
//         };
//     },
//     (report_id = 0x03, collection = APPLICATION, usage_page = GENERIC_DESKTOP, usage = SYSTEM_CONTROL) = {
//         (usage_min = 0x81, usage_max = 0xB7, logical_min = 1) = {
//             #[item_settings data,array,absolute,not_null] usage_id=input;
//         };
//     }
// )]
// #[allow(dead_code)]
// pub struct MyKeyboardReport {
//     pub(crate) modifier: u8,
//     pub(crate) reserved: u8,
//     pub(crate) leds: u8,
//     pub(crate) keycodes: [u8; 6],
//     pub(crate) usage_id: u16,
// }

// impl AsInputReport for MyKeyboardReport {

// }
