use core::cell::RefCell;

use defmt::{error, info, warn};
use embassy_futures::select::{select, select4, Either4};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_time::Timer;
use embassy_usb::driver::Driver;
use embedded_hal::digital::{InputPin, OutputPin};
#[cfg(feature = "async_matrix")]
use embedded_hal_async::digital::Wait;
use embedded_io_async::{Read, Write};
use embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash;
use futures::pin_mut;
use rmk_config::RmkConfig;

use crate::action::KeyAction;
#[cfg(not(feature = "rapid_debouncer"))]
use crate::debounce::default_bouncer::DefaultDebouncer;
#[cfg(feature = "rapid_debouncer")]
use crate::debounce::fast_debouncer::RapidDebouncer;
use crate::debounce::DebouncerTrait;
use crate::keyboard::{communication_task, Keyboard, KeyboardReportMessage};
use crate::keymap::KeyMap;
use crate::run_usb_keyboard;
use crate::split::central::CentralMatrix;
use crate::split::{
    driver::{PeripheralMatrixMonitor, SplitReader, SplitWriter},
    SplitMessage, SPLIT_MESSAGE_MAX_SIZE,
};
use crate::storage::Storage;
use crate::usb::KeyboardUsbDevice;
use crate::via::process::VialService;
use crate::{
    keyboard::keyboard_task,
    light::{led_hid_task, LightService},
    via::vial_task,
};

use super::driver::SplitDriverError;
/// Initialize and run the keyboard service, with given keyboard usb config. This function never returns.
///
/// # Arguments
///
/// * `driver` - embassy usb driver instance
/// * `input_pins` - input gpio pins
/// * `output_pins` - output gpio pins
/// * `flash` - optional **async** flash storage, which is used for storing keymap and keyboard configs
/// * `keymap` - default keymap definition
/// * `keyboard_config` - other configurations of the keyboard, check [RmkConfig] struct for details
pub(crate) async fn initialize_serial_split_central_and_run<
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    Out: OutputPin,
    F: AsyncNorFlash,
    D: Driver<'static>,
    const TOTAL_ROW: usize,
    const TOTAL_COL: usize,
    const CENTRAL_ROW: usize,
    const CENTRAL_COL: usize,
    const CENTRAL_ROW_OFFSET: usize,
    const CENTRAL_COL_OFFSET: usize,
    const NUM_LAYER: usize,
>(
    #[cfg(feature = "col2row")] input_pins: [In; CENTRAL_ROW],
    #[cfg(not(feature = "col2row"))] input_pins: [In; CENTRAL_COL],
    #[cfg(feature = "col2row")] output_pins: [Out; CENTRAL_COL],
    #[cfg(not(feature = "col2row"))] output_pins: [Out; CENTRAL_ROW],
    driver: D,
    flash: Option<F>,
    default_keymap: [[[KeyAction; TOTAL_COL]; TOTAL_ROW]; NUM_LAYER],
    keyboard_config: RmkConfig<'static, Out>,
) -> ! {
    // Initialize storage and keymap
    let (mut storage, keymap) = match flash {
        Some(f) => {
            let mut s = Storage::new(f, &default_keymap, keyboard_config.storage_config).await;
            let keymap = RefCell::new(
                KeyMap::<TOTAL_ROW, TOTAL_COL, NUM_LAYER>::new_from_storage(
                    default_keymap,
                    Some(&mut s),
                )
                .await,
            );
            (Some(s), keymap)
        }
        None => {
            let keymap = RefCell::new(
                KeyMap::<TOTAL_ROW, TOTAL_COL, NUM_LAYER>::new_from_storage::<F>(
                    default_keymap,
                    None,
                )
                .await,
            );
            (None, keymap)
        }
    };

    static keyboard_channel: Channel<CriticalSectionRawMutex, KeyboardReportMessage, 8> =
        Channel::new();
    let mut keyboard_report_sender = keyboard_channel.sender();
    let mut keyboard_report_receiver = keyboard_channel.receiver();

    // Keyboard matrix, use COL2ROW by default
    #[cfg(all(feature = "col2row", feature = "rapid_debouncer"))]
    let debouncer: RapidDebouncer<CENTRAL_ROW, CENTRAL_COL> = RapidDebouncer::new();
    #[cfg(all(not(feature = "col2row"), feature = "rapid_debouncer"))]
    let debouncer: RapidDebouncer<CENTRAL_COL, CENTRAL_ROW> = RapidDebouncer::new();
    #[cfg(all(feature = "col2row", not(feature = "rapid_debouncer")))]
    let debouncer: DefaultDebouncer<CENTRAL_ROW, CENTRAL_COL> = DefaultDebouncer::new();
    #[cfg(all(not(feature = "col2row"), not(feature = "rapid_debouncer")))]
    let debouncer: DefaultDebouncer<CENTRAL_COL, CENTRAL_ROW> = DefaultDebouncer::new();

    #[cfg(feature = "col2row")]
    let matrix = CentralMatrix::<
        In,
        Out,
        _,
        TOTAL_ROW,
        TOTAL_COL,
        CENTRAL_ROW_OFFSET,
        CENTRAL_COL_OFFSET,
        CENTRAL_ROW,
        CENTRAL_COL,
    >::new(input_pins, output_pins, debouncer);
    #[cfg(not(feature = "col2row"))]
    let matrix = CentralMatrix::<
        In,
        Out,
        _,
        TOTAL_ROW,
        TOTAL_COL,
        CENTRAL_ROW_OFFSET,
        CENTRAL_COL_OFFSET,
        CENTRAL_COL,
        CENTRAL_ROW,
    >::new(input_pins, output_pins, debouncer);

    // Create keyboard services and devices
    let (mut keyboard, mut usb_device, mut vial_service, mut light_service) = (
        Keyboard::new(matrix, &keymap),
        KeyboardUsbDevice::new(driver, keyboard_config.usb_config),
        VialService::new(&keymap, keyboard_config.vial_config),
        LightService::from_config(keyboard_config.light_config),
    );

    loop {
        // Run all tasks, if one of them fails, wait 1 second and then restart
        if let Some(ref mut s) = storage {
            run_usb_keyboard(
                &mut usb_device,
                &mut keyboard,
                s,
                &mut light_service,
                &mut vial_service,
                &mut keyboard_report_receiver,
                &mut keyboard_report_sender,
            )
            .await;
        } else {
            // Run 5 tasks: usb, keyboard, led, vial, communication
            let usb_fut = usb_device.device.run();
            let keyboard_fut = keyboard_task(&mut keyboard, &mut keyboard_report_sender);
            let communication_fut = communication_task(
                &mut keyboard_report_receiver,
                &mut usb_device.keyboard_hid_writer,
                &mut usb_device.other_hid_writer,
            );
            let led_fut = led_hid_task(&mut usb_device.keyboard_hid_reader, &mut light_service);
            let via_fut = vial_task(&mut usb_device.via_hid, &mut vial_service);
            // let peripheral_fut = select_slice(&mut peripheral_futs);
            pin_mut!(usb_fut);
            pin_mut!(keyboard_fut);
            pin_mut!(led_fut);
            pin_mut!(via_fut);
            pin_mut!(communication_fut);
            match select4(
                usb_fut,
                select(keyboard_fut, communication_fut),
                // select(led_fut, peripheral_fut),
                led_fut,
                via_fut,
            )
            .await
            {
                Either4::First(_) => {
                    error!("Usb task is died");
                }
                Either4::Second(_) => error!("Keyboard task is died"),
                Either4::Third(_) => error!("Led task is died"),
                Either4::Fourth(_) => error!("Via task is died"),
            }
        }

        warn!("Detected failure, restarting keyboard sevice after 1 second");
        Timer::after_secs(1).await;
    }
}

// Receive split message from peripheral via serial and process it
///
/// Generic parameters:
/// - `const ROW`: row number of the peripheral's matrix
/// - `const COL`: column number of the peripheral's matrix
/// - `const ROW_OFFSET`: row offset of the peripheral's matrix in the whole matrix
/// - `const COL_OFFSET`: column offset of the peripheral's matrix in the whole matrix
/// - `S`: a serial port that implements `Read` and `Write` trait in embedded-io-async
pub(crate) async fn run_serial_peripheral_monitor<
    const ROW: usize,
    const COL: usize,
    const ROW_OFFSET: usize,
    const COL_OFFSET: usize,
    S: Read + Write,
>(
    id: usize,
    receiver: S,
) {
    let split_serial_driver: SerialSplitDriver<S> = SerialSplitDriver::new(receiver);
    let peripheral = PeripheralMatrixMonitor::<ROW, COL, ROW_OFFSET, COL_OFFSET, _>::new(
        split_serial_driver,
        id,
    );
    info!("Running peripheral monitor {}", id);
    peripheral.run().await;
}

/// Serial driver for BOTH split central and peripheral
pub(crate) struct SerialSplitDriver<S: Read + Write> {
    serial: S,
}

impl<S: Read + Write> SerialSplitDriver<S> {
    pub(crate) fn new(serial: S) -> Self {
        Self { serial }
    }
}

impl<S: Read + Write> SplitReader for SerialSplitDriver<S> {
    async fn read(&mut self) -> Result<SplitMessage, SplitDriverError> {
        let mut buf = [0_u8; SPLIT_MESSAGE_MAX_SIZE];
        let n_bytes = self
            .serial
            .read(&mut buf)
            .await
            .map_err(|_e| SplitDriverError::SerialError)?;
        if n_bytes == 0 {
            return Err(SplitDriverError::EmptyMessage);
        }
        let message: SplitMessage = postcard::from_bytes(&buf).map_err(|e| {
            error!("Postcard deserialize split message error: {}", e);
            SplitDriverError::DeserializeError
        })?;
        Ok(message)
    }
}

impl<S: Read + Write> SplitWriter for SerialSplitDriver<S> {
    async fn write(&mut self, message: &SplitMessage) -> Result<usize, SplitDriverError> {
        let mut buf = [0_u8; SPLIT_MESSAGE_MAX_SIZE];
        let bytes = postcard::to_slice(message, &mut buf).map_err(|e| {
            error!("Postcard serialize split message error: {}", e);
            SplitDriverError::SerializeError
        })?;
        self.serial
            .write(bytes)
            .await
            .map_err(|_e| SplitDriverError::SerialError)
    }
}
