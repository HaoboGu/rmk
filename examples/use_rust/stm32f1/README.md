# STM32F1 example

This example shows the minimal size of RMK. `storage`, `vial` and `defmt` feature are all disabled. You can compile the example using 


```
cargo +nightly build --release
```

and check the size using 

```
cargo +nightly size --release
```