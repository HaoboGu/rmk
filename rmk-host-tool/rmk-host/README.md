# rmk-host

Runtime-free host-side client for **Rynk**, RMK's native host-communication
protocol.

This crate is transport-agnostic and pulls in no transport or async-runtime
dependencies. [`Client`] drives the Rynk protocol over any [`Transport`] — a
byte link to a device.

- implement [`Transport`] (two async methods, `send` / `recv`),
- hand the transport to [`Client::connect`].

Native serial, native BLE, and web transports live in separate crates. Apps
depend on `rmk-host` plus the transport crate they need.

## License

MIT OR Apache-2.0
