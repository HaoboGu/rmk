# PY32F07x RMK Example

Driven by the [py32-hal](https://github.com/py32-rs/py32-hal) and [musb](https://github.com/decaday/musb) projects.

## Memory Usage

The py32f07x has only 16KB of RAM. To make the most of this limited memory, we are using the [`nightly`](https://docs.embassy.dev/embassy-executor/git/std/index.html#task-arena) feature of [embassy_executor](https://docs.embassy.dev/embassy-executor/git/std/index.html).

## Memory Issues

In this example, the `codegen-units` in the Cargo.toml profile is set to 8.

This change reduces stack usage by 7KB. This is quite strange because one would expect lower `codegen-units` to improve performance.

The reduced stack memory is primarily for `embassy_executor::raw::TaskStorage<F>::poll`.

If you set `codegen-units` to 1 and use flip-link, the extra 7KB of memory will cause the program to hardfault when running the `main` function.

Additionally, when I upgraded [py32-hal](https://github.com/py32-rs/py32-hal) from version 0.2.0 to 0.2.1, the stack for the `poll` function grew by 8KB. Upon investigation, this change occurred after I moved the USB driver from [py32-hal](https://github.com/py32-rs/py32-hal) to the [musb](https://github.com/decaday/musb) crate: [#21 · py32-hal](https://github.com/py32-rs/py32-hal/pull/21).

If you have any insights, feel free to reach out to me: decaday [myDecaday@outlook.com](mailto:myDecaday@outlook.com). 

For more details, you can check out this blog (in Chinese): https://decaday.github.io/blog/