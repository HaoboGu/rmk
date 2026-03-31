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

pub use renderer::{
    DefaultOledRenderer, DisplayDriver, DisplayRenderer, RenderContext, write_battery,
};

use rmk_macro::processor;

use crate::event::{BatteryStateEvent, LayerChangeEvent, LedIndicatorEvent, WpmUpdateEvent};

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
#[processor(subscribe = [LayerChangeEvent, WpmUpdateEvent, LedIndicatorEvent, BatteryStateEvent])]
pub struct DisplayProcessor<D, R = DefaultOledRenderer>
where
    D: DisplayDriver,
    R: DisplayRenderer<D::Color>,
{
    display: D,
    renderer: R,
    layer: u8,
    wpm: u16,
    caps_lock: bool,
    num_lock: bool,
    battery: BatteryStateEvent,
    initialized: bool,
}

impl<D> DisplayProcessor<D, DefaultOledRenderer>
where
    D: DisplayDriver,
    DefaultOledRenderer: DisplayRenderer<D::Color>,
{
    /// Create a new display processor with the built-in [`DefaultOledRenderer`].
    ///
    /// The display is lazily initialised on the first event.
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
    /// The display is lazily initialised on the first event.
    pub fn with_renderer(display: D, renderer: R) -> Self {
        Self {
            display,
            renderer,
            layer: 0,
            wpm: 0,
            caps_lock: false,
            num_lock: false,
            battery: BatteryStateEvent::NotAvailable,
            initialized: false,
        }
    }

    /// Redraw the full display by delegating to the renderer.
    async fn render(&mut self) {
        if !self.initialized {
            self.display.init().await;
            self.initialized = true;
        }

        let bbox = self.display.bounding_box();
        let ctx = RenderContext {
            layer: self.layer,
            wpm: self.wpm,
            caps_lock: self.caps_lock,
            num_lock: self.num_lock,
            battery: self.battery,
            width: bbox.size.width,
            height: bbox.size.height,
        };

        self.renderer.render(&ctx, &mut self.display);

        self.display.flush().await;
    }

    async fn on_layer_change_event(&mut self, event: LayerChangeEvent) {
        self.layer = event.layer;
        self.render().await;
    }

    async fn on_wpm_update_event(&mut self, event: WpmUpdateEvent) {
        self.wpm = event.wpm;
        self.render().await;
    }

    async fn on_led_indicator_event(&mut self, event: LedIndicatorEvent) {
        self.caps_lock = event.indicator.caps_lock();
        self.num_lock = event.indicator.num_lock();
        self.render().await;
    }

    async fn on_battery_state_event(&mut self, event: BatteryStateEvent) {
        self.battery = event;
        self.render().await;
    }
}
