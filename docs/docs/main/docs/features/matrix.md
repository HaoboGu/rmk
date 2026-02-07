# Matrix

The keyboard matrix is the core hardware system responsible for scanning switches and detecting key presses. It serves as the bridge between the physical keyboard hardware and the firmware's key processing logic.

## Matrix Types in RMK

RMK provides three built-in matrix implementations to match different hardware designs:

### Normal Matrix

The standard approach. Keys are wired in a row-column grid, using diodes to prevent [ghosting](https://en.wikipedia.org/wiki/Key_rollover#Key_jamming_and_ghosting). RMK supports both col2row and row2col diode configurations to match your PCB design. You can set the diode direction in the [matrix configuration](../configuration/keyboard_matrix#matrix-configuration).

### Direct Pin Matrix

Each key connects directly to its own GPIO pin, eliminating the matrix grid and the need for diodes. All key states are read simultaneously without scanning. This method requires a high number of GPIO pins, so it's best for small keyboards and macropads.

### Bidirectional Matrix

The bidirectional matrix design uses dynamically switchable GPIO pins that can change between input and output modes during the scan cycle. Because the bidirectional matrix is more complicated than the normal matrix, only the [Rust API](https://github.com/HaoboGu/rmk/blob/main/rmk/src/matrix/bidirectional_matrix.rs) is provided at the moment. 

## Async Matrix Feature

Async matrix is a power-saving feature that transforms how the matrix operates, dramatically reducing power consumption for wireless keyboards. This feature works out-of-the-box for nRF52 series. STM32 requires additional EXTI (external interrupt) configuration due to hardware limitations—see the [Low Power](./low_power) documentation for details.

To enable it, add the `async_matrix` feature in `Cargo.toml`:

```toml
rmk = { version = "...", features = ["async_matrix"] }
```

## Configuration

For detailed matrix configuration options, pin assignments, and platform-specific setup, see the [Matrix Configuration](../configuration/keyboard_matrix#matrix-configuration) documentation.

## Customization via Traits

RMK's matrix system is built on a trait-based architecture. Any matrix or debouncer that implements the corresponding trait can be seamlessly integrated into RMK, making both components highly extensible without touching core firmware code:

**`MatrixTrait`**: Defines the core scanning interface. Implement this trait to support external I/O expanders, non-standard electrical designs, or specialized scanning algorithms.

**`DebouncerTrait`**: Controls switch bounce filtering. RMK includes default and fast debouncing algorithms, and you can also implement custom debouncing logic optimized for your own use cases.

The following is an example demonstrating how to use a customized matrix:

```rust
struct YourOwnMatrix<const ROW: usize, const COL: usize> {}
impl<const ROW: usize, const COL: usize> MatrixTrait<ROW, COL> for YourOwnMatrix<ROW, COL> {
    // Implement the `MatrixTrait`
}

let my_matrix = YourOwnMatrix::new(); // Create the matrix struct

// .. Other initialization

// Run the main process
join3(
    run_all!(my_matrix),
    keyboard.run(),
    run_rmk(&keymap, driver, &mut storage, rmk_config),
)
.await;
```

## See Also

- [How key matrices work](https://pcbheaven.com/wikipages/How_Key_Matrices_Works/)
