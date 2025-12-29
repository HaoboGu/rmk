# RMK py32f07x example

Compatible with: py32f072, py32f071, py32f070.

Please always run with `cargo run --release`.

> [!NOTE]
> **Maintenance & Compatibility Note**
>
> This example relies on specific HALs that may have different release cycles compared to the upstream Embassy/RMK.
>
> To ensure stability, this example **locks the RMK dependency to a specific version** and is **not actively updated** to track the RMK `main` branch daily. It is maintained on a periodic basis to align with major release milestones.

## Storage

Although `py32-hal` provides a flash interface, py32f07x has only 16K RAM. Enabling `storage` feature will cause a stack overflow.

The async state machine inherently increases stack usage. While setting `codegen-units = 1` can mitigate this by allowing the compiler to see a broader optimization context, there is still a RAM shortfall of approximately 4K. For a deep dive into this issue, see: [Details (Chinese)](https://decaday.github.io/blog/stack-overflow-learn/).
