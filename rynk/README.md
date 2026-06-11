# rynk

Runtime-free host-side client for **Rynk**, RMK's native host-communication
protocol. Use it to read and write a running RMK keyboard's keymap, combos,
forks, morse, macros, and behavior, and to observe live status.

This crate owns the protocol state machine only. Device discovery, connection,
and byte I/O live in separate transport crates such as `rynk-serial` and
`rynk-ble`.

## Concepts

- **[`Client`]** drives the protocol over any byte link: handshake, typed
  methods for the command surface, and pull-based topic delivery via
  `next_event`, which decodes each push into a typed `IncomingTopic` (topics are
  best-effort — re-read a missed value with the matching `Get*` call).
  Requests are serialized through `&mut self` — no background task, no shared
  state.
- **The byte link is embedded-io-async `Read + Write`** — the same traits the
  firmware's session loop reads, re-exported as `rynk::io` so the trait version
  always matches. A third-party transport is its own crate implementing them
  (anything with an embedded-io adapter already qualifies); an app depends on
  `rynk` plus that crate and calls [`Client::connect`].

## Example

```rust,no_run
# async fn run() -> Result<(), Box<dyn std::error::Error>> {
// Discover marked Rynk keyboards, pick one, and open it (the handshake runs
// inside `connect`). `rynk-ble` mirrors this flow with `discover().await`.
let device = rynk_serial::discover()?
    .into_iter()
    .next()
    .ok_or("no Rynk keyboard found")?;
let mut client = device.connect().await?;

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
