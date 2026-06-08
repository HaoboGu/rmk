# rmk-host

Runtime-free host-side client for **Rynk**, RMK's native host-communication
protocol. Use it to read and write a running RMK keyboard's keymap, combos,
forks, morse, macros, and behavior, and to observe live status.

This crate owns the protocol state machine only. Device discovery, connection,
and byte I/O live in separate transport crates such as `rmk-host-serial` and
`rmk-host-ble`.

## Concepts

- **[`Client`]** drives the protocol over a [`Transport`]: handshake, a typed
  method per command, and pull-based topic delivery via `next_event`, which
  decodes each push into a typed `Event` (topics are best-effort — re-read a
  missed value with the matching `Get*` call). Requests are serialized through
  `&mut self` — no background task, no shared state.
- **[`Transport`]** is a byte pipe. A third-party transport is its own crate
  implementing `Transport` against `rmk-host`; an app depends on `rmk-host`
  plus that crate and calls [`Client::connect`].

## Example

```rust,no_run
# async fn run() -> Result<(), Box<dyn std::error::Error>> {
let mut client = rmk_host_serial::connect_serial().await?;
let caps = *client.capabilities();
println!("{}×{}×{} keymap", caps.num_layers, caps.num_rows, caps.num_cols);

let key = client.get_key(0, 0, 0).await?;
println!("L0(0,0) = {key:?}");
# Ok(()) }
```

Each method returns the response value directly; a device rejection surfaces as
`RequestError::Rejected`, so `?` propagates both transport and firmware errors.

## License

MIT OR Apache-2.0
