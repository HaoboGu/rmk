//! Display support.
//!
//! Provides [`DisplayProcessor`] — a processor that subscribes to keyboard
//! state events and renders them on any display implementing [`DisplayDriver`].
//!
//! # Customisation
//!
//! The processor is generic over a [`DisplayDriver`] and a [`DisplayRenderer`].
//! The built-in [`DefaultOledRenderer`] adapts automatically between landscape and
//! portrait layouts.  To draw your own content implement [`DisplayRenderer<C>`]
//! for your color type and pass it via [`DisplayProcessor::with_renderer`].
//!
//! # Feature flags
//!
//! - `display` — base traits and processor (requires `embedded-graphics`)
//! - `ssd1306` — SSD1306 OLED driver support (implies `display`)
//!
//! # Example — SSD1306 with default renderer
//!
//! ```rust,ignore
//! use ssd1306::{I2CDisplayInterface, Ssd1306Async, prelude::*};
//! use rmk::display::DisplayProcessor;
//!
//! let interface = I2CDisplayInterface::new(i2c);
//! let display = Ssd1306Async::new(interface, DisplaySize128x32, DisplayRotation::Rotate0)
//!     .into_buffered_graphics_mode();
//!
//! let mut oled = DisplayProcessor::new(display);
//! run_all!(matrix, oled);
//! ```
//!
//! # Example — custom renderer
//!
//! ```rust,ignore
//! use rmk::display::{DisplayRenderer, DisplayProcessor, RenderContext};
//! use embedded_graphics::{prelude::*, pixelcolor::BinaryColor, text::Text,
//!     mono_font::{ascii::FONT_6X10, MonoTextStyle}};
//!
//! struct BigLayer;
//!
//! impl DisplayRenderer<BinaryColor> for BigLayer {
//!     fn render<D: DrawTarget<Color = BinaryColor>>(
//!         &mut self, ctx: &RenderContext, display: &mut D,
//!     ) {
//!         let style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
//!         let mut buf: heapless::String<8> = heapless::String::new();
//!         core::fmt::write(&mut buf, format_args!("L{}", ctx.layer)).ok();
//!         Text::new(&buf, Point::new(0, 12), style).draw(display).ok();
//!     }
//! }
//!
//! let mut oled = DisplayProcessor::with_renderer(display, BigLayer);
//! run_all!(matrix, oled);
//! ```

mod drivers;
mod renderer;

#[cfg(feature = "oled_async")]
pub use display_interface_i2c;
use embassy_time::{Duration, Instant};
#[cfg(feature = "oled_async")]
pub use oled_async;
pub use renderer::{DefaultOledRenderer, DisplayDriver, DisplayRenderer, RenderContext, write_battery};
use rmk_macro::processor;
#[cfg(feature = "ssd1306")]
pub use ssd1306;

#[cfg(feature = "_ble")]
use crate::event::BleStatusChangeEvent;
#[cfg(all(feature = "split", feature = "_ble"))]
use crate::event::PeripheralBatteryEvent;
use crate::event::{
    BatteryStateEvent, KeyboardEvent, LayerChangeEvent, LedIndicatorEvent, SleepStateEvent, WpmUpdateEvent,
};
#[cfg(feature = "split")]
use crate::event::{CentralConnectedEvent, PeripheralConnectedEvent};
use crate::processor::PollingProcessor;

/// Processor that renders keyboard state on a display.
///
/// Subscribes to [`LayerChangeEvent`], [`WpmUpdateEvent`], [`LedIndicatorEvent`],
/// and [`BatteryStateEvent`], redrawing the screen whenever any of these change.
///
/// The rendering is delegated to a [`DisplayRenderer`].  Use [`new`](Self::new)
/// for the built-in [`DefaultOledRenderer`], or [`with_renderer`](Self::with_renderer)
/// for a custom one.
///
/// # Generics
///
/// - `D` — display driver, must implement [`DisplayDriver`].
/// - `R` — the renderer, defaults to [`DefaultOledRenderer`].
#[processor(subscribe = [KeyboardEvent, LayerChangeEvent, WpmUpdateEvent, LedIndicatorEvent, BatteryStateEvent, SleepStateEvent], manual_polling = true)]
#[cfg_attr(feature = "_ble", processor(subscribe = [BleStatusChangeEvent]))]
#[cfg_attr(feature = "split", processor(subscribe = [PeripheralConnectedEvent, CentralConnectedEvent]))]
#[cfg_attr(all(feature = "split", feature = "_ble"), processor(subscribe = [PeripheralBatteryEvent]))]
pub struct DisplayProcessor<D, R = DefaultOledRenderer>
where
    D: DisplayDriver,
    R: DisplayRenderer<D::Color>,
{
    display: D,
    renderer: R,
    ctx: RenderContext,
    initialized: bool,
    last_render: Instant,
    /// Minimum time between renders (rate-limiter for event-driven renders).
    min_render_interval: Duration,
    /// Poll interval for animations. `Duration::MAX` disables polling.
    render_interval: Duration,
}

impl<D> DisplayProcessor<D, DefaultOledRenderer>
where
    D: DisplayDriver,
    DefaultOledRenderer: DisplayRenderer<D::Color>,
{
    /// Create a new display processor with the built-in [`DefaultOledRenderer`].
    ///
    /// Polling is disabled by default — the display only redraws on events.
    /// Use [`with_render_interval`](Self::with_render_interval) to enable
    /// periodic redraws for animations.
    pub fn new(display: D) -> Self {
        Self::with_renderer(display, DefaultOledRenderer)
    }
}

impl<D, R> DisplayProcessor<D, R>
where
    D: DisplayDriver,
    R: DisplayRenderer<D::Color>,
{
    /// Create a new display processor with a custom [`DisplayRenderer`].
    ///
    /// Polling is disabled by default. Call
    /// [`with_render_interval`](Self::with_render_interval) to enable it.
    pub fn with_renderer(display: D, renderer: R) -> Self {
        Self {
            display,
            renderer,
            ctx: RenderContext::default(),
            initialized: false,
            last_render: Instant::from_ticks(0),
            min_render_interval: Duration::from_millis(33),
            render_interval: Duration::MAX,
        }
    }

    /// Set the poll interval for periodic redraws (animations).
    ///
    /// When set, the display redraws at this interval even without events.
    /// Without this, the display only redraws when keyboard state changes.
    pub fn with_render_interval(mut self, interval: Duration) -> Self {
        self.render_interval = interval;
        self
    }

    /// Set the minimum time between event-driven renders.
    ///
    /// When events arrive faster than this interval, renders are skipped
    /// and the latest state is drawn on the next render. Default: 10 ms.
    pub fn with_min_render_interval(mut self, interval: Duration) -> Self {
        self.min_render_interval = interval;
        self
    }

    /// Periodic poll — drives animations even when no events arrive.
    async fn poll(&mut self) {
        self.render().await;
    }

    /// Redraw the display if enough time has passed since the last render.
    ///
    /// When events arrive faster than the display can refresh (e.g. rapid
    /// key presses), the render is skipped — the updated state will be
    /// drawn on the next event that passes the time check.
    async fn render(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.last_render) < self.min_render_interval {
            return;
        }

        if !self.initialized {
            self.display.init().await;
            self.initialized = true;
        }

        self.renderer.render(&self.ctx, &mut self.display);
        self.display.flush().await;

        self.last_render = Instant::now();
    }

    async fn on_layer_change_event(&mut self, event: LayerChangeEvent) {
        self.ctx.layer = event.layer;
        self.render().await;
    }

    async fn on_wpm_update_event(&mut self, event: WpmUpdateEvent) {
        self.ctx.wpm = event.wpm;
    }

    async fn on_led_indicator_event(&mut self, event: LedIndicatorEvent) {
        self.ctx.caps_lock = event.indicator.caps_lock();
        self.ctx.num_lock = event.indicator.num_lock();
        self.render().await;
    }

    async fn on_battery_state_event(&mut self, event: BatteryStateEvent) {
        self.ctx.battery = event;
        self.render().await;
    }

    async fn on_keyboard_event(&mut self, event: KeyboardEvent) {
        self.ctx.key_pressed = event.pressed;
        self.render().await;
    }

    async fn on_sleep_state_event(&mut self, event: SleepStateEvent) {
        self.ctx.sleeping = event.sleeping;
        self.render().await;
    }

    #[cfg(feature = "_ble")]
    async fn on_ble_status_change_event(&mut self, event: BleStatusChangeEvent) {
        self.ctx.ble_status = event.0;
        self.render().await;
    }

    #[cfg(feature = "split")]
    async fn on_peripheral_connected_event(&mut self, event: PeripheralConnectedEvent) {
        if let Some(slot) = self.ctx.peripherals_connected.get_mut(event.id) {
            *slot = event.connected;
        }
        self.render().await;
    }

    #[cfg(feature = "split")]
    async fn on_central_connected_event(&mut self, event: CentralConnectedEvent) {
        self.ctx.central_connected = event.connected;
        self.render().await;
    }

    #[cfg(all(feature = "split", feature = "_ble"))]
    async fn on_peripheral_battery_event(&mut self, event: PeripheralBatteryEvent) {
        if let Some(slot) = self.ctx.peripheral_batteries.get_mut(event.id) {
            *slot = event.state;
        }
        self.render().await;
    }
}

impl<D, R> PollingProcessor for DisplayProcessor<D, R>
where
    D: DisplayDriver,
    R: DisplayRenderer<D::Color>,
{
    fn interval(&self) -> Duration {
        if self.ctx.sleeping {
            Duration::MAX
        } else {
            self.render_interval
        }
    }

    async fn update(&mut self) {
        self.poll().await;
    }
}
