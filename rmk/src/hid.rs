//! A thin hid wrapper layer which supports writing/reading HID reports via USB and BLE

use defmt::Format;
use embassy_usb::{
    class::hid::{HidReader, HidReaderWriter, HidWriter, ReadError},
    driver::Driver,
};

use usbd_hid::descriptor::AsInputReport;

#[derive(PartialEq, Debug, Format)]
pub(crate) enum HidError {
    UsbDisabled,
    UsbPartialRead,
    BufferOverflow,
    ReportSerializeError,
    BleDisconnected,
    BleRawError,
}

/// Type of connection
pub(crate) enum ConnectionType {
    Usb,
    Ble,
}

/// Trait for getting connection type
pub(crate) trait ConnectionTypeWrapper {
    fn get_conn_type(&self) -> ConnectionType;
}

/// Wrapper trait for hid reading
pub(crate) trait HidReaderWrapper: ConnectionTypeWrapper {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, HidError>;
}

/// Wrapper trait for hid writing
pub(crate) trait HidWriterWrapper: ConnectionTypeWrapper {
    async fn write_serialize<IR: AsInputReport>(&mut self, r: &IR) -> Result<(), HidError>;
    async fn write(&mut self, report: &[u8]) -> Result<(), HidError>;
}

pub(crate) trait HidReaderWriterWrapper: HidReaderWrapper + HidWriterWrapper {}
impl<T: HidReaderWrapper + HidWriterWrapper> HidReaderWriterWrapper for T {}

/// Wrapper struct for writing via USB
pub(crate) struct UsbHidWriter<'d, D: Driver<'d>, const N: usize> {
    usb_writer: HidWriter<'d, D, N>,
}

impl<'d, D: Driver<'d>, const N: usize> ConnectionTypeWrapper for UsbHidWriter<'d, D, N> {
    fn get_conn_type(&self) -> ConnectionType {
        ConnectionType::Usb
    }
}

impl<'d, D: Driver<'d>, const N: usize> HidWriterWrapper for UsbHidWriter<'d, D, N> {
    async fn write_serialize<IR: AsInputReport>(&mut self, r: &IR) -> Result<(), HidError> {
        self.usb_writer
            .write_serialize(r)
            .await
            .map_err(|e| match e {
                embassy_usb::driver::EndpointError::BufferOverflow => HidError::BufferOverflow,
                embassy_usb::driver::EndpointError::Disabled => HidError::UsbDisabled,
            })
    }

    async fn write(&mut self, report: &[u8]) -> Result<(), HidError> {
        self.usb_writer.write(report).await.map_err(|e| match e {
            embassy_usb::driver::EndpointError::BufferOverflow => HidError::BufferOverflow,
            embassy_usb::driver::EndpointError::Disabled => HidError::UsbDisabled,
        })
    }
}

impl<'d, D: Driver<'d>, const N: usize> UsbHidWriter<'d, D, N> {
    pub(crate) fn new(usb_writer: HidWriter<'d, D, N>) -> Self {
        Self { usb_writer }
    }
}

/// Wrapper struct for reading via USB
pub(crate) struct UsbHidReader<'d, D: Driver<'d>, const N: usize> {
    usb_reader: HidReader<'d, D, N>,
}

impl<'d, D: Driver<'d>, const N: usize> ConnectionTypeWrapper for UsbHidReader<'d, D, N> {
    fn get_conn_type(&self) -> ConnectionType {
        ConnectionType::Usb
    }
}

impl<'d, D: Driver<'d>, const N: usize> HidReaderWrapper for UsbHidReader<'d, D, N> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, HidError> {
        self.usb_reader.read(buf).await.map_err(|e| match e {
            ReadError::BufferOverflow => HidError::BufferOverflow,
            ReadError::Disabled => HidError::UsbDisabled,
            ReadError::Sync(_) => HidError::UsbPartialRead,
        })
    }
}

impl<'d, D: Driver<'d>, const N: usize> UsbHidReader<'d, D, N> {
    pub(crate) fn new(usb_reader: HidReader<'d, D, N>) -> Self {
        Self { usb_reader }
    }
}

/// Wrapper struct for reading and writing via USB
pub(crate) struct UsbHidReaderWriter<'d, D: Driver<'d>, const READ_N: usize, const WRITE_N: usize> {
    usb_reader_writer: HidReaderWriter<'d, D, READ_N, WRITE_N>,
}

impl<'d, D: Driver<'d>, const READ_N: usize, const WRITE_N: usize>
    UsbHidReaderWriter<'d, D, READ_N, WRITE_N>
{
    pub(crate) fn new(usb_reader_writer: HidReaderWriter<'d, D, READ_N, WRITE_N>) -> Self {
        Self { usb_reader_writer }
    }
}

impl<'d, D: Driver<'d>, const READ_N: usize, const WRITE_N: usize> ConnectionTypeWrapper
    for UsbHidReaderWriter<'d, D, READ_N, WRITE_N>
{
    fn get_conn_type(&self) -> ConnectionType {
        ConnectionType::Usb
    }
}

impl<'d, D: Driver<'d>, const READ_N: usize, const WRITE_N: usize> HidReaderWrapper
    for UsbHidReaderWriter<'d, D, READ_N, WRITE_N>
{
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, HidError> {
        self.usb_reader_writer.read(buf).await.map_err(|e| match e {
            ReadError::BufferOverflow => HidError::BufferOverflow,
            ReadError::Disabled => HidError::UsbDisabled,
            ReadError::Sync(_) => HidError::UsbPartialRead,
        })
    }
}

impl<'d, D: Driver<'d>, const READ_N: usize, const WRITE_N: usize> HidWriterWrapper
    for UsbHidReaderWriter<'d, D, READ_N, WRITE_N>
{
    async fn write_serialize<IR: AsInputReport>(&mut self, r: &IR) -> Result<(), HidError> {
        self.usb_reader_writer
            .write_serialize(r)
            .await
            .map_err(|e| match e {
                embassy_usb::driver::EndpointError::BufferOverflow => HidError::BufferOverflow,
                embassy_usb::driver::EndpointError::Disabled => HidError::UsbDisabled,
            })
    }

    async fn write(&mut self, report: &[u8]) -> Result<(), HidError> {
        self.usb_reader_writer
            .write(report)
            .await
            .map_err(|e| match e {
                embassy_usb::driver::EndpointError::BufferOverflow => HidError::BufferOverflow,
                embassy_usb::driver::EndpointError::Disabled => HidError::UsbDisabled,
            })
    }
}
