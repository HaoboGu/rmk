# esp32c3 BLE example

To run this example, you should have latest Rust in **esp** channel and `esp-idf` toolchain installed. The full instruction of installing `esp-idf` toolchain can be found [here](https://docs.esp-rs.org/book/installation/index.html) and [here](https://docs.esp-rs.org/std-training/02_2_software.html)

To run the example, make sure that you have esp-idf environment, `ldproxy` and `espflash` installed correctly. Then, run 

```
cd examples/use_rust/esp32c3_ble
cargo run --release
```

If everything is good, you'll see the log as the following:

```shell
cargo run --release  
    Compiling ...
    ...
    ...
    Finished `release` profile [optimized + debuginfo] target(s) in 51.39s
     Running `espflash flash --monitor --log-format defmt target/riscv32imc-esp-espidf/release/rmk-esp32c3`
[2024-04-07T12:49:21Z INFO ] Detected 2 serial ports
[2024-04-07T12:49:21Z INFO ] Ports which match a known common dev board are highlighted
[2024-04-07T12:49:21Z INFO ] Please select a port
[2024-04-07T12:50:24Z INFO ] Serial port: '/dev/cu/xx'
[2024-04-07T12:50:24Z INFO ] Connecting...
[2024-04-07T12:50:24Z INFO ] Using flash stub
Chip type:         esp32c3 (revision v0.4)
Crystal frequency: 40 MHz
Flash size:        4MB
Features:          WiFi, BLE
MAC address:       aa:aa:aa:aa:aa:aa
App/part. size:    607,488/4,128,768 bytes, 14.71%
[2024-04-07T12:50:24Z INFO ] Segment at address '0x0' has not changed, skipping write
[2024-04-07T12:50:24Z INFO ] Segment at address '0x8000' has not changed, skipping write
[00:00:03] [========================================]     337/337     0x10000                                                                                                                    [2024-04-07T12:50:28Z INFO ] Flashing has completed!
```

If you want to get some insight of segments of your binary, [`espsegs`](https://github.com/bjoernQ/espsegs) would help:

```
# Install it first
cargo install --git https://github.com/bjoernQ/espsegs

# Check all segments
espsegs target/riscv32imc-esp-espidf/release/rmk-esp32c3 --chip esp32c3
```