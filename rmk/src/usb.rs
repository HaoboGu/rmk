use log::{error, info};
use usb_device::{
    class_prelude::{UsbBus, UsbBusAllocator},
    prelude::{UsbDevice, UsbDeviceBuilder, UsbVidPid},
    UsbError,
};
use usbd_hid::{
    descriptor::{KeyboardReport, MediaKeyboardReport, SerializedDescriptor, SystemControlReport},
    hid_class::{
        HIDClass, HidClassSettings, HidCountryCode, HidProtocol, HidSubClass, ProtocolModeConfig,
    },
};

use crate::{config::KeyboardConfig, via::ViaReport};

pub struct KeyboardUsbDevice<'a, B: UsbBus> {
    /// Usb hid instance
    hid: HIDClass<'a, B>,
    /// Consumer control hid instance
    consumer_control_hid: HIDClass<'a, B>,
    /// System control hid instance
    system_control_hid: HIDClass<'a, B>,
    /// Via communication instance
    via_hid: HIDClass<'a, B>,
    /// Usb device instance
    usb_device: UsbDevice<'a, B>,
}

impl<'a, B: UsbBus> KeyboardUsbDevice<'a, B> {
    pub fn new(usb_allocator: &'a UsbBusAllocator<B>, config: &KeyboardConfig<'a>) -> Self {
        KeyboardUsbDevice {
            hid: HIDClass::new_ep_in_with_settings(
                usb_allocator,
                KeyboardReport::desc(),
                10,
                HidClassSettings {
                    subclass: HidSubClass::Boot,
                    protocol: HidProtocol::Keyboard,
                    config: ProtocolModeConfig::ForceBoot,
                    locale: HidCountryCode::NotSupported,
                },
            ),
            consumer_control_hid: HIDClass::new_ep_in_with_settings(
                usb_allocator,
                MediaKeyboardReport::desc(),
                10,
                HidClassSettings {
                    subclass: HidSubClass::NoSubClass,
                    protocol: HidProtocol::Keyboard,
                    config: ProtocolModeConfig::DefaultBehavior,
                    locale: HidCountryCode::NotSupported,
                },
            ),
            system_control_hid: HIDClass::new_ep_in_with_settings(
                usb_allocator,
                SystemControlReport::desc(),
                10,
                HidClassSettings {
                    subclass: HidSubClass::NoSubClass,
                    protocol: HidProtocol::Keyboard,
                    config: ProtocolModeConfig::DefaultBehavior,
                    locale: HidCountryCode::NotSupported,
                },
            ),
            via_hid: HIDClass::new(usb_allocator, ViaReport::desc(), 10),
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
        self.usb_device.poll(&mut [
            &mut self.hid,
            &mut self.consumer_control_hid,
            &mut self.system_control_hid,
            &mut self.via_hid,
        ]);
    }

    /// Read via report, returns the length of the report, 0 if no report is available.
    pub fn read_via_report(&mut self, report: &mut ViaReport) -> usize {
        // Use output_data: host to device data
        match self.via_hid.pull_raw_output(&mut report.output_data) {
            Ok(l) => l,
            Err(UsbError::WouldBlock) => 0,
            Err(e) => {
                error!("Read via report error: {:?}", e);
                0
            }
        }
    }

    /// Send keyboard hid report
    pub fn send_keyboard_report(&self, report: &KeyboardReport) {
        match self.hid.push_input(report) {
            Ok(_) => (),
            Err(UsbError::WouldBlock) => (),
            Err(e) => error!("Send keyboard report error: {:?}", e),
        }
    }

    /// Send consumer control report, commonly used in keyboard media control
    pub fn send_consumer_control_report(&self, report: &MediaKeyboardReport) {
        match self.consumer_control_hid.push_input(report) {
            Ok(_) => (),
            Err(UsbError::WouldBlock) => (),
            Err(e) => info!("Send consumer control report error: {:?}", e),
        }
    }

    /// Send system control report
    pub fn send_system_control_report(&self, report: &SystemControlReport) {
        match self.system_control_hid.push_input(report) {
            Ok(_) => (),
            Err(UsbError::WouldBlock) => (),
            Err(e) => error!("Send system control report error: {:?}", e),
        }
    }
}
