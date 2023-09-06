use usb_device::{
    class_prelude::{UsbBus, UsbBusAllocator},
    prelude::{UsbDevice, UsbDeviceBuilder, UsbVidPid},
};
use usbd_hid::{
    descriptor::{KeyboardReport, SerializedDescriptor},
    hid_class::HIDClass,
};

pub fn create_usb_device_and_hid_class<'a, B: UsbBus>(
    usb_bus: &'a UsbBusAllocator<B>,
    vid: u16,
    pid: u16,
    manufacturer: &'a str,
    product: &'a str,
    serial_numer: &'a str,
) -> (HIDClass<'a, B>, UsbDevice<'a, B>) {
    let hid = HIDClass::new(usb_bus, KeyboardReport::desc(), 10);
    let usb_dev = UsbDeviceBuilder::new(usb_bus, UsbVidPid(vid, pid))
        .manufacturer(manufacturer)
        .product(product)
        .serial_number(serial_numer)
        .build();

    (hid, usb_dev)
}
