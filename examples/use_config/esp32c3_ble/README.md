# esp32c3 BLE example

To run this example, you should have latest Rust installed. The full instruction of installing esp Rust toolchain can be found [here](https://docs.esp-rs.org/book/installation/index.html).

[`espflash`](https://github.com/esp-rs/espflash) should also be installed:

```
cargo install cargo-espflash espflash
```

After having everything installed, use the following command to run the example:

```
cd examples/use_config/esp32c3_ble
cargo run --release
```

If everything is good, you'll see the log as the following:

```shell
cargo run --release  
    Compiling ...
    ...
    ...
    Finished `release` profile [optimized + debuginfo] target(s) in 11.70s
     Running `espflash flash --monitor --port /dev/cu.usbmodem211401 target/riscv32imc-unknown-none-elf/release/rmk-esp32c3`
[2025-04-10T10:01:23Z INFO ] Serial port: '/dev/cu.usbmodem211401'
[2025-04-10T10:01:23Z INFO ] Connecting...
[2025-04-10T10:01:23Z INFO ] Using flash stub
Chip type:         esp32c3 (revision v0.1)
Crystal frequency: 40 MHz
Flash size:        4MB
Features:          WiFi 6, BT 5
MAC address:       40:4c:ca:5b:c7:dc
App/part. size:    768,944/4,128,768 bytes, 18.62%
[2025-04-10T10:01:23Z INFO ] Segment at address '0x0' has not changed, skipping write
[2025-04-10T10:01:23Z INFO ] Segment at address '0x8000' has not changed, skipping write
[00:00:06] [========================================]     411/411     0x10000                                                                                             [2025-04-10T10:01:31Z INFO ] Flashing has completed!
```

If espflash reports the following error:

```
Error: espflash::connection_failed

  × Error while connecting to device
  ╰─▶ Serial port not found
```

You should to identify which serial port are connected to your esp board, and use `--port` to specify the used serial port:

```
# Suppose that the esp board are connected to /dev/cu.usbmodem211401
cargo run --release -- --port /dev/cu.usbmodem211401
```

If you want to get some insight of segments of your binary, [`espsegs`](https://github.com/bjoernQ/espsegs) would help:

```
# Install it first
cargo install --git https://github.com/bjoernQ/espsegs

# Check all segments
espsegs target/riscv32imc-unknown-none-elf/release/rmk-esp32c3 --chip esp32c3
```