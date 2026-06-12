#![no_std]
#![no_main]

mod keymap;
mod rgb;
mod vial;

use core::convert::Infallible;

use defmt::{info, unwrap};
use defmt_rtt as _;
use embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice;
use embassy_executor::Spawner;
use embassy_nrf::gpio::{Input, Level, Output, OutputDrive, Pull};
use embassy_nrf::interrupt::{self, InterruptExt};
use embassy_nrf::mode::Async;
use embassy_nrf::peripherals::{RNG, TWISPI0, USBD};
use embassy_nrf::saadc::{self, Input as _};
use embassy_nrf::spim::{self, Spim};
use embassy_nrf::usb::Driver;
use embassy_nrf::usb::vbus_detect::HardwareVbusDetect;
use embassy_nrf::{bind_interrupts, pac, rng, usb};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use keymap::{COL, ROW};
use nrf_mpsl::Flash;
use nrf_sdc::mpsl::MultiprotocolServiceLayer;
use nrf_sdc::{self as sdc, mpsl};
use panic_probe as _;
use rand_chacha::ChaCha12Rng;
use rand_core::SeedableRng;
use rgb::StatusRgb;
use rmk::ble::{BleTransport, build_ble_stack};
use rmk::config::{
    BehaviorConfig, BleBatteryConfig, DeviceConfig, PositionalConfig, RmkConfig, StorageConfig, VialConfig,
};
use rmk::debounce::default_debouncer::DefaultDebouncer;
use rmk::host::HostService;
use rmk::input_device::adc::{AnalogEventType, NrfAdc};
use rmk::input_device::battery::BatteryProcessor;
use rmk::keyboard::Keyboard;
use rmk::matrix::hc595_matrix::Hc595Matrix;
use rmk::processor::builtin::wpm::WpmProcessor;
use rmk::usb::UsbTransport;
use rmk::watchdog::Nrf52Watchdog;
use rmk::{HostResources, KeymapData, initialize_keymap_and_storage, run_all};
use static_cell::StaticCell;
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

bind_interrupts!(struct Irqs {
    USBD => usb::InterruptHandler<USBD>;
    SAADC => saadc::InterruptHandler;
    TWISPI0 => spim::InterruptHandler<TWISPI0>;
    RNG => rng::InterruptHandler<RNG>;
    EGU0_SWI0 => nrf_sdc::mpsl::LowPrioInterruptHandler;
    CLOCK_POWER => nrf_sdc::mpsl::ClockInterruptHandler, usb::vbus_detect::InterruptHandler;
    RADIO => nrf_sdc::mpsl::HighPrioInterruptHandler;
    TIMER0 => nrf_sdc::mpsl::HighPrioInterruptHandler;
    RTC0 => nrf_sdc::mpsl::HighPrioInterruptHandler;
});

#[embassy_executor::task]
async fn mpsl_task(mpsl: &'static MultiprotocolServiceLayer<'static>) -> ! {
    mpsl.run().await
}

const L2CAP_TXQ: u8 = 3;
const L2CAP_RXQ: u8 = 3;
const L2CAP_MTU: usize = 251;
const UNLOCK_KEYS: &[(u8, u8)] = &[(0, 0), (1, 1)];

struct NoCs;

impl embedded_hal::digital::ErrorType for NoCs {
    type Error = Infallible;
}

impl embedded_hal::digital::OutputPin for NoCs {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn build_sdc<'d, const N: usize>(
    p: nrf_sdc::Peripherals<'d>,
    rng: &'d mut rng::Rng<Async>,
    mpsl: &'d MultiprotocolServiceLayer,
    mem: &'d mut sdc::Mem<N>,
) -> Result<nrf_sdc::SoftdeviceController<'d>, nrf_sdc::Error> {
    sdc::Builder::new()?
        .support_adv()
        .support_peripheral()
        .support_dle_peripheral()
        .support_phy_update_peripheral()
        .support_le_2m_phy()
        .peripheral_count(1)?
        .buffer_cfg(L2CAP_MTU as u16, L2CAP_MTU as u16, L2CAP_TXQ, L2CAP_RXQ)?
        .build(p, rng, mpsl, mem)
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Hello Agar Mini BLE!");

    let mut nrf_config = embassy_nrf::config::Config::default();
    nrf_config.dcdc.reg0_voltage = Some(embassy_nrf::config::Reg0Voltage::_3V3);
    nrf_config.dcdc.reg0 = true;
    nrf_config.dcdc.reg1 = true;
    let p = embassy_nrf::init(nrf_config);

    let mpsl_p = mpsl::Peripherals::new(p.RTC0, p.TIMER0, p.TEMP, p.PPI_CH19, p.PPI_CH30, p.PPI_CH31);
    let lfclk_cfg = mpsl::raw::mpsl_clock_lfclk_cfg_t {
        source: mpsl::raw::MPSL_CLOCK_LF_SRC_RC as u8,
        rc_ctiv: mpsl::raw::MPSL_RECOMMENDED_RC_CTIV as u8,
        rc_temp_ctiv: mpsl::raw::MPSL_RECOMMENDED_RC_TEMP_CTIV as u8,
        accuracy_ppm: mpsl::raw::MPSL_DEFAULT_CLOCK_ACCURACY_PPM as u16,
        skip_wait_lfclk_started: mpsl::raw::MPSL_DEFAULT_SKIP_WAIT_LFCLK_STARTED != 0,
    };
    static MPSL: StaticCell<MultiprotocolServiceLayer> = StaticCell::new();
    static SESSION_MEM: StaticCell<mpsl::SessionMem<1>> = StaticCell::new();
    let mpsl = MPSL.init(unwrap!(mpsl::MultiprotocolServiceLayer::with_timeslots(
        mpsl_p,
        Irqs,
        lfclk_cfg,
        SESSION_MEM.init(mpsl::SessionMem::new())
    )));
    spawner.spawn(mpsl_task(&*mpsl).unwrap());

    let sdc_p = sdc::Peripherals::new(
        p.PPI_CH17, p.PPI_CH18, p.PPI_CH20, p.PPI_CH21, p.PPI_CH22, p.PPI_CH23, p.PPI_CH24, p.PPI_CH25, p.PPI_CH26,
        p.PPI_CH27, p.PPI_CH28, p.PPI_CH29,
    );
    let mut rng = rng::Rng::new(p.RNG, Irqs);
    let mut rng_gen = ChaCha12Rng::from_rng(&mut rng).unwrap();
    let mut sdc_mem = sdc::Mem::<4096>::new();
    let sdc = unwrap!(build_sdc(sdc_p, &mut rng, mpsl, &mut sdc_mem));
    let mut host_resources = HostResources::new();
    let ble_addr = {
        let ficr = pac::FICR;
        let high = u64::from(ficr.deviceid(1).read());
        let addr = high << 32 | u64::from(ficr.deviceid(0).read());
        let addr = addr | 0x0000_c000_0000_0000;
        unwrap!(addr.to_le_bytes()[..6].try_into())
    };
    let stack = build_ble_stack(sdc, ble_addr, &mut rng_gen, &mut host_resources).await;

    let driver = Driver::new(p.USBD, Irqs, HardwareVbusDetect::new(Irqs));
    let flash = Flash::take(mpsl, p.NVMC);

    let mut spi_config = spim::Config::default();
    spi_config.frequency = spim::Frequency::M8;
    let spi = Spim::new_txonly(p.TWISPI0, Irqs, p.P1_13, p.P0_28, spi_config);
    static SPI_BUS: StaticCell<Mutex<NoopRawMutex, Spim<'static>>> = StaticCell::new();
    let spi_bus = SPI_BUS.init(Mutex::new(spi));
    let spi_device = SpiDevice::new(spi_bus, NoCs);
    let latch = Output::new(p.P1_00, Level::High, OutputDrive::Standard);
    let row_pins = [
        Input::new(p.P0_30, Pull::Up),
        Input::new(p.P0_31, Pull::Up),
        Input::new(p.P0_29, Pull::Up),
        Input::new(p.P0_02, Pull::Up),
    ];

    let adc_pin = p.P0_05.degrade_saadc();
    let saadc_config = saadc::Config::default();
    let channel_cfg = saadc::ChannelConfig::single_ended(adc_pin.degrade_saadc());
    interrupt::SAADC.set_priority(interrupt::Priority::P3);
    let saadc = saadc::Saadc::new(p.SAADC, Irqs, saadc_config, [channel_cfg]);
    saadc.calibrate().await;

    let ble_device_config = DeviceConfig {
        vid: 0x9d5b,
        pid: 0x2561,
        manufacturer: "KBDFans",
        product_name: "Agar Mini BLE",
        serial_number: "vial:f64c2b3c:000001",
    };
    let usb_device_config = DeviceConfig {
        product_name: "Agar Mini BLE (USB)",
        ..ble_device_config
    };
    let vial_config = VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF, UNLOCK_KEYS);
    let ble_battery_config = BleBatteryConfig::new(None, true, None, false);
    let storage_config = StorageConfig {
        start_addr: 0xA0000,
        num_sectors: 6,
        ..Default::default()
    };
    let rmk_config = RmkConfig {
        device_config: ble_device_config,
        vial_config,
        ble_battery_config,
        storage_config,
    };

    let mut keymap_data = KeymapData::new(keymap::get_default_keymap());
    let key_config = PositionalConfig::default();
    let mut behavior_config = BehaviorConfig::default();
    let (keymap, mut storage) = initialize_keymap_and_storage(
        &mut keymap_data,
        flash,
        &storage_config,
        &mut behavior_config,
        &key_config,
    )
    .await;

    let debouncer = DefaultDebouncer::<ROW, COL>::new();
    let mut matrix = Hc595Matrix::<_, _, _, _, ROW, COL, 0, 0, true>::new(spi_device, latch, row_pins, debouncer).await;
    let mut keyboard = Keyboard::new(&keymap);
    let host_service = HostService::new(&keymap, &rmk_config);

    let mut adc_device = NrfAdc::new(
        saadc,
        [AnalogEventType::Battery],
        embassy_time::Duration::from_secs(12),
        None,
    );
    let mut batt_proc = BatteryProcessor::new(2000, 2820);

    let mut status_rgb = StatusRgb::new(
        Output::new(p.P1_11, Level::High, OutputDrive::Standard),
        Output::new(p.P1_10, Level::High, OutputDrive::Standard),
        Output::new(p.P0_03, Level::High, OutputDrive::Standard),
    );

    let mut usb_transport = UsbTransport::new(driver, usb_device_config).with_host_service(&host_service);
    let mut ble_transport = BleTransport::new(&stack, rmk_config)
        .await
        .with_host_service(&host_service);
    let mut wpm_processor = WpmProcessor::new();
    let mut watchdog_runner = Nrf52Watchdog::default_runner(p.WDT);

    run_all!(
        matrix,
        adc_device,
        storage,
        usb_transport,
        ble_transport,
        wpm_processor,
        batt_proc,
        keyboard,
        status_rgb,
        watchdog_runner
    )
    .await;
}
