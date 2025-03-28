#![no_std]
#![no_main]

use bt_hci::controller::ExternalController;
use embassy_executor::Spawner;
use esp_hal::{clock::CpuClock, timer::timg::TimerGroup};
use esp_wifi::ble::controller::BleConnector;
use {esp_alloc as _, esp_backtrace as _};


#[esp_hal_embassy::main]
async fn main(_s: Spawner) {
    esp_println::logger::init_logger_from_env();
    let peripherals = esp_hal::init({
        let mut config = esp_hal::Config::default();
        config.cpu_clock = CpuClock::max();
        config
    });
    esp_alloc::heap_allocator!(72 * 1024);
    let timg0 = TimerGroup::new(peripherals.TIMG0);

    let mut rng = esp_hal::rng::Trng::new(peripherals.RNG, peripherals.ADC1);

    let init = esp_wifi::init(timg0.timer0, rng.rng.clone(), peripherals.RADIO_CLK).unwrap();

    let systimer = esp_hal::timer::systimer::SystemTimer::new(peripherals.SYSTIMER);
    esp_hal_embassy::init(systimer.alarm0);


    let bluetooth = peripherals.BT;
    let connector = BleConnector::new(&init, bluetooth);
    let controller: ExternalController<_, 20> = ExternalController::new(connector);

    const L2CAP_MTU: usize = 255;
    rmk::ble::trouble::run::<_, _, { L2CAP_MTU }>(controller, &mut rng).await;
}
