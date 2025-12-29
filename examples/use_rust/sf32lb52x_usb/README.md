# RMK SF32LB52x USB example

> [!NOTE]
> **Maintenance & Compatibility Note**
>
> This example relies on specific HALs that may have different release cycles compared to the upstream Embassy/RMK.
>
> To ensure stability, this example **locks the RMK dependency to a specific version** and is **not actively updated** to track the RMK `main` branch daily. It is maintained on a periodic basis to align with major release milestones.

## Flash and Run

### 1\. Flashing the Firmware

First, you need to configure your serial port. You will find a `runner` command in `.cargo/config.toml`:

```toml
runner = 'sftool -c SF32LB52 --port <YOUR_PORT_HERE> write_flash ftab.bin@0x12000000'
```

Simply replace `<YOUR_PORT_HERE>` with your device's serial port:

  * **Linux/Mac**: e.g., `/dev/ttyUSB0`
  * **Windows**: e.g., `COM9`

After saving the change, you can flash the firmware by running:

```bash
cargo run
```

### 2\. Debugging and Viewing RTT Logs

You can use `probe-rs` to `attach` to the chip and view defmt RTT (Real-Time Transfer) logs from your application.

SF32LB52x uses UART to emulate SWD (SWD-over-UART). To allow probe-rs to recognize your serial port as a `SiFli uart debug probe`, you must set the `SIFLI_UART_DEBUG=1` environment variable (This variable tells probe-rs to treat all serial ports as potential debug ports)

Use the following commands in your terminal to set the variable and start the attach process:

  * **For Linux/Mac (sh):**

    ```sh
    SIFLI_UART_DEBUG=1 probe-rs attach --chip SF32LB52 examples/use_rust/sf32lb52x_usb/target/thumbv8m.main-none-eabihf/debug/rmk-sf32lb52x-usb
    ```

  * **For Windows (PowerShell):**

    ```powershell
    $env:SIFLI_UART_DEBUG=1; probe-rs attach --chip SF32LB52 examples\use_rust\sf32lb52x_usb\target\thumbv8m.main-none-eabihf\debug\rmk-sf32lb52x-usb
    ```

### **Notes**

  * **probe-rs**: Please ensure the version is greater than `0.28.0` to support sf32.  **Currently, `probe-rs` only supports `attach`, not `run` or `download`.**
  * **Bootloader**: We do not provide a bootloader because the chip comes with one from the factory. As long as you have not erased it, there is no need to flash it again. If you need to flash the bootloader, please flash any example in the [SiFli-SDK](https://github.com/OpenSiFli/SiFli-SDK).
  * **ftab.bin**: The flash table, which contains metadata about the firmware, such as its size. The runner command automatically flashes both your firmware and the `ftab.bin` to the correct address.

**For more information, see:**  

 - [SiFli-rs Flash and Debug Guide](https://github.com/OpenSiFli/sifli-rs/blob/main/docs/flash_and_debug.md)  
 - [SiFli Wiki](https://wiki.sifli.com/)
 - [sftool](https://github.com/OpenSiFli/sftool)
 - [SiFli-hal](https://github.com/OpenSiFli/sifli-rs)

SF32LB52x is a dual-mode Bluetooth (BT/BLE) MCU. However, support for its Bluetooth features has not yet been implemented in the [sifli-rs]([https://github.com/OpenSiFli/sifli-rs](https://github.com/OpenSiFli/sifli-rs)) .

If you are interested, contributions are welcome\! Please visit the [sifli-rs](https://github.com/OpenSiFli/sifli-rs) repository to get started.

