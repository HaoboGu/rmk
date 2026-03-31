//! Display support.
//!
//! Provides [`OledDisplayProcessor`] — a processor that subscribes to keyboard
//! state events and renders them on an SSD1306-compatible OLED display.
//!
//! # Customisation
//!
//! The processor is generic over a [`DisplayRenderer<C>`].  The built-in
//! [`DefaultRenderer`] adapts automatically between landscape and portrait
//! layouts (see its docs for details).  To draw your own content implement
//! [`DisplayRenderer<C>`] for your color type and pass it via
//! [`OledDisplayProcessor::with_renderer`].
//!
//! # Feature flag
//!
//! Enable the `display` feature in your `Cargo.toml`:
//! ```toml
//! rmk = { version = "...", features = ["display"] }
//! ```
//!
//! You will also need `ssd1306` and `embedded-graphics` in your own
//! dependencies:
//! ```toml
//! ssd1306 = "0.9"
//! embedded-graphics = "0.8"
//! ```
//!
//! # Example — default renderer
//!
//! ```rust,ignore
//! use ssd1306::{I2CDisplayInterface, Ssd1306, prelude::*};
//! use rmk::display::OledDisplayProcessor;
//!
//! let interface = I2CDisplayInterface::new(i2c);
//! let display = Ssd1306::new(interface, DisplaySize128x32, DisplayRotation::Rotate0)
//!     .into_buffered_graphics_mode();
//!
//! let mut oled = OledDisplayProcessor::new(display);
//! run_all!(matrix, oled);
//! ```
//!
//! # Example — custom renderer
//!
//! ```rust,ignore
//! use rmk::display::{DisplayRenderer, OledDisplayProcessor, RenderContext};
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
//! let mut oled = OledDisplayProcessor::with_renderer(display, BigLayer);
//! run_all!(matrix, oled);
//! ```

mod renderer;

pub use renderer::{DefaultRenderer, DisplayRenderer, RenderContext, write_battery};

use display_interface::WriteOnlyDataCommand;
use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};
use rmk_macro::processor;
use ssd1306::{mode::BufferedGraphicsMode, mode::DisplayConfig, size::DisplaySize};

use crate::event::{BatteryStateEvent, LayerChangeEvent, LedIndicatorEvent, WpmUpdateEvent};

/// Processor that renders keyboard state on an SSD1306 OLED display.
///
/// Subscribes to [`LayerChangeEvent`], [`WpmUpdateEvent`], [`LedIndicatorEvent`],
/// and [`BatteryStateEvent`], redrawing the screen whenever any of these change.
///
/// The rendering is delegated to a [`DisplayRenderer`].  Use [`new`](Self::new)
/// for the built-in [`DefaultRenderer`], or [`with_renderer`](Self::with_renderer)
/// for a custom one.
///
/// # Generics
///
/// - `DI` — display interface (I2C or SPI), must implement
///   [`display_interface::WriteOnlyDataCommand`].
/// - `SIZE` — display size, e.g. [`ssd1306::prelude::DisplaySize128x32`].
/// - `R` — the renderer, defaults to [`DefaultRenderer`].
#[processor(subscribe = [LayerChangeEvent, WpmUpdateEvent, LedIndicatorEvent, BatteryStateEvent])]
pub struct OledDisplayProcessor<DI, SIZE, R = DefaultRenderer>
where
    DI: WriteOnlyDataCommand,
    SIZE: DisplaySize,
    R: DisplayRenderer<BinaryColor>,
    ssd1306::Ssd1306<DI, SIZE, BufferedGraphicsMode<SIZE>>: DrawTarget<Color = BinaryColor> + DisplayConfig,
{
    display: ssd1306::Ssd1306<DI, SIZE, BufferedGraphicsMode<SIZE>>,
    renderer: R,
    layer: u8,
    wpm: u16,
    caps_lock: bool,
    num_lock: bool,
    battery: BatteryStateEvent,
    /// Tracks whether `init()` has been called; lazy so `new()` stays sync.
    initialized: bool,
}

impl<DI, SIZE> OledDisplayProcessor<DI, SIZE, DefaultRenderer>
where
    DI: WriteOnlyDataCommand,
    SIZE: DisplaySize,
    ssd1306::Ssd1306<DI, SIZE, BufferedGraphicsMode<SIZE>>: DrawTarget<Color = BinaryColor> + DisplayConfig,
{
    /// Create a new display processor with the built-in [`DefaultRenderer`].
    ///
    /// The display is lazily initialised on the first event
    pub fn new(display: ssd1306::Ssd1306<DI, SIZE, BufferedGraphicsMode<SIZE>>) -> Self {
        Self::with_renderer(display, DefaultRenderer)
    }
}

impl<DI, SIZE, R> OledDisplayProcessor<DI, SIZE, R>
where
    DI: WriteOnlyDataCommand,
    SIZE: DisplaySize,
    R: DisplayRenderer<BinaryColor>,
    ssd1306::Ssd1306<DI, SIZE, BufferedGraphicsMode<SIZE>>: DrawTarget<Color = BinaryColor> + DisplayConfig,
{
    /// Create a new display processor with a custom [`DisplayRenderer`].
    ///
    /// The display is lazily initialised on the first event.
    pub fn with_renderer(display: ssd1306::Ssd1306<DI, SIZE, BufferedGraphicsMode<SIZE>>, renderer: R) -> Self {
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
    fn render(&mut self) {
        if !self.initialized {
            self.display.init().ok();
            self.initialized = true;
        }

        self.display.clear(BinaryColor::Off).ok();

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

        self.display.flush().ok();
    }

    async fn on_layer_change_event(&mut self, event: LayerChangeEvent) {
        self.layer = event.layer;
        self.render();
    }

    async fn on_wpm_update_event(&mut self, event: WpmUpdateEvent) {
        self.wpm = event.wpm;
        self.render();
    }

    async fn on_led_indicator_event(&mut self, event: LedIndicatorEvent) {
        self.caps_lock = event.indicator.caps_lock();
        self.num_lock = event.indicator.num_lock();
        self.render();
    }

    async fn on_battery_state_event(&mut self, event: BatteryStateEvent) {
        self.battery = event;
        self.render();
    }
}
