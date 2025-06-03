# Low-power

RMK supports low-power mode by using utilizing embassy's low-power feature and `Wait` trait in `embedded-hal-async`. To enable low-power mode, add `async_matrix` feature to your `Cargo.toml`:

```diff
rmk = { version = "0.7", features = [
     "nrf52840_ble",
+    "async_matrix",
] }
```

If you're using nRF chips or rp2040, you're all set! You've already got your keyboard running in low-power mode.

For stm32, there's some limitations about Exti(see [here](https://docs.embassy.dev/embassy-stm32/git/stm32g474pc/exti/struct.ExtiInput.html)):

> EXTI is not built into Input itself because it needs to take ownership of the corresponding EXTI channel, which is a limited resource.
>
> Pins PA5, PB5, PC5… all use EXTI channel 5, so you can’t use EXTI on, say, PA5 and PC5 at the same time.

There are a few more things that you have to do:

1. Enable `exti` feature of your `embassy-stm32` dependency

2. Ensure that your input pins don't share same EXTI channel

3. If you're using `keyboard.toml`, nothing more to do. The `[rmk_keyboard]` macro will check your `Cargo.toml` and do the work for you. But if you're using Rust code, you need to use `ExtiInput` as your input pins, and update generics type of RMK keyboard run:

```rust
    let pd9 = ExtiInput::new(p.PD9,  p.EXTI9, Pull::Down);
    let pd8 = ExtiInput::new(p.PD8,  p.EXTI8, Pull::Down);
    let pb13 = ExtiInput::new(p.PB13, p.EXTI13, Pull::Down);
    let pb12 = ExtiInput::new(p.PB12, p.EXTI12, Pull::Down);
    let input_pins = [pd9, pd8, pb13, pb12];

    let mut matrix = Matrix::<_, _, _, ROW, COL>::new(input_pins, output_pins, debouncer);
```
