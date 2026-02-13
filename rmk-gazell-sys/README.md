# rmk-gazell-sys

Low-level FFI bindings to Nordic Gazell protocol for nRF52 series MCUs.

This is a `-sys` crate providing unsafe bindings to the Nordic nRF5 SDK's Gazell protocol implementation. Most users should use the safe wrapper provided by `rmk::wireless::GazellTransport` instead.

## Overview

rmk-gazell-sys provides a minimal C shim layer that wraps the Nordic Gazell SDK and exposes a simple C API that is bound to Rust using bindgen. This follows the same architecture pattern as `nrf-sdc-sys`.

### Architecture

```
┌─────────────────────────────────────┐
│  rmk::wireless::GazellTransport     │  ← Safe Rust API
│  (rmk/src/wireless/gazell.rs)      │
└──────────────┬──────────────────────┘
               │
┌──────────────▼──────────────────────┐
│  rmk-gazell-sys (this crate)        │  ← Unsafe FFI bindings
│  - Rust bindings (bindgen)          │
│  - C shim (gazell_shim.c)           │
└──────────────┬──────────────────────┘
               │
┌──────────────▼──────────────────────┐
│  Nordic nRF5 SDK v17.1.0            │  ← Gazell protocol stack
│  (External dependency)              │
└─────────────────────────────────────┘
```

## Prerequisites

### 1. Hardware

This crate supports the following Nordic MCUs:

- nRF52840 (recommended for keyboards)
- nRF52833
- nRF52832

### 2. Nordic nRF5 SDK

You must download and install the Nordic nRF5 SDK v17.1.0 or later.

**Download**:
- Official website: https://www.nordicsemi.com/Products/Development-software/nRF5-SDK
- Direct link (v17.1.0): https://nsscprodmedia.blob.core.windows.net/prod/software-and-other-downloads/sdks/nrf5/binaries/nrf5_sdk_17.1.0_ddde560.zip

**Installation**:

```bash
# Download and extract
wget https://nsscprodmedia.blob.core.windows.net/prod/software-and-other-downloads/sdks/nrf5/binaries/nrf5_sdk_17.1.0_ddde560.zip
unzip nrf5_sdk_17.1.0_ddde560.zip -d ~/nRF5_SDK_17.1.0

# Set environment variable
export NRF5_SDK_PATH=~/nRF5_SDK_17.1.0
```

**Permanent setup** (add to `~/.bashrc` or `~/.zshrc`):

```bash
export NRF5_SDK_PATH=~/nRF5_SDK_17.1.0
```

### 3. Build Tools

- **Rust toolchain**: Install from https://rustup.rs
- **ARM target**: `rustup target add thumbv7em-none-eabihf`
- **GCC ARM toolchain**: For compiling C code
  - Ubuntu/Debian: `sudo apt install gcc-arm-none-eabi`
  - macOS: `brew install arm-none-eabi-gcc`
  - Windows: Install from ARM's website

## Usage

### As a dependency

Add to your `Cargo.toml`:

```toml
[dependencies]
rmk-gazell-sys = { path = "../rmk-gazell-sys", optional = true }

[features]
wireless_gazell_nrf52840 = ["rmk-gazell-sys/nrf52840"]
```

### Building

```bash
# Set SDK path (if not in environment)
export NRF5_SDK_PATH=/path/to/nRF5_SDK_17.1.0

# Build for nRF52840
cargo build --target thumbv7em-none-eabihf --features nrf52840

# Build for nRF52833
cargo build --target thumbv7em-none-eabihf --features nrf52833

# Build for nRF52832
cargo build --target thumbv7em-none-eabihf --features nrf52832
```

### Example (unsafe FFI usage)

This crate provides unsafe bindings. Most users should use the safe wrapper instead.

```rust
use rmk_gazell_sys as sys;

unsafe {
    // Configure Gazell
    let config = sys::gz_config_t {
        channel: 2,
        data_rate: 2,  // 2 Mbps
        tx_power: 0,   // 0 dBm
        max_retries: 3,
        ack_timeout_us: 250,
        base_address: [0xE7, 0xE7, 0xE7, 0xE7],
        address_prefix: 0xC2,
    };

    // Initialize
    let result = sys::gz_init(&config);
    if result != sys::GZ_OK {
        panic!("Gazell init failed");
    }

    // Set device mode (transmitter)
    sys::gz_set_mode(sys::GZ_MODE_DEVICE);

    // Send a packet
    let data = [0xAA, 0xBB, 0xCC];
    let result = sys::gz_send(data.as_ptr(), data.len() as u8);
    if result != sys::GZ_OK {
        panic!("Send failed");
    }

    // Cleanup
    sys::gz_deinit();
}
```

### Recommended: Use Safe Wrapper

Instead of using this crate directly, use the safe wrapper:

```rust
use rmk::wireless::{GazellTransport, GazellConfig, WirelessTransport};

let config = GazellConfig::low_latency();
let mut transport = GazellTransport::new(config);

transport.init()?;
transport.set_device_mode()?;

let frame = [0xAA, 0xBB, 0xCC];
transport.send_frame(&frame)?;
```

## API Reference

### Error Codes

- `GZ_OK` (0): Success
- `GZ_ERR_SEND_FAILED` (-1): Transmission failed
- `GZ_ERR_RECEIVE_FAILED` (-2): Reception failed
- `GZ_ERR_FRAME_TOO_LARGE` (-3): Frame exceeds 32 bytes
- `GZ_ERR_NOT_INITIALIZED` (-4): Gazell not initialized
- `GZ_ERR_BUSY` (-5): TX FIFO full
- `GZ_ERR_INVALID_CONFIG` (-6): Invalid configuration
- `GZ_ERR_HARDWARE` (-7): Hardware error

### Functions

- `gz_init(config)`: Initialize Gazell with configuration
- `gz_set_mode(mode)`: Set device mode (DEVICE or HOST)
- `gz_send(data, len)`: Send frame (blocking with timeout)
- `gz_recv(buf, len, max)`: Receive frame (non-blocking)
- `gz_is_ready()`: Check if ready to transmit
- `gz_flush()`: Flush TX/RX FIFOs
- `gz_deinit()`: Deinitialize Gazell

See `c/gazell_shim.h` for detailed API documentation.

## Features

### Chip Support

- `nrf52840`: Enable support for nRF52840 (default for RMK)
- `nrf52833`: Enable support for nRF52833
- `nrf52832`: Enable support for nRF52832

You must enable exactly one chip feature when building.

## Troubleshooting

### Build Errors

**"NRF5_SDK_PATH not set"**

Set the environment variable:
```bash
export NRF5_SDK_PATH=/path/to/nRF5_SDK_17.1.0
```

**"No chip feature enabled"**

Enable one chip feature:
```bash
cargo build --features nrf52840
```

**"Failed to generate bindings"**

Ensure bindgen dependencies are installed:
- Ubuntu/Debian: `sudo apt install llvm-dev libclang-dev clang`
- macOS: `xcode-select --install`

**Linker errors about missing Gazell library**

Verify the SDK path is correct and contains:
```
$NRF5_SDK_PATH/components/proprietary_rf/gzll/gcc/libgzll_nrf52840_gcc.a
```

## License

The Rust bindings and C shim code in this crate are dual-licensed under:

- MIT License
- Apache License, Version 2.0

**Nordic nRF5 SDK**: The Nordic nRF5 SDK (including the Gazell protocol stack) is licensed under the Nordic 5-Clause License. Users must comply with Nordic's license terms when using this SDK. See the SDK documentation for details.

## Resources

- [Nordic Gazell Documentation](https://infocenter.nordicsemi.com/topic/sdk_nrf5_v17.1.0/group__gzll.html)
- [nRF5 SDK Download](https://www.nordicsemi.com/Products/Development-software/nRF5-SDK)
- [RMK Documentation](https://github.com/HaoboGu/rmk)

## Contributing

This is part of the RMK keyboard firmware project. For issues and contributions, see the main RMK repository.
