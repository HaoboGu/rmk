# Low Power

RMK supports low-power mode by using utilizing embassy's low-power feature and `Wait` trait in `embedded-hal-async`.

## Usage

By default RMK uses a busy-loop for matrix scanning, which is not very power efficient. To enable the low-power mode, add the `async_matrix` feature to your `Cargo.toml`:

```toml {3}
rmk = { version = "...", features = [
    "nrf52840_ble",
    "async_matrix",
] }
```

If you're using nRF chips or RP2040, you're all set! Your keyboard is now running in low-power mode. The `async_matrix` feature enables interrupt-based input detection, puts your microcontroller into sleep mode when no key are being pressed.

For STM32, there's some limitations about Exti(see [here](https://docs.embassy.dev/embassy-stm32/git/stm32g474pc/exti/struct.ExtiInput.html)):

> EXTI is not built into Input itself because it needs to take ownership of the corresponding EXTI channel, which is a limited resource.
>
> Pins PA5, PB5, PC5… all use EXTI channel 5, so you can’t use EXTI on, say, PA5 and PC5 at the same time.

There are a few more things that you have to do:

1. Enable the `exti` feature for your `embassy-stm32` dependency in `Cargo.toml`
2. Ensure that your input pins don't share same EXTI channel
3. For configuration:
    - If you're using `keyboard.toml`, you are all set. The `[rmk_keyboard]` macro will automatically check your `Cargo.toml` and handlt it for you.
    - If you're using Rust code, you'll need to use `ExtiInput` for your input pins:

```rust
    let pd9 = ExtiInput::new(p.PD9,  p.EXTI9, Pull::Down);
    let pd8 = ExtiInput::new(p.PD8,  p.EXTI8, Pull::Down);
    let pb13 = ExtiInput::new(p.PB13, p.EXTI13, Pull::Down);
    let pb12 = ExtiInput::new(p.PB12, p.EXTI12, Pull::Down);
    let row_pins = [pd9, pd8, pb13, pb12];

    let mut matrix = Matrix::<_, _, _, ROW, COL, true>::new(row_pins, col_pins, debouncer);
```
