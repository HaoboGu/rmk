# Binary size

RMK has included many optimizations by default to of binary size. But there are still some tricks to reduce the binary size more. If you got linker error like:

```
= note: rust-lld: error:
        ERROR(cortex-m-rt): The .text section must be placed inside the FLASH memory.
        Set _stext to an address smaller than 'ORIGIN(FLASH) + LENGTH(FLASH)'
```

or some errors occur when writing configs to flash, that means that your microcontroller's internal flash is not big enough.

::: tip
For the minimal example, please checkout `examples/use_rust/stm32f1` and `examples/use_config/stm32f1` example.
:::

There are several approaches to solve the problem:

## Common approaches

### Change `DEFMT_LOG` level

Logging is quite useful when debugging the firmware, but it requires a lot of flash. You can change the default logging level to `error` at `.cargo/config.toml`, to print only error messages and save flash:

```diff
# .cargo/config.toml

[env]
- DEFMT_LOG = "debug"
+ DEFMT_LOG = "error"
```

### Enable unstable feature

According to [embassy's doc](https://embassy.dev/book/#_my_binary_is_still_big_filled_with_stdfmt_stuff), you can set the following in your `.cargo/config.toml`

```toml
[unstable]
build-std = ["core"]
build-std-features = ["panic_immediate_abort"]
```

And then compile your project with **nightly** Rust:

```
cargo +nightly build --release
# Or
cargo +nightly size --release
```

### For `keyboard.toml` users

RMK provides several options that you can use to reduce the binary size:

1. If you don't need storage, you can disable the `storage` feature to save some flash. To disable `storage` feature you need to disable default features of `rmk` crate, and then enable other features you need, for example, "col2row".

2. You can also fully remove `defmt` by removing `defmt` feature from `rmk` crate and similar feature gates from all other dependencies.

3. If you don't need vial support, you can also disable the `vial` feature by disabling default features of `rmk` crate.

```toml
# Default features `defmt`, `vial`, and `storage` are disabled
rmk = { version = "0.7", default-features = false, features = ["col2row"] }
```

If you're using `keyboard.toml`, you'll also need to disable the storage, defmt and vial in toml config:

```toml
# Disable storage, defmt and vial in keyboard.toml
[storage]
enabled = false

[dependency]
defmt_log = false

[rmk]
vial_enabled = false
```

### For Rust code users

## Use `panic-halt`

By default, RMK uses `panic-probe` to print error messages if panic occurs. But `panic-probe` actually takes lots of flash because the panic call can not be optimized. The solution is to use `panic-halt` instead of `panic-probe`:

```diff
# In your binary's Cargo.toml

- panic-probe = { version = "1.0", features = ["print-defmt"] }
+ panic-halt = "1.0"
```

The in `main.rs`, use `panic-halt` instead:

```diff
// src/main.rs

- use panic_probe as _;
+ use panic_halt as _;

```

## Remove `defmt-rtt`

You can also remove the entire defmt-rtt logger to save flash.

```diff
# In your binary's Cargo.toml
- defmt-rtt = "1.0"
```

In this case, you have to implement an empty defmt logger.

```diff
# src/main.rs
- use defmt_rtt as _;

+ #[defmt::global_logger]
+ struct Logger;
+
+ unsafe impl defmt::Logger for Logger {
+     fn acquire() {}
+     unsafe fn flush() {}
+     unsafe fn release() {}
+     unsafe fn write(_bytes: &[u8]) {}
+ }

```

## Totally remove storage and vial support

You can disable `storage` and `vial` feature in `Cargo.toml`:

```toml
# Default features `defmt`, `vial`, and `storage` are disabled
rmk = { version = "0.7", default-features = false, features = ["col2row"] }
```

And then remove anything no longer needed in `main.rs`.