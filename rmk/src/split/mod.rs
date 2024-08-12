use crate::action::KeyAction;
use crate::debounce::fast_debouncer::RapidDebouncer;
use crate::keyboard::{communication_task, Keyboard, KeyboardReportMessage};
use crate::keymap::KeyMap;
use crate::storage::Storage;
use crate::usb::KeyboardUsbDevice;
use crate::via::process::VialService;
use crate::{
    keyboard::keyboard_task,
    light::{led_hid_task, LightService},
    via::vial_task,
};
use core::cell::RefCell;
use defmt::*;
use embassy_futures::select::{select, select4, select_slice, Either4};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::Channel,
};
use embassy_time::Timer;
use embassy_usb::driver::Driver;
use embedded_hal::digital::{InputPin, OutputPin};
#[cfg(feature = "async_matrix")]
use embedded_hal_async::digital::Wait;
use embedded_storage_async::nor_flash::NorFlash as AsyncNorFlash;
use futures::pin_mut;
use master::{MasterMatrix, SlaveCache};
use rmk_config::RmkConfig;

pub(crate) mod master;
pub(crate) mod slave;
pub(crate) mod driver;

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
pub async fn initialize_split_master_and_run<
    F: AsyncNorFlash,
    D: Driver<'static>,
    #[cfg(feature = "async_matrix")] In: Wait + InputPin,
    #[cfg(not(feature = "async_matrix"))] In: InputPin,
    Out: OutputPin,
    const ROW: usize,
    const COL: usize,
    const NUM_LAYER: usize,
>(
    driver: D,
    #[cfg(feature = "col2row")] input_pins: [In; ROW],
    #[cfg(not(feature = "col2row"))] input_pins: [In; COL],
    #[cfg(feature = "col2row")] output_pins: [Out; COL],
    #[cfg(not(feature = "col2row"))] output_pins: [Out; ROW],
    flash: Option<F>,
    default_keymap: [[[KeyAction; COL]; ROW]; NUM_LAYER],
    keyboard_config: RmkConfig<'static, Out>,
) -> ! {
    // Initialize storage and keymap
    let (mut storage, keymap) = match flash {
        Some(f) => {
            let mut s = Storage::new(f, &default_keymap, keyboard_config.storage_config).await;
            let keymap = RefCell::new(
                KeyMap::<ROW, COL, NUM_LAYER>::new_from_storage(default_keymap, Some(&mut s)).await,
            );
            (Some(s), keymap)
        }
        None => {
            let keymap = RefCell::new(
                KeyMap::<ROW, COL, NUM_LAYER>::new_from_storage::<F>(default_keymap, None).await,
            );
            (None, keymap)
        }
    };

    static keyboard_channel: Channel<CriticalSectionRawMutex, KeyboardReportMessage, 8> =
        Channel::new();
    let mut keyboard_report_sender = keyboard_channel.sender();
    let mut keyboard_report_receiver = keyboard_channel.receiver();

    // Create keyboard services and devices

    // Keyboard matrix, use COL2ROW by default
    #[cfg(all(feature = "col2row", feature = "rapid_debouncer"))]
    let matrix = MasterMatrix::<In, Out, RapidDebouncer<ROW, COL>, ROW, COL, 0, 0, ROW, COL>::new(input_pins, output_pins);
    // #[cfg(all(feature = "col2row", not(feature = "rapid_debouncer")))]
    // let matrix = Matrix::<_, _, DefaultDebouncer<ROW, COL>, ROW, COL>::new(input_pins, output_pins);
    // #[cfg(all(not(feature = "col2row"), feature = "rapid_debouncer"))]
    // let matrix = Matrix::<_, _, RapidDebouncer<COL, ROW>, COL, ROW>::new(input_pins, output_pins);
    // #[cfg(all(not(feature = "col2row"), not(feature = "rapid_debouncer")))]
    // let matrix = Matrix::<_, _, DefaultDebouncer<COL, ROW>, COL, ROW>::new(input_pins, output_pins);

    // TODO: Get SLAVE_NUM and all corresponding configs from config file
    const SLAVE_NUM: usize = 1;
    let mut slave_futs: heapless::Vec<_, 8> = (0..SLAVE_NUM)
        .into_iter()
        .map(|i| {
            let slave = SlaveCache::<1, 2>::new(i);
            slave.run()
        })
        .collect();

    let (mut keyboard, mut usb_device, mut vial_service, mut light_service) = (
        Keyboard::new(matrix, &keymap),
        KeyboardUsbDevice::new(driver, keyboard_config.usb_config),
        VialService::new(&keymap, keyboard_config.vial_config),
        LightService::from_config(keyboard_config.light_config),
    );

    loop {
        // Run all tasks, if one of them fails, wait 1 second and then restart
        if let Some(ref mut _s) = storage {
            // run_usb_keyboard(
            //     &mut usb_device,
            //     &mut keyboard,
            //     s,
            //     &mut light_service,
            //     &mut vial_service,
            //     &mut keyboard_report_receiver,
            //     &mut keyboard_report_sender,
            // )
            // .await;
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
            let slave_fut = select_slice(&mut slave_futs);
            pin_mut!(usb_fut);
            pin_mut!(keyboard_fut);
            pin_mut!(led_fut);
            pin_mut!(via_fut);
            pin_mut!(communication_fut);
            match select4(
                usb_fut,
                select(keyboard_fut, communication_fut),
                select(led_fut, slave_fut),
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
