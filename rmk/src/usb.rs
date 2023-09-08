use usb_device::{
    class_prelude::{UsbBus, UsbBusAllocator},
    prelude::{UsbDevice, UsbDeviceBuilder, UsbVidPid}, UsbError,
};
use usbd_hid::{
    descriptor::{KeyboardReport, SerializedDescriptor},
    hid_class::HIDClass,
};

use crate::config::KeyboardConfig;

pub struct KeyboardUsbDevice<'a, B: UsbBus> {
    /// Usb hid device instance
    hid: HIDClass<'a, B>,
    usb_device: UsbDevice<'a, B>,
}

impl<'a, B: UsbBus> KeyboardUsbDevice<'a, B> {
    pub fn new(usb_allocator: &'a UsbBusAllocator<B>, config: &KeyboardConfig<'a>) -> Self {
        KeyboardUsbDevice {
            hid: HIDClass::new(usb_allocator, KeyboardReport::desc(), 10),
            usb_device: UsbDeviceBuilder::new(
                usb_allocator,
                UsbVidPid(config.usb_config.vid, config.usb_config.pid),
            )
            .manufacturer(config.usb_config.manufacturer)
            .product(config.usb_config.product)
            .serial_number(config.usb_config.serial_number)
            .build(),
        }
    }

    /// Usb polling
    pub fn usb_poll(&mut self) {
        self.usb_device.poll(&mut [&mut self.hid]);
    }

    /// Send keyboard hid report
    pub fn send_keyboard_report(&self, report: &KeyboardReport) {
        match self.hid.push_input(report) {
            Ok(_) => (),
            Err(UsbError::WouldBlock) => (),
            Err(_) => panic!("push raw input error"),
        }
    }
}