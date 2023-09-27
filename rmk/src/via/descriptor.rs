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
pub struct ViaReport {
    pub input_data: [u8; 32],
    pub output_data: [u8; 32],
}
