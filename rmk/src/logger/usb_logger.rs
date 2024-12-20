use core::fmt::Write as _;
use core::future::Future;

use embassy_futures::join::join;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pipe::Pipe;
use embassy_usb::class::cdc_acm::{CdcAcmClass, Receiver, Sender, State};
use embassy_usb::driver::Driver;
use log::{Metadata, Record};

/// A trait that can be implemented and then passed to the usb logger
pub trait ReceiverHandler {
    /// Data comes in from the serial port with each command and runs this function
    fn handle_data(&self, data: &[u8]) -> impl Future<Output = ()> + Send;

    /// Create a new instance of the Handler
    fn new() -> Self;
}

/// Use this Handler if you don't wish to use any handler
pub struct DummyHandler;

impl ReceiverHandler for DummyHandler {
    async fn handle_data(&self, _data: &[u8]) {}
    fn new() -> Self {
        Self {}
    }
}

/// The logger state containing buffers that must live as long as the USB peripheral.
pub struct LoggerState<'d> {
    pub(crate) state: State<'d>,
    config_descriptor: [u8; 128],
    bos_descriptor: [u8; 16],
    msos_descriptor: [u8; 256],
    control_buf: [u8; 64],
}

impl<'d> LoggerState<'d> {
    /// Create a new instance of the logger state.
    pub fn new() -> Self {
        Self {
            state: State::new(),
            config_descriptor: [0; 128],
            bos_descriptor: [0; 16],
            msos_descriptor: [0; 256],
            control_buf: [0; 64],
        }
    }
}

/// The packet size used in the usb logger, to be used with `create_future_from_class`
pub const MAX_PACKET_SIZE: u8 = 64;

/// The logger handle, which contains a pipe with configurable size for buffering log messages.
pub struct UsbLogger<const N: usize, T: ReceiverHandler + Send + Sync> {
    buffer: Pipe<CriticalSectionRawMutex, N>,
    custom_style: Option<fn(&Record, &mut Writer<'_, N>) -> ()>,
    recieve_handler: Option<T>,
}

impl<const N: usize, T: ReceiverHandler + Send + Sync> UsbLogger<N, T> {
    /// Create a new logger instance.
    pub const fn new() -> Self {
        Self {
            buffer: Pipe::new(),
            custom_style: None,
            recieve_handler: None,
        }
    }

    /// Create a new logger instance with a custom formatter.
    pub const fn with_custom_style(custom_style: fn(&Record, &mut Writer<'_, N>) -> ()) -> Self {
        Self {
            buffer: Pipe::new(),
            custom_style: Some(custom_style),
            recieve_handler: None,
        }
    }

    /// Add a command handler to the logger
    pub fn with_handler(&mut self, handler: T) {
        self.recieve_handler = Some(handler);
    }

    pub async fn run_logger_class<'d, D>(
        &self,
        sender: &mut Sender<'d, D>,
        receiver: &mut Receiver<'d, D>,
    ) where
        D: Driver<'d>,
    {
        let log_fut = async {
            let mut rx: [u8; MAX_PACKET_SIZE as usize] = [0; MAX_PACKET_SIZE as usize];
            sender.wait_connection().await;
            loop {
                let len = self.buffer.read(&mut rx[..]).await;
                let _ = sender.write_packet(&rx[..len]).await;
                if len as u8 == MAX_PACKET_SIZE {
                    let _ = sender.write_packet(&[]).await;
                }
            }
        };
        let reciever_fut = async {
            let mut reciever_buf: [u8; MAX_PACKET_SIZE as usize] = [0; MAX_PACKET_SIZE as usize];
            receiver.wait_connection().await;
            loop {
                if let Ok(n) = receiver.read_packet(&mut reciever_buf).await {
                    match &self.recieve_handler {
                        Some(handler) => {
                            let data = &reciever_buf[..n];
                            handler.handle_data(data).await;
                        }
                        None => (),
                    }
                };
            }
        };

        join(log_fut, reciever_fut).await;
    }

    /// Creates the futures needed for the logger from a given class
    /// This can be used in cases where the usb device is already in use for another connection
    pub async fn create_future_from_class<'d, D>(&'d self, class: CdcAcmClass<'d, D>)
    where
        D: Driver<'d>,
    {
        let (mut sender, mut receiver) = class.split();
        loop {
            self.run_logger_class(&mut sender, &mut receiver).await;
        }
    }
}

impl<const N: usize, T: ReceiverHandler + Send + Sync> log::Log for UsbLogger<N, T> {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            if let Some(custom_style) = self.custom_style {
                custom_style(record, &mut Writer(&self.buffer));
            } else {
                let _ = write!(Writer(&self.buffer), "{}\r\n", record.args());
            }
        }
    }

    fn flush(&self) {}
}

/// A writer that writes to the USB logger buffer.
pub struct Writer<'d, const N: usize>(&'d Pipe<CriticalSectionRawMutex, N>);

impl<'d, const N: usize> core::fmt::Write for Writer<'d, N> {
    fn write_str(&mut self, s: &str) -> Result<(), core::fmt::Error> {
        // The Pipe is implemented in such way that we cannot
        // write across the wraparound discontinuity.
        let b = s.as_bytes();
        if let Ok(n) = self.0.try_write(b) {
            if n < b.len() {
                // We wrote some data but not all, attempt again
                // as the reason might be a wraparound in the
                // ring buffer, which resolves on second attempt.
                let _ = self.0.try_write(&b[n..]);
            }
        }
        Ok(())
    }
}
