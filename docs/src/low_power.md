# Low-power

RMK supports low-power mode by using utilizing embassy's low-power feature and `Wait` trait in `embedded-hal-async`. To enable low-power mode, add `async_matrix` feature to your `Cargo.toml`:

```diff
rmk = { version = "0.2.4", features = [
     "nrf52840_ble",
     "col2row",
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
    let pd9 = ExtiInput::new(Input::new(p.PD9, Pull::Down).degrade(), p.EXTI9.degrade());
    let pd8 = ExtiInput::new(Input::new(p.PD8, Pull::Down).degrade(), p.EXTI8.degrade());
    let pb13 = ExtiInput::new(Input::new(p.PB13, Pull::Down).degrade(), p.EXTI13.degrade());
    let pb12 = ExtiInput::new(Input::new(p.PB12, Pull::Down).degrade(), p.EXTI12.degrade());
    let input_pins = [pd9, pd8, pb13, pb12];

    // ...Other initialization code

    // Run RMK
    run_rmk(
        input_pins,
        output_pins,
        driver,
        f,
        crate::keymap::KEYMAP,
        keyboard_config,
        spawner,
    )
    .await;

```
