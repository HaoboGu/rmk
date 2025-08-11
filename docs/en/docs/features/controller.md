# Controller

Controller is designed for the output devices. It provides a unified interface for controlling various output devices, like display.

## External Controller
In RMK, controllers can be out-of-tree. To use those controllers with RMK's  convenient TOML-configuration, you can add relative code for the controller in the `main.rs` or `central.rs`, where the attributes `#[rmk_keyboard]`, `#[rmk_central]` or `#[rmk_peripheral]` are.

In the attributes `#[rmk_keyboard]`, `#[rmk_central]` or `#[rmk_peripheral]`. We can declare a controller as below. The name of the function is unique as the controller's name. And the body of the function returns the controller. The return type can be omitted.
```rust
#[rmk_central]
mod keybaord_central {
    // ...

    #[controller]
    // the controller is named `display_controller`, the return type can be ignored.
    fn display_controller() {

        // prepare the config for the controller
        let mut config = ::embassy_nrf::spim::Config::default();
        config.frequency = ::embassy_nrf::spim::Frequency::M1;
        let spi = ::embassy_nrf::spim::Spim::new_txonly(p.SPI3, Irqs, p.P0_06, p.P0_05, config);
        let cs = ::embassy_nrf::gpio::Output::new(
            p.P0_26,
            ::embassy_nrf::gpio::Level::High,
            ::embassy_nrf::gpio::OutputDrive::Standard
        );

        // initialize the controller
        let controller = rmk_display::spec::nice_view::create_controller::<_, _, 2>(spi, cs);

        // return the controller
        controller
    }

    // ...
}
```

## Bind External Interrupt
Some external controllers may need interrupt. We can declare the interrupt as well.

```rust
#[rmk_central]
mod keybaord_central {
    // ...

    // declare the interrupt
    add_interrupt!(SPIM3 => ::embassy_nrf::spim::InterruptHandler<::embassy_nrf::peripherals::SPI3>);

    // ...
}
```
