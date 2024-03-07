// Recursive expansion of gatt_service macro
// ==========================================

pub struct HidService2 {
    input_report_value_handle: u16,
    input_report_cccd_handle: u16,
    hid_info_value_handle: u16,
    report_map_value_handle: u16,
    protocol_mode_value_handle: u16,
    hid_control_value_handle: u16,
}
#[allow(unused)]
impl HidService2 {
    pub fn new(
        sd: &mut ::nrf_softdevice::Softdevice,
    ) -> Result<Self, ::nrf_softdevice::ble::gatt_server::RegisterError> {
        let mut service_builder = ::nrf_softdevice::ble::gatt_server::builder::ServiceBuilder::new(
            sd,
            ::nrf_softdevice::ble::Uuid::new_16(6162u16),
        )?;
        let input_report = {
            let val = [0u8, 1u8];
            let mut attr = ::nrf_softdevice::ble::gatt_server::characteristic::Attribute::new(&val);
            if <[u8; 8] as ::nrf_softdevice::ble::GattValue>::MAX_SIZE
                != <[u8; 8] as ::nrf_softdevice::ble::GattValue>::MIN_SIZE
            {
                attr = attr
                    .variable_len(<[u8; 8] as ::nrf_softdevice::ble::GattValue>::MAX_SIZE as u16);
            }
            attr = attr
                .read_security(::nrf_softdevice::ble::SecurityMode::JustWorks)
                .write_security(::nrf_softdevice::ble::SecurityMode::JustWorks);
            let props = ::nrf_softdevice::ble::gatt_server::characteristic::Properties {
                read: true,
                write: true,
                write_without_response: false,
                notify: true,
                indicate: false,
                ..Default::default()
            };
            let metadata = ::nrf_softdevice::ble::gatt_server::characteristic::Metadata::new(props);
            let mut cb = service_builder.add_characteristic(
                ::nrf_softdevice::ble::Uuid::new_16(10829u16),
                attr,
                metadata,
            )?;
            let _ = cb.add_descriptor(
                ::nrf_softdevice::ble::Uuid::new_16(10504u16),
                ::nrf_softdevice::ble::gatt_server::characteristic::Attribute::new([0, 1])
                    .security(::nrf_softdevice::ble::SecurityMode::JustWorks)
                    .variable_len([0, 1].len() as u16),
            )?;
            cb.build()
        };
        let hid_info = {
            let val = [0x1, 0x1, 0x0, 0x03];
            let mut attr = ::nrf_softdevice::ble::gatt_server::characteristic::Attribute::new(&val);
            if <u8 as ::nrf_softdevice::ble::GattValue>::MAX_SIZE
                != <u8 as ::nrf_softdevice::ble::GattValue>::MIN_SIZE
            {
                attr = attr.variable_len(<u8 as ::nrf_softdevice::ble::GattValue>::MAX_SIZE as u16);
            }
            attr = attr
                .read_security(::nrf_softdevice::ble::SecurityMode::JustWorks)
                .write_security(::nrf_softdevice::ble::SecurityMode::JustWorks);
            let props = ::nrf_softdevice::ble::gatt_server::characteristic::Properties {
                read: true,
                write: false,
                write_without_response: false,
                notify: false,
                indicate: false,
                ..Default::default()
            };
            let metadata = ::nrf_softdevice::ble::gatt_server::characteristic::Metadata::new(props);
            let mut cb = service_builder.add_characteristic(
                ::nrf_softdevice::ble::Uuid::new_16(10826u16),
                attr,
                metadata,
            )?;
            cb.build()
        };
        let report_map = {
            let val = BleKeyboardReport::desc();
            let mut attr = ::nrf_softdevice::ble::gatt_server::characteristic::Attribute::new(&val);
            if <[u8; 71] as ::nrf_softdevice::ble::GattValue>::MAX_SIZE
                != <[u8; 71] as ::nrf_softdevice::ble::GattValue>::MIN_SIZE
            {
                attr = attr
                    .variable_len(<[u8; 71] as ::nrf_softdevice::ble::GattValue>::MAX_SIZE as u16);
            }
            attr = attr
                .read_security(::nrf_softdevice::ble::SecurityMode::JustWorks)
                .write_security(::nrf_softdevice::ble::SecurityMode::JustWorks);
            let props = ::nrf_softdevice::ble::gatt_server::characteristic::Properties {
                read: true,
                write: false,
                write_without_response: false,
                notify: false,
                indicate: false,
                ..Default::default()
            };
            let metadata = ::nrf_softdevice::ble::gatt_server::characteristic::Metadata::new(props);
            let mut cb = service_builder.add_characteristic(
                ::nrf_softdevice::ble::Uuid::new_16(10827u16),
                attr,
                metadata,
            )?;
            cb.build()
        };
        let protocol_mode = {
            let val = [1u8];
            let mut attr = ::nrf_softdevice::ble::gatt_server::characteristic::Attribute::new(&val);
            if <[u8; 1] as ::nrf_softdevice::ble::GattValue>::MAX_SIZE
                != <[u8; 1] as ::nrf_softdevice::ble::GattValue>::MIN_SIZE
            {
                attr = attr
                    .variable_len(<[u8; 1] as ::nrf_softdevice::ble::GattValue>::MAX_SIZE as u16);
            }
            attr = attr
                .read_security(::nrf_softdevice::ble::SecurityMode::JustWorks)
                .write_security(::nrf_softdevice::ble::SecurityMode::JustWorks);
            let props = ::nrf_softdevice::ble::gatt_server::characteristic::Properties {
                read: true,
                write: false,
                write_without_response: true,
                notify: false,
                indicate: false,
                ..Default::default()
            };
            let metadata = ::nrf_softdevice::ble::gatt_server::characteristic::Metadata::new(props);
            let mut cb = service_builder.add_characteristic(
                ::nrf_softdevice::ble::Uuid::new_16(10830u16),
                attr,
                metadata,
            )?;
            cb.build()
        };
        let hid_control = {
            let val = [0u8];
            let mut attr = ::nrf_softdevice::ble::gatt_server::characteristic::Attribute::new(&val);
            if <[u8; 1] as ::nrf_softdevice::ble::GattValue>::MAX_SIZE
                != <[u8; 1] as ::nrf_softdevice::ble::GattValue>::MIN_SIZE
            {
                attr = attr
                    .variable_len(<[u8; 1] as ::nrf_softdevice::ble::GattValue>::MAX_SIZE as u16);
            }
            attr = attr
                .read_security(::nrf_softdevice::ble::SecurityMode::JustWorks)
                .write_security(::nrf_softdevice::ble::SecurityMode::JustWorks);
            let props = ::nrf_softdevice::ble::gatt_server::characteristic::Properties {
                read: true,
                write: false,
                write_without_response: true,
                notify: false,
                indicate: false,
                ..Default::default()
            };
            let metadata = ::nrf_softdevice::ble::gatt_server::characteristic::Metadata::new(props);
            let mut cb = service_builder.add_characteristic(
                ::nrf_softdevice::ble::Uuid::new_16(10828u16),
                attr,
                metadata,
            )?;
            cb.build()
        };
        let _ = service_builder.build();
        Ok(Self {
            input_report_value_handle: input_report.value_handle,
            input_report_cccd_handle: input_report.cccd_handle,
            hid_info_value_handle: hid_info.value_handle,
            report_map_value_handle: report_map.value_handle,
            protocol_mode_value_handle: protocol_mode.value_handle,
            hid_control_value_handle: hid_control.value_handle,
        })
    }
    pub fn input_report_get(
        &self,
    ) -> Result<[u8; 8], ::nrf_softdevice::ble::gatt_server::GetValueError> {
        let sd = unsafe { ::nrf_softdevice::Softdevice::steal() };
        let buf = &mut [0u8; <[u8; 8] as ::nrf_softdevice::ble::GattValue>::MAX_SIZE];
        let size =
            ::nrf_softdevice::ble::gatt_server::get_value(sd, self.input_report_value_handle, buf)?;
        Ok(<[u8; 8] as ::nrf_softdevice::ble::GattValue>::from_gatt(
            &buf[..size],
        ))
    }
    pub fn input_report_set(
        &self,
        val: &[u8; 8],
    ) -> Result<(), ::nrf_softdevice::ble::gatt_server::SetValueError> {
        let sd = unsafe { ::nrf_softdevice::Softdevice::steal() };
        let buf = <[u8; 8] as ::nrf_softdevice::ble::GattValue>::to_gatt(val);
        ::nrf_softdevice::ble::gatt_server::set_value(sd, self.input_report_value_handle, buf)
    }
    pub fn input_report_notify(
        &self,
        conn: &::nrf_softdevice::ble::Connection,
        val: &[u8; 8],
    ) -> Result<(), ::nrf_softdevice::ble::gatt_server::NotifyValueError> {
        let buf = <[u8; 8] as ::nrf_softdevice::ble::GattValue>::to_gatt(val);
        ::nrf_softdevice::ble::gatt_server::notify_value(conn, self.input_report_value_handle, buf)
    }
    pub fn hid_info_get(&self) -> Result<u8, ::nrf_softdevice::ble::gatt_server::GetValueError> {
        let sd = unsafe { ::nrf_softdevice::Softdevice::steal() };
        let buf = &mut [0u8; <u8 as ::nrf_softdevice::ble::GattValue>::MAX_SIZE];
        let size =
            ::nrf_softdevice::ble::gatt_server::get_value(sd, self.hid_info_value_handle, buf)?;
        Ok(<u8 as ::nrf_softdevice::ble::GattValue>::from_gatt(
            &buf[..size],
        ))
    }
    pub fn hid_info_set(
        &self,
        val: &u8,
    ) -> Result<(), ::nrf_softdevice::ble::gatt_server::SetValueError> {
        let sd = unsafe { ::nrf_softdevice::Softdevice::steal() };
        let buf = <u8 as ::nrf_softdevice::ble::GattValue>::to_gatt(val);
        ::nrf_softdevice::ble::gatt_server::set_value(sd, self.hid_info_value_handle, buf)
    }
    pub fn report_map_get(
        &self,
    ) -> Result<[u8; 71], ::nrf_softdevice::ble::gatt_server::GetValueError> {
        let sd = unsafe { ::nrf_softdevice::Softdevice::steal() };
        let buf = &mut [0u8; <[u8; 71] as ::nrf_softdevice::ble::GattValue>::MAX_SIZE];
        let size =
            ::nrf_softdevice::ble::gatt_server::get_value(sd, self.report_map_value_handle, buf)?;
        Ok(<[u8; 71] as ::nrf_softdevice::ble::GattValue>::from_gatt(
            &buf[..size],
        ))
    }
    pub fn report_map_set(
        &self,
        val: &[u8; 71],
    ) -> Result<(), ::nrf_softdevice::ble::gatt_server::SetValueError> {
        let sd = unsafe { ::nrf_softdevice::Softdevice::steal() };
        let buf = <[u8; 71] as ::nrf_softdevice::ble::GattValue>::to_gatt(val);
        ::nrf_softdevice::ble::gatt_server::set_value(sd, self.report_map_value_handle, buf)
    }
    pub fn protocol_mode_get(
        &self,
    ) -> Result<[u8; 1], ::nrf_softdevice::ble::gatt_server::GetValueError> {
        let sd = unsafe { ::nrf_softdevice::Softdevice::steal() };
        let buf = &mut [0u8; <[u8; 1] as ::nrf_softdevice::ble::GattValue>::MAX_SIZE];
        let size = ::nrf_softdevice::ble::gatt_server::get_value(
            sd,
            self.protocol_mode_value_handle,
            buf,
        )?;
        Ok(<[u8; 1] as ::nrf_softdevice::ble::GattValue>::from_gatt(
            &buf[..size],
        ))
    }
    pub fn protocol_mode_set(
        &self,
        val: &[u8; 1],
    ) -> Result<(), ::nrf_softdevice::ble::gatt_server::SetValueError> {
        let sd = unsafe { ::nrf_softdevice::Softdevice::steal() };
        let buf = <[u8; 1] as ::nrf_softdevice::ble::GattValue>::to_gatt(val);
        ::nrf_softdevice::ble::gatt_server::set_value(sd, self.protocol_mode_value_handle, buf)
    }
    pub fn hid_control_get(
        &self,
    ) -> Result<[u8; 1], ::nrf_softdevice::ble::gatt_server::GetValueError> {
        let sd = unsafe { ::nrf_softdevice::Softdevice::steal() };
        let buf = &mut [0u8; <[u8; 1] as ::nrf_softdevice::ble::GattValue>::MAX_SIZE];
        let size =
            ::nrf_softdevice::ble::gatt_server::get_value(sd, self.hid_control_value_handle, buf)?;
        Ok(<[u8; 1] as ::nrf_softdevice::ble::GattValue>::from_gatt(
            &buf[..size],
        ))
    }
    pub fn hid_control_set(
        &self,
        val: &[u8; 1],
    ) -> Result<(), ::nrf_softdevice::ble::gatt_server::SetValueError> {
        let sd = unsafe { ::nrf_softdevice::Softdevice::steal() };
        let buf = <[u8; 1] as ::nrf_softdevice::ble::GattValue>::to_gatt(val);
        ::nrf_softdevice::ble::gatt_server::set_value(sd, self.hid_control_value_handle, buf)
    }
}
impl ::nrf_softdevice::ble::gatt_server::Service for HidService2 {
    type Event = HidService2Event;
    fn on_write(&self, handle: u16, data: &[u8]) -> Option<Self::Event> {
        if handle == self.input_report_value_handle {
            if data.len() < <[u8; 8] as ::nrf_softdevice::ble::GattValue>::MIN_SIZE {
                return self
                    .input_report_get()
                    .ok()
                    .map(HidService2Event::InputReportWrite);
            } else {
                return Some(HidService2Event::InputReportWrite(
                    <[u8; 8] as ::nrf_softdevice::ble::GattValue>::from_gatt(data),
                ));
            }
        }
        if handle == self.input_report_cccd_handle && !data.is_empty() {
            match data[0] & 0x01 {
                0x00 => {
                    return Some(HidService2Event::InputReportCccdWrite {
                        notifications: false,
                    })
                }
                0x01 => {
                    return Some(HidService2Event::InputReportCccdWrite {
                        notifications: true,
                    })
                }
                _ => {}
            }
        }
        if handle == self.protocol_mode_value_handle {
            if data.len() < <[u8; 1] as ::nrf_softdevice::ble::GattValue>::MIN_SIZE {
                return self
                    .protocol_mode_get()
                    .ok()
                    .map(HidService2Event::ProtocolModeWrite);
            } else {
                return Some(HidService2Event::ProtocolModeWrite(
                    <[u8; 1] as ::nrf_softdevice::ble::GattValue>::from_gatt(data),
                ));
            }
        }
        if handle == self.hid_control_value_handle {
            if data.len() < <[u8; 1] as ::nrf_softdevice::ble::GattValue>::MIN_SIZE {
                return self
                    .hid_control_get()
                    .ok()
                    .map(HidService2Event::HidControlWrite);
            } else {
                return Some(HidService2Event::HidControlWrite(
                    <[u8; 1] as ::nrf_softdevice::ble::GattValue>::from_gatt(data),
                ));
            }
        }
        None
    }
}
#[allow(unused)]
pub enum HidService2Event {
    InputReportWrite([u8; 8]),
    InputReportCccdWrite { notifications: bool },
    ProtocolModeWrite([u8; 1]),
    HidControlWrite([u8; 1]),
}