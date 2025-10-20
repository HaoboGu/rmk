# Binary size

RMK has included many optimizations by default to of binary size. But there are still some tricks to reduce the binary size more. If you got linker error like:

```
= note: rust-lld: error:
        ERROR(cortex-m-rt): The .text section must be placed inside the FLASH memory.
        Set _stext to an address smaller than 'ORIGIN(FLASH) + LENGTH(FLASH)'
```

or some errors occur when writing configs to flash, that means that your microcontroller's internal flash is not big enough.

There are several approaches to solve the problem:

## Change `DEFMT_LOG` level

Logging is quite useful when debugging the firmware, but it requires a lot of flash. You can change the default logging level to `error` at `.cargo/config.toml`, to print only error messages and save flash:

```diff
# .cargo/config.toml

[env]
- DEFMT_LOG = "debug"
+ DEFMT_LOG = "error"
```

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

## Enable unstable feature

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

This config will reduce about 4-6kb of binary size furthermore.

After applying all above approaches, total binary size of stm32h7 example can be reduced from about 93KB to 54KB, which means the binary size decreases about 42%!

## Disable `storage` feature and `defmt` feature

If you don't need storage, you can disable the `storage` feature to save some flash. To disable `storage` feature you need to disable default features of `rmk` crate, and then enable features you need manually.

You can also fully remove `defmt` by removing `defmt` feature from `rmk` crate and similar feature gates from all other dependencies.

```toml
rmk = { version = "0.7", default-features = false, features = ["col2row"] }
```

If you re using `keyboard.toml`, you'll also need to disable the storage or defmt in toml config:

```toml
# Disable storage and defmt in keyboard.toml
[storage]
enabled = false

[dependency]
defmt_log = false
```