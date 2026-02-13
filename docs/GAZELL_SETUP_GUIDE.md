# Gazell 2.4G Wireless Setup Guide

Complete guide for setting up Nordic Gazell protocol support in RMK keyboard firmware.

## Overview

This guide covers:
- Installing Nordic nRF5 SDK
- Building RMK with Gazell support
- Flashing firmware to nRF52840 devices
- Testing the wireless connection
- Troubleshooting common issues

## Prerequisites

### Hardware Requirements

**Minimum Setup:**
- **Dongle**: nRF52840 Dongle (USB receiver)
- **Keyboard**: nRF52840 DK or custom PCB with nRF52840

**Supported MCUs:**
- nRF52840 (recommended - 1MB flash, 256KB RAM, USB)
- nRF52833 (512KB flash, 128KB RAM, USB)
- nRF52832 (512KB flash, 64KB RAM, no USB - requires external USB-to-serial)

**Development Tools:**
- USB cables (USB-A to micro-USB or USB-C depending on your boards)
- Debugger: J-Link, DAPLink, or ST-Link (for SWD programming)
  - nRF52840 DK has built-in J-Link debugger
  - nRF52840 Dongle requires DFU bootloader (no SWD exposed)

### Software Requirements

1. **Rust Toolchain**

   ```bash
   # Install Rust (if not already installed)
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

   # Add ARM Cortex-M4F target
   rustup target add thumbv7em-none-eabihf
   ```

2. **Build Tools**

   ```bash
   # Ubuntu/Debian
   sudo apt update
   sudo apt install -y \
       gcc-arm-none-eabi \
       libnewlib-arm-none-eabi \
       libstdc++-arm-none-eabi-newlib \
       llvm \
       clang \
       libclang-dev

   # macOS
   brew install arm-none-eabi-gcc llvm

   # Windows (using MSYS2)
   pacman -S mingw-w64-x86_64-arm-none-eabi-gcc mingw-w64-x86_64-clang
   ```

3. **Flashing Tool**

   ```bash
   # probe-rs (recommended)
   cargo install probe-rs-tools --locked

   # Alternative: nrfjprog (Nordic's official tool)
   # Download from: https://www.nordicsemi.com/Products/Development-tools/nRF-Command-Line-Tools
   ```

---

## Step 1: Install Nordic nRF5 SDK

### Download SDK

1. Go to Nordic's website: https://www.nordicsemi.com/Products/Development-software/nRF5-SDK

2. Download **nRF5 SDK v17.1.0** (or later):
   ```bash
   cd ~
   wget https://nsscprodmedia.blob.core.windows.net/prod/software-and-other-downloads/sdks/nrf5/binaries/nrf5_sdk_17.1.0_ddde560.zip
   ```

3. Extract the SDK:
   ```bash
   unzip nrf5_sdk_17.1.0_ddde560.zip -d ~/nRF5_SDK_17.1.0
   ```

### Set Environment Variable

**Linux/macOS:**

Add to `~/.bashrc` or `~/.zshrc`:
```bash
export NRF5_SDK_PATH=~/nRF5_SDK_17.1.0
```

Then reload:
```bash
source ~/.bashrc  # or source ~/.zshrc
```

**Windows (PowerShell):**

Add to your PowerShell profile:
```powershell
$env:NRF5_SDK_PATH = "C:\nRF5_SDK_17.1.0"
```

Or set permanently via System Properties > Environment Variables.

### Verify Installation

```bash
echo $NRF5_SDK_PATH
ls $NRF5_SDK_PATH/components/proprietary_rf/gzll
```

You should see Gazell library files (`.a` files in `gcc/` subdirectory).

---

## Step 2: Build Firmware

### Clone RMK Repository

```bash
git clone https://github.com/HaoboGu/rmk.git
cd rmk
```

### Build Dongle Firmware (Host/Receiver)

```bash
cd examples/use_rust/nrf52840_dongle
cargo build --release --target thumbv7em-none-eabihf
```

**Output:** `target/thumbv7em-none-eabihf/release/rmk-nrf52840-dongle`

### Build Keyboard Firmware (Device/Transmitter)

```bash
cd ../nrf52840_2g4
cargo build --release --target thumbv7em-none-eabihf
```

**Output:** `target/thumbv7em-none-eabihf/release/rmk-nrf52840-2g4`

### Build Troubleshooting

**Error: "NRF5_SDK_PATH not set"**

Solution: Set the environment variable as described in Step 1.

**Error: "bindgen: Unable to find libclang"**

Solution:
```bash
# Ubuntu/Debian
sudo apt install libclang-dev

# macOS
xcode-select --install

# Set LIBCLANG_PATH if needed
export LIBCLANG_PATH=/usr/lib/llvm-14/lib  # Adjust version as needed
```

**Error: "linker `rust-lld` not found"**

Solution:
```bash
rustup component add llvm-tools-preview
```

**Error: "cannot find -lgzll_nrf52840_gcc"**

Solution: Verify SDK path contains the library:
```bash
ls $NRF5_SDK_PATH/components/proprietary_rf/gzll/gcc/libgzll_nrf52840_gcc.a
```

---

## Step 3: Flash Firmware

### Option A: Flash Dongle (USB DFU Bootloader)

The nRF52840 Dongle comes with a bootloader that allows USB flashing.

1. **Enter DFU Mode:**
   - Press the RESET button on the dongle
   - Red LED should blink (DFU mode active)

2. **Convert ELF to HEX:**
   ```bash
   cd examples/use_rust/nrf52840_dongle
   cargo objcopy --release --target thumbv7em-none-eabihf -- -O ihex target/rmk-nrf52840-dongle.hex
   ```

3. **Flash using nrfutil:**
   ```bash
   # Install nrfutil (if not installed)
   pip3 install nrfutil

   # Create DFU package
   nrfutil pkg generate --hw-version 52 --sd-req 0x00 \
       --application target/rmk-nrf52840-dongle.hex \
       --application-version 1 dongle.zip

   # Flash
   nrfutil dfu usb-serial -pkg dongle.zip -p /dev/ttyACM0  # Linux
   # Or: nrfutil dfu usb-serial -pkg dongle.zip -p COM3    # Windows
   ```

### Option B: Flash via SWD (Development Boards)

For nRF52840 DK or custom boards with SWD access:

```bash
# Flash dongle
cd examples/use_rust/nrf52840_dongle
probe-rs run --chip nRF52840_xxAA --release

# Flash keyboard
cd ../nrf52840_2g4
probe-rs run --chip nRF52840_xxAA --release
```

**Alternative with nrfjprog:**
```bash
# Convert to HEX first
arm-none-eabi-objcopy -O ihex target/thumbv7em-none-eabihf/release/rmk-nrf52840-2g4 firmware.hex

# Flash
nrfjprog --program firmware.hex --chiperase --verify --reset
```

---

## Step 4: Test Wireless Connection

### Monitor Logs

**Terminal 1 - Dongle Logs:**
```bash
# If using probe-rs with RTT logging
probe-rs attach --chip nRF52840_xxAA
# Or monitor serial output if using serial logging
```

**Terminal 2 - Keyboard Logs:**
```bash
probe-rs attach --chip nRF52840_xxAA
```

### Expected Output

**Dongle (Host Mode):**
```
INFO  RMK nRF52840 Dongle starting...
INFO  Gazell: Initialized (channel=2, rate=2Mbps, power=0dBm)
INFO  Gazell: Set to host mode (receiver)
INFO  USB initialized, waiting for host connection...
INFO  Dongle ready! Listening for keyboard packets on 2.4GHz...
INFO  Received 2.4G packet: 3 bytes
INFO  Elink frame type: 0xAA
```

**Keyboard (Device Mode):**
```
INFO  RMK nRF52840 2.4G Keyboard starting...
INFO  Gazell: Initialized (channel=2, rate=2Mbps, power=0dBm)
INFO  Gazell: Set to device mode (transmitter)
INFO  Keyboard ready! Starting test transmission...
INFO  Sent test packet #0 successfully
INFO  Sent test packet #1 successfully
...
```

### Verify USB HID

1. Plug dongle into PC
2. Check device enumeration:

   **Linux:**
   ```bash
   lsusb | grep RMK
   # Should show: Bus 001 Device 005: ID 1209:0001 RMK Dongle
   ```

   **Windows:**
   - Open Device Manager
   - Look under "Human Interface Devices"
   - Should see "RMK Dongle"

3. Test input (once keyboard integration is complete):
   ```bash
   # Linux - monitor HID events
   sudo evtest /dev/input/eventX  # Find correct eventX using evtest --list
   ```

---

## Step 5: Performance Testing

### Latency Test

Measure keyboard-to-USB latency:

```bash
# Connect logic analyzer to:
# - Keyboard: GPIO pin that toggles on key press
# - Dongle: USB D+/D- lines

# Measure time from GPIO toggle to USB packet transmission
# Target: < 5ms end-to-end latency
```

### Packet Loss Test

Run long-term stability test:

```bash
# On keyboard firmware, add packet counter
# On dongle firmware, check for missing sequence numbers
# Target: < 0.01% packet loss over 1 hour
```

### Range Test

Measure maximum operating distance:

1. Place dongle in fixed location
2. Move keyboard away incrementally
3. Monitor RSSI and packet success rate
4. Typical range: 10-15 meters line-of-sight

---

## Troubleshooting

### Build Issues

**Problem:** `error: linking with 'rust-lld' failed`

**Solution:** Check memory.x file exists and memory regions are correct:
```ld
MEMORY
{
  FLASH : ORIGIN = 0x00000000, LENGTH = 1024K
  RAM : ORIGIN = 0x20000000, LENGTH = 256K
}
```

**Problem:** `undefined reference to '__aeabi_dmul'`

**Solution:** Add to `.cargo/config.toml`:
```toml
[target.thumbv7em-none-eabihf]
rustflags = [
  "-C", "link-arg=-Tlink.x",
  "-C", "link-arg=--nmagic",
]
```

### Runtime Issues

**Problem:** Dongle LED doesn't light up after flashing

**Causes:**
- Firmware not flashed correctly
- Bootloader corrupted
- Power supply issue

**Solution:**
- Re-flash using nrfjprog with `--recover` option
- Check USB cable and port
- Try different USB port (some ports provide insufficient power)

**Problem:** No wireless packets received

**Causes:**
- Keyboard and dongle on different channels
- Base address mismatch
- Out of range
- Nordic SDK library not linked

**Solution:**
1. Verify both use same `GazellConfig`:
   ```rust
   let config = GazellConfig::low_latency();
   ```

2. Check build logs for Gazell library linking:
   ```
   cargo:warning=Linking Gazell library: libgzll_nrf52840_gcc.a
   ```

3. Reduce distance to < 1 meter for initial testing

4. Check channel is not used by WiFi:
   ```bash
   # On Linux with WiFi analyzer
   iwlist wlan0 scan | grep Channel
   ```

**Problem:** Intermittent packet loss

**Causes:**
- WiFi interference (2.4GHz band)
- Multiple Gazell devices on same channel
- Low battery (for battery-powered keyboard)

**Solution:**
- Change Gazell channel:
  ```rust
  let mut config = GazellConfig::low_latency();
  config.channel = 25;  // Try different channels (0-100)
  ```

- Increase TX power:
  ```rust
  config.tx_power = 4;  // +4 dBm (check local regulations)
  ```

- Enable retries:
  ```rust
  config.max_retries = 5;
  ```

### Debugging Tips

1. **Enable verbose logging:**
   ```rust
   // In main.rs, set log level
   defmt::set_log_level(defmt::LevelFilter::Trace);
   ```

2. **Check Gazell state:**
   ```rust
   if gazell.is_ready() {
       info!("Gazell ready");
   } else {
       error!("Gazell not ready!");
   }
   ```

3. **Monitor with oscilloscope:**
   - Probe RF output (with proper attenuator)
   - Check 2.4GHz signal presence
   - Verify packet timing

4. **Use Nordic's Power Profiler Kit:**
   - Measure current consumption
   - Identify sleep mode issues
   - Optimize battery life

---

## Advanced Configuration

### Custom Addresses

For multiple keyboards with one dongle:

```rust
// Keyboard 1
let mut config = GazellConfig::low_latency();
config.address_prefix = 0xC2;  // Default

// Keyboard 2
config.address_prefix = 0xC3;  // Different address
```

Dongle must listen on all pipes (handled automatically by RMK).

### Low-Latency vs. Low-Power

**Low Latency (gaming keyboards):**
```rust
let config = GazellConfig::low_latency();
// - 2Mbps data rate
// - Channel 2 (minimal WiFi interference)
// - 250µs ACK timeout
// - Low retries (3)
```

**Low Power (office keyboards):**
```rust
let config = GazellConfig::balanced();
// - 1Mbps data rate
// - Medium retries (5)
// - Can add sleep modes in main loop
```

### Security Considerations

⚠️ **Warning:** Current implementation does not include encryption.

For production keyboards:
- Use Nordic's ESB (Enhanced ShockBurst) with AES encryption
- Implement pairing mechanism
- Add device authentication

**Roadmap:** AES-CCM encryption support planned for future release.

---

## Next Steps

Once basic wireless is working:

1. **Integrate with Key Matrix:**
   - Replace test packets with actual key scan data
   - Use RMK's matrix scanning APIs

2. **Add Elink Protocol:**
   - Encode keyboard reports using Elink frames
   - Handle split keyboard scenarios

3. **Implement Battery Monitoring:**
   - Read ADC for battery voltage
   - Send battery level in frames

4. **Add Low-Power Modes:**
   - Sleep between key scans
   - Wake on key press interrupt

5. **Multi-Device Support:**
   - Use `DeviceManager` on dongle side
   - Implement device switching (Fn+1/2/3)

---

## Resources

### Documentation

- [Nordic Gazell Documentation](https://infocenter.nordicsemi.com/topic/sdk_nrf5_v17.1.0/group__gzll.html)
- [nRF52840 Product Specification](https://infocenter.nordicsemi.com/pdf/nRF52840_PS_v1.8.pdf)
- [RMK GitHub Repository](https://github.com/HaoboGu/rmk)
- [Elink Protocol Specification](../elink-protocol/docs/PROTOCOL.md)

### Community

- [RMK Discord Server](https://discord.gg/rmk) (TODO: add actual link)
- [GitHub Issues](https://github.com/HaoboGu/rmk/issues)

### Hardware

- [nRF52840 Dongle](https://www.nordicsemi.com/Products/Development-hardware/nrf52840-dongle)
- [nRF52840 DK](https://www.nordicsemi.com/Products/Development-hardware/nrf52840-dk)
- [nice!nano v2](https://nicekeyboards.com/nice-nano/) - nRF52840 keyboard controller

---

## License

RMK is dual-licensed under MIT/Apache-2.0.

Nordic nRF5 SDK is licensed under Nordic 5-Clause License. Users must comply with Nordic's license terms.

---

## Contributing

Found an issue or want to improve this guide? Please submit a PR or open an issue on GitHub!

**Tested Configurations:**
- ✅ nRF52840 Dongle + nRF52840 DK (Ubuntu 22.04, SDK v17.1.0)
- ⏳ nRF52833 DK + nRF52833 DK (testing in progress)
- ⏳ nRF52832 DK + nRF52832 DK (testing in progress)

Please report your test results to help others!
