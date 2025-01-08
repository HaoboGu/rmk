/// Traits and types for HID message reporting and listening.

use core::future::Future;

use defmt::Format;
use embassy_usb::{class::hid::ReadError, driver::EndpointError};
use serde::Serialize;
use usbd_hid::descriptor::{AsInputReport, MediaKeyboardReport, MouseReport, SystemControlReport};

use crate::{usb::descriptor::KeyboardReport, CONNECTION_STATE};

#[derive(Serialize)]
pub enum Report {
    /// Normal keyboard hid report
    KeyboardReport(KeyboardReport),
    /// Mouse hid report
    MouseReport(MouseReport),
    /// Media keyboard report
    MediaKeyboardReport(MediaKeyboardReport),
    /// System control report
    SystemControlReport(SystemControlReport),
}

impl AsInputReport for Report {}

#[derive(PartialEq, Debug, Format)]
pub(crate) enum HidError {
    UsbReadError(ReadError),
    UsbEndpointError(EndpointError),
    // FIXME: remove unused errors
    UsbDisabled,
    UsbPartialRead,
    BufferOverflow,
    ReportSerializeError,
    BleDisconnected,
    BleRawError,
}

/// HidReporter trait is used for reporting HID messages to the host, via USB, BLE, etc.
///
/// 对于HidReporter来说，有两种情况，第一种是他的report是来自channel，也就是通过`self.report_receiver().receive()`获取。
/// 第二种是他的report来自其他地方，比如via里面，在同一个任务中，直接read就拿到了report，此时不再需要channel。
pub trait HidReporter {
    /// The report type that the reporter receives from input processors.
    type ReportType: AsInputReport;

    /// Get the report to be sent to the host
    fn get_report(&mut self) -> impl Future<Output = Self::ReportType>;

    /// Run the reporter task.
    fn run_reporter(&mut self) -> impl Future<Output = ()> {
        async {
            loop {
                let report = self.get_report().await;
                // Only send the report after the connection is established.
                if CONNECTION_STATE.load(core::sync::atomic::Ordering::Acquire) {
                    self.write_report(report).await;
                }
            }
        }
    }

    /// Write report to the host, return the number of bytes written if success.
    fn write_report(
        &mut self,
        report: Self::ReportType,
    ) -> impl Future<Output = Result<usize, HidError>>;
}

/// HidListener trait is used for listening to HID messages from the host, via USB, BLE, etc.
///
/// HidListener only receives `[u8; READ_N]`, the raw HID report from the host.
/// Then processes the received message, forward to other tasks
pub trait HidListener<const READ_N: usize> {
    /// Report type
    type ReportType;

    /// Read HID report from the host
    fn read_report(&mut self) -> impl Future<Output = Result<[u8; READ_N], HidError>>;

    /// Process the received HID raw report and deserialize it.
    fn process_report(&mut self, report: [u8; READ_N]) -> impl Future<Output = Self::ReportType>;
}

// /// Type of connection
// pub(crate) enum ConnectionType {
//     Usb,
//     Ble,
// }

// /// Trait for getting connection type
// pub(crate) trait ConnectionTypeWrapper {
//     fn get_conn_type(&self) -> ConnectionType;
// }

// /// Wrapper trait for hid reading
// pub(crate) trait HidReaderWrapper: ConnectionTypeWrapper {
//     async fn read(&mut self, buf: &mut [u8]) -> Result<usize, HidError>;
// }

// /// Wrapper trait for hid writing
// pub(crate) trait HidWriterWrapper: ConnectionTypeWrapper {
//     async fn write_serialize<IR: AsInputReport>(&mut self, r: &IR) -> Result<(), HidError>;
//     async fn write(&mut self, report: &[u8]) -> Result<(), HidError>;
// }

// pub(crate) trait HidReaderWriterWrapper: HidReaderWrapper + HidWriterWrapper {}
// impl<T: HidReaderWrapper + HidWriterWrapper> HidReaderWriterWrapper for T {}

// /// Wrapper struct for writing via USB
// pub(crate) struct UsbHidWriter<'d, D: Driver<'d>, const N: usize> {
//     pub(crate) usb_writer: HidWriter<'d, D, N>,
// }

// impl<'d, D: Driver<'d>, const N: usize> ConnectionTypeWrapper for UsbHidWriter<'d, D, N> {
//     fn get_conn_type(&self) -> ConnectionType {
//         ConnectionType::Usb
//     }
// }

// impl<'d, D: Driver<'d>, const N: usize> HidWriterWrapper for UsbHidWriter<'d, D, N> {
//     async fn write_serialize<IR: AsInputReport>(&mut self, r: &IR) -> Result<(), HidError> {
//         self.usb_writer
//             .write_serialize(r)
//             .await
//             .map_err(|e| match e {
//                 embassy_usb::driver::EndpointError::BufferOverflow => HidError::BufferOverflow,
//                 embassy_usb::driver::EndpointError::Disabled => HidError::UsbDisabled,
//             })
//     }

//     async fn write(&mut self, report: &[u8]) -> Result<(), HidError> {
//         self.usb_writer.write(report).await.map_err(|e| match e {
//             embassy_usb::driver::EndpointError::BufferOverflow => HidError::BufferOverflow,
//             embassy_usb::driver::EndpointError::Disabled => HidError::UsbDisabled,
//         })
//     }
// }

// impl<'d, D: Driver<'d>, const N: usize> UsbHidWriter<'d, D, N> {
//     pub(crate) fn new(usb_writer: HidWriter<'d, D, N>) -> Self {
//         Self { usb_writer }
//     }
// }

// /// Wrapper struct for reading via USB
// pub(crate) struct UsbHidReader<'d, D: Driver<'d>, const N: usize> {
//     usb_reader: HidReader<'d, D, N>,
// }

// impl<'d, D: Driver<'d>, const N: usize> ConnectionTypeWrapper for UsbHidReader<'d, D, N> {
//     fn get_conn_type(&self) -> ConnectionType {
//         ConnectionType::Usb
//     }
// }

// impl<'d, D: Driver<'d>, const N: usize> HidReaderWrapper for UsbHidReader<'d, D, N> {
//     async fn read(&mut self, buf: &mut [u8]) -> Result<usize, HidError> {
//         self.usb_reader.read(buf).await.map_err(|e| match e {
//             ReadError::BufferOverflow => HidError::BufferOverflow,
//             ReadError::Disabled => HidError::UsbDisabled,
//             ReadError::Sync(_) => HidError::UsbPartialRead,
//         })
//     }
// }

// impl<'d, D: Driver<'d>, const N: usize> UsbHidReader<'d, D, N> {
//     pub(crate) fn new(usb_reader: HidReader<'d, D, N>) -> Self {
//         Self { usb_reader }
//     }
// }

// /// Wrapper struct for reading and writing via USB
// pub(crate) struct UsbHidReaderWriter<'d, D: Driver<'d>, const READ_N: usize, const WRITE_N: usize> {
//     usb_reader_writer: HidReaderWriter<'d, D, READ_N, WRITE_N>,
// }

// impl<'d, D: Driver<'d>, const READ_N: usize, const WRITE_N: usize>
//     UsbHidReaderWriter<'d, D, READ_N, WRITE_N>
// {
//     pub(crate) fn new(usb_reader_writer: HidReaderWriter<'d, D, READ_N, WRITE_N>) -> Self {
//         Self { usb_reader_writer }
//     }
// }

// impl<'d, D: Driver<'d>, const READ_N: usize, const WRITE_N: usize> ConnectionTypeWrapper
//     for UsbHidReaderWriter<'d, D, READ_N, WRITE_N>
// {
//     fn get_conn_type(&self) -> ConnectionType {
//         ConnectionType::Usb
//     }
// }

// impl<'d, D: Driver<'d>, const READ_N: usize, const WRITE_N: usize> HidReaderWrapper
//     for UsbHidReaderWriter<'d, D, READ_N, WRITE_N>
// {
//     async fn read(&mut self, buf: &mut [u8]) -> Result<usize, HidError> {
//         self.usb_reader_writer.read(buf).await.map_err(|e| match e {
//             ReadError::BufferOverflow => HidError::BufferOverflow,
//             ReadError::Disabled => HidError::UsbDisabled,
//             ReadError::Sync(_) => HidError::UsbPartialRead,
//         })
//     }
// }

// impl<'d, D: Driver<'d>, const READ_N: usize, const WRITE_N: usize> ConnectionTypeWrapper
//     for HidReaderWriter<'d, D, READ_N, WRITE_N>
// {
//     fn get_conn_type(&self) -> ConnectionType {
//         ConnectionType::Usb
//     }
// }

// impl<'d, D: Driver<'d>, const READ_N: usize, const WRITE_N: usize> HidWriterWrapper
//     for HidReaderWriter<'d, D, READ_N, WRITE_N>
// {
//     async fn write_serialize<IR: AsInputReport>(&mut self, r: &IR) -> Result<(), HidError> {
//         self.write_serialize(r).await.map_err(|e| match e {
//             embassy_usb::driver::EndpointError::BufferOverflow => HidError::BufferOverflow,
//             embassy_usb::driver::EndpointError::Disabled => HidError::UsbDisabled,
//         })
//     }

//     async fn write(&mut self, report: &[u8]) -> Result<(), HidError> {
//         self.write(report).await.map_err(|e| match e {
//             embassy_usb::driver::EndpointError::BufferOverflow => HidError::BufferOverflow,
//             embassy_usb::driver::EndpointError::Disabled => HidError::UsbDisabled,
//         })
//     }
// }

// impl<'d, D: Driver<'d>, const READ_N: usize, const WRITE_N: usize> HidWriterWrapper
//     for UsbHidReaderWriter<'d, D, READ_N, WRITE_N>
// {
//     async fn write_serialize<IR: AsInputReport>(&mut self, r: &IR) -> Result<(), HidError> {
//         self.usb_reader_writer
//             .write_serialize(r)
//             .await
//             .map_err(|e| match e {
//                 embassy_usb::driver::EndpointError::BufferOverflow => HidError::BufferOverflow,
//                 embassy_usb::driver::EndpointError::Disabled => HidError::UsbDisabled,
//             })
//     }

//     async fn write(&mut self, report: &[u8]) -> Result<(), HidError> {
//         self.usb_reader_writer
//             .write(report)
//             .await
//             .map_err(|e| match e {
//                 embassy_usb::driver::EndpointError::BufferOverflow => HidError::BufferOverflow,
//                 embassy_usb::driver::EndpointError::Disabled => HidError::UsbDisabled,
//             })
//     }
// }
