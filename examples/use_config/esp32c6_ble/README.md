# esp32c6 BLE example

To run this example, you should have latest Rust in **esp** channel and `esp-idf` toolchain installed. The full instruction of installing `esp-idf` toolchain can be found [here](https://docs.esp-rs.org/book/installation/index.html) and [here](https://docs.esp-rs.org/std-training/02_2_software.html)

To run the example, make sure that you have esp-idf environment, `ldproxy` and `espflash` installed correctly. Then, run 

```
cd examples/use_config/esp32c6_ble
cargo run --release
```

If everything is good, you'll see the log as the following:

```shell
cargo run --release  
    Compiling ...
    ...
    ...
    Finished `release` profile [optimized + debuginfo] target(s) in 51.39s
     Running `espflash flash --monitor --log-format defmt target/riscv32imac-esp-espidf/release/rmk-esp32c6`
[2024-08-29T12:14:05Z INFO ] Serial port: 'COM6'
[2024-08-29T12:14:05Z INFO ] Connecting...
[2024-08-29T12:14:05Z INFO ] Using flash stub
Chip type:         esp32c6 (revision v0.0)
Crystal frequency: 40 MHz
Flash size:        4MB
Features:          WiFi 6, BT 5
MAC address:       aa:aa:aa:aa:aa:aa
App/part. size:    892,624/4,128,768 bytes, 21.62%
[2024-08-29T12:14:06Z INFO ] Segment at address '0x0' has not changed, skipping write
[2024-08-29T12:14:06Z INFO ] Segment at address '0x8000' has not changed, skipping write
[00:00:05] [========================================]     483/483     0x10000  [2024-08-29T12:14:12Z INFO ] Flashing has completed!
```

If you want to get some insight of segments of your binary, [`espsegs`](https://github.com/bjoernQ/espsegs) would help:

```
# Install it first
cargo install --git https://github.com/bjoernQ/espsegs

# Check all segments
espsegs target/riscv32imac-esp-espidf/release/rmk-esp32c6 --chip esp32c6
```