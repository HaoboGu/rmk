# Gazell FFI Integration Plan (nRF52-only)

> Goal: follow the same *engineering pattern* as `nrf-sdc`, but for Gazell.
> Note: `nrf-sdc` is BLE SDC, so we only reuse its **FFI structure**, not code.

## Why this plan

- No mature Rust Gazell SDK
- Gazell lives in Nordic nRF5 SDK (C)
- We need a minimal, stable Rust interface under `WirelessTransport`

## High-level architecture

```
rmk/
├─ rmk-gazell-sys/          # C shim + bindgen + build.rs
│  ├─ build.rs
│  ├─ src/lib.rs            # raw FFI bindings
│  └─ c/gazell_shim.c        # minimal C wrapper
├─ rmk/src/wireless/gazell.rs  # safe-ish Rust wrapper
└─ rmk/src/wireless/mod.rs
```

## Layering (mirrors nrf-sdc style)

1. **C shim**
   - Small C wrapper around Gazell API
   - Hides SDK headers and macros from Rust
   - Exposes a *flat* C API for bindgen

2. **`rmk-gazell-sys`**
   - `build.rs` compiles shim with `cc`
   - `bindgen` generates `extern "C"` bindings
   - Exposes raw, unsafe functions

3. **Rust wrapper**
   - `GazellTransport` implements `WirelessTransport`
   - Owns init/tx/rx state
   - Converts raw errors to `WirelessError`

## Minimal C shim API (example)

```c
// c/gazell_shim.h
int gz_init(const struct gz_config* cfg);
int gz_set_device_mode(void);
int gz_set_host_mode(void);
int gz_send(const uint8_t* data, uint8_t len);
int gz_recv(uint8_t* out, uint8_t* out_len);
```

## Rust wrapper API (example)

```rust
pub struct GazellTransport {
    config: GazellConfig,
    initialized: bool,
}

impl WirelessTransport for GazellTransport {
    fn send_frame(&mut self, frame: &[u8]) -> Result<()> {
        // call gz_send
    }
    fn recv_frame(&mut self) -> Result<Option<Vec<u8, 64>>> {
        // call gz_recv
    }
}
```

## Feature gating

- `features = ["wireless_gazell"]`
- Only build `rmk-gazell-sys` for nRF52 targets

## Build steps (sketch)

1. `build.rs` locates nRF5 SDK
2. `cc` compiles `gazell_shim.c` against SDK headers
3. `bindgen` generates Rust bindings
4. `rmk` enables `wireless_gazell` feature to link sys crate

## Risks & mitigations

- **SDK path management**: require `NRF5_SDK_PATH` env var
- **Licensing**: Nordic SDK license must be respected
- **Build complexity**: keep shim minimal; avoid deep SDK deps

## Next steps

1. Decide SDK path / version
2. Define shim header + config structs
3. Implement `rmk-gazell-sys` crate skeleton
4. Replace mock in `rmk/src/wireless/gazell.rs`

