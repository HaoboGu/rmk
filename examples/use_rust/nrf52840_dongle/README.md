# RMK nRF52840 Dongle Firmware

USB dongle firmware for RMK 2.4G wireless keyboards.

## Overview

This dongle acts as a bridge between a 2.4G wireless RMK keyboard and a PC:

```
Keyboard (2.4G) → Dongle (USB) → PC
```

The dongle:
- Receives Elink frames over 2.4G (Nordic Gazell protocol)
- Extracts keyboard events from the frames
- Forwards them to the PC as USB HID reports

## Hardware Requirements

- **nRF52840 Dongle** (PCA10059) or compatible board
- USB port for connection to PC
- 2.4GHz antenna (built-in on nRF52840 Dongle)

## Building

```bash
# Install target if not already installed
rustup target add thumbv7em-none-eabihf

# Build release firmware
cargo build --release
```

## Flashing

### Method 1: Using probe-rs (Recommended)

```bash
# Flash directly
cargo flash --chip nRF52840 --release

# Or run with logging
cargo run --release
```

### Method 2: Using nrfutil (for nRF52840 Dongle without debugger)

```bash
# 1. Build the firmware
cargo build --release

# 2. Convert to hex
arm-none-eabi-objcopy -O ihex \
    target/thumbv7em-none-eabihf/release/rmk-nrf52840-dongle \
    dongle.hex

# 3. Generate DFU package
nrfutil pkg generate --hw-version 52 --sd-req 0x00 \
    --application dongle.hex --application-version 1 \
    dongle.zip

# 4. Put dongle in DFU mode (press RESET button)

# 5. Flash via DFU
nrfutil dfu usb-serial -pkg dongle.zip -p /dev/ttyACM0
```

## Usage

1. **Flash the dongle** using one of the methods above
2. **Plug the dongle into your PC**
3. **Power on your 2.4G RMK keyboard**
4. The dongle should automatically connect and forward key presses

## LED Status Indicators

- **No LED**: Waiting for USB connection
- **Solid LED**: USB connected, waiting for keyboard
- **Blinking LED**: Receiving data from keyboard

(Note: LED behavior depends on board design)

## Debugging

### View logs via probe-rs

```bash
# Run with RTT logging
cargo run --release
```

### View logs via Serial

Some boards have a serial port that can be used for logging:

```bash
# Monitor serial output
screen /dev/ttyACM0 115200
```

## Configuration

### USB VID/PID

Edit `src/main.rs`:

```rust
let mut config = Config::new(0x1209, 0x0001); // Change these values
```

### 2.4G Channel

Edit Gazell configuration (when implemented):

```rust
let config = GazellConfig {
    channel: 4, // Change channel here
    ..Default::default()
};
```

## Development Status

### ✅ Implemented
- USB HID device initialization
- USB HID keyboard report sending
- Project structure and build system

### 🚧 In Progress
- 2.4G Gazell receiver implementation
- Elink frame parsing
- Message routing from 2.4G to USB

### ⏳ Planned
- LED status indicators
- Multi-device pairing
- Battery level monitoring
- Configuration via USB

## Troubleshooting

### Dongle not recognized by PC

- Try different USB port
- Check USB cable
- Re-flash the firmware

### No input from keyboard

- Check keyboard is powered on
- Verify 2.4G channel matches keyboard
- Check logs for reception errors

### Build errors

```bash
# Update dependencies
cargo update

# Clean build
cargo clean && cargo build --release
```

## Related Documentation

- [RMK 2.4G Development Roadmap](../../../docs/RMK_2G4_DEVELOPMENT.md)
- [Elink Protocol Specification](../../../../elink-protocol/docs/protocol-specification-en.md)
- [Nordic Gazell Protocol](https://infocenter.nordicsemi.com/topic/com.nordic.infocenter.sdk5.v15.0.0/group__gzll.html)

## License

Same as RMK project.
