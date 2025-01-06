use core::{future::Future, marker::PhantomData};

use defmt::error;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Receiver};
use embassy_usb::{class::hid::HidWriter, driver::Driver, Builder};
use usbd_hid::descriptor::{MediaKeyboardReport, MouseReport, SystemControlReport};

use crate::{
    hid::{self, HidWriterWrapper, UsbHidWriter},
    keyboard::{write_other_report_to_host, KEYBOARD_REPORT_CHANNEL, KEY_REPORT_CHANNEL},
    usb::{
        build_usb_writer,
        descriptor::{CompositeReport, KeyboardReport},
    },
    CONNECTION_STATE, REPORT_CHANNEL_SIZE,
};
pub enum Report {
    /// Normal keyboard hid report
    KeyboardReport(KeyboardReport),
    /// Composite keyboard report: mouse + media(consumer) + system control
    CompositeReport(CompositeReport),
    /// Mouse hid report
    MouseReport(MouseReport),
    /// Media keyboard report
    MediaKeyboardReport(MediaKeyboardReport),
    /// System control report
    SystemControlReport(SystemControlReport),
}



/// Reporter
/// 现在reporter是两个思路
/// 1. 和Processor一样，一个reporter处理一个report类型，对应的就是一个channel。但是这样的话，在发送的时候需要把每个USB/BLE writer放到每个reporter里面。这样其实带来一个问题，就是对于同一种reporter，必须得知道同时兼容的USB/BLE
/// 2. 用一个通用的report，接受所有的report类型，然后这个reporter根据这次的report类型进行分发。类似现在KeyboardUsbDevice的逻辑。这个缺点就是如何让用户自定义是一个问题，因为这样的话就必须需要修改这个通用的reporter的代码，添加新的report类型到channel中来。
/// 
/// 问题是：report需不需要和USB/BLE 绑定，也就是说， USB Keyboard Reporter 和 BLE Keyboard Reporter 是不是应该是两个不同的实现？
/// 
/// 因为事实上，真正的HIDWriter应该是两种，并且两种同时生效（USB和BLE需要可以切换）。
///
/// Reporter trait is used for reporting HID messages to the host, via USB, BLE, etc.
pub trait Reporter {
    /// The report type that the reporter receives from input processors.
    /// It should be a variant of the `Report` enum.
    type ReportType;

    /// Get the report receiver for the reporter.
    fn report_receiver(
        &self,
    ) -> Receiver<CriticalSectionRawMutex, Self::ReportType, REPORT_CHANNEL_SIZE>;

    /// Run the reporter task.
    fn run(&mut self) -> impl Future<Output = ()> {
        async {
            loop {
                let report = self.report_receiver().receive().await;
                // Only send the report after the connection is established.
                if CONNECTION_STATE.load(core::sync::atomic::Ordering::Acquire) {
                    self.write_report(report).await;
                }
            }
        }
    }

    /// Write report to the host
    fn write_report(&mut self, report: Self::ReportType) -> impl Future<Output = ()>;
}

pub trait UsbReporterTrait: Reporter {
    fn new_f<'d, D: Driver<'d>>(usb_builder: &mut Builder<'d, D>) -> Self;
}

pub(crate) struct KeyReporter<W: HidWriterWrapper> {
    keyboard_hid_reporter: W,
}

impl<W: HidWriterWrapper> Reporter for KeyReporter<W> {
    type ReportType = KeyboardReport;

    fn report_receiver(
        &self,
    ) -> Receiver<CriticalSectionRawMutex, Self::ReportType, REPORT_CHANNEL_SIZE> {
        KEY_REPORT_CHANNEL.receiver()
    }

    async fn write_report(&mut self, report: Self::ReportType) {
        match self.keyboard_hid_reporter.write_serialize(&report).await {
            Ok(()) => {}
            Err(e) => error!("Send keyboard report error: {}", e),
        };
    }
}



impl<'d, D: Driver<'d>, const N: usize> Reporter for HidWriter<'d, D, N> {
    type ReportType = KeyboardReport;

    fn report_receiver(
        &self,
    ) -> Receiver<CriticalSectionRawMutex, Self::ReportType, REPORT_CHANNEL_SIZE> {
        KEY_REPORT_CHANNEL.receiver()
    }

    async fn write_report(&mut self, report: Self::ReportType) {
        match self.write_serialize(&report).await {
            Ok(()) => {}
            Err(e) => error!("Send keyboard report error: {}", e),
        };
    }
}

impl<'d, D: Driver<'d>, const N: usize> UsbReporterTrait for HidWriter<'d, D, N> {
    fn new_f(usb_builder: &mut Builder<'d, D>) -> Self {
        build_usb_writer::<D, KeyboardReport, 8>(usb_builder)
        // Self {
        // keyboard_hid_reporter
        // }
    }
}
