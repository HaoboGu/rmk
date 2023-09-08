use usb_device::{
    class_prelude::{UsbBus, UsbBusAllocator},
    prelude::{UsbDevice, UsbDeviceBuilder, UsbVidPid},
};
use usbd_hid::{
    descriptor::{KeyboardReport, SerializedDescriptor},
    hid_class::HIDClass,
};

use crate::config::KeyboardConfig;

pub fn create_usb_device_and_hid_class<'a, B: UsbBus>(
    usb_bus: &'a UsbBusAllocator<B>,
    config: &KeyboardConfig<'a>,
) -> (HIDClass<'a, B>, UsbDevice<'a, B>) {
    let hid = HIDClass::new(usb_bus, KeyboardReport::desc(), 10);
    let usb_dev = UsbDeviceBuilder::new(
        usb_bus,
        UsbVidPid(config.usb_config.vid, config.usb_config.pid),
    )
    .manufacturer(config.usb_config.manufacturer)
    .product(config.usb_config.product)
    .serial_number(config.usb_config.serial_number)
    .build();

    (hid, usb_dev)
}
