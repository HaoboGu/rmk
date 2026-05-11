use super::WatchdogFeed;

/// ESP32 MWDT (Main Watchdog Timer) wrapper.
pub struct Esp32Watchdog<TG: esp_hal::timer::timg::TimerGroupInstance> {
    inner: esp_hal::timer::timg::Wdt<TG>,
}

impl<TG: esp_hal::timer::timg::TimerGroupInstance> Esp32Watchdog<TG> {
    pub fn new(wdt: esp_hal::timer::timg::Wdt<TG>) -> Self {
        Self { inner: wdt }
    }
}

impl<TG: esp_hal::timer::timg::TimerGroupInstance + Send> WatchdogFeed for Esp32Watchdog<TG> {
    fn feed(&mut self) {
        self.inner.feed();
    }
}
