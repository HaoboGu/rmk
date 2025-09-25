# RMK py32f07x example

Compatible with: py32f072, py32f071, py32f070.

Please always run with `cargo run --release`.

## Storage

Although `py32-hal` provides a flash interface, py32f07x has only 16K RAM. Enabling `storage` will cause a stack overflow.

You may try adjusting the compilation parameters, but some strange issues may occur: [Details (Chinese)](https://decaday.github.io/blog/stack-overflow-learn/).