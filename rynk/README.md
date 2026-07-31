# rynk

Runtime-free host-side client for **Rynk**, RMK's native host-communication
protocol. Use it to read and write a running RMK keyboard's keymap, combos,
forks, morse, macros, and behavior, and to observe live status.

This crate owns the protocol state machine only. Device discovery, connection,
and byte I/O live in separate transport crates such as `rynk-serial` and
`rynk-ble`. The `rynk-kle` crate converts KLE exports / Vial `vial.json` ↔
RMK's `[layout]` and decodes layouts into `rynk::layout` types — natively and,
via its `wasm` feature, on the web; the `rmkit layout` CLI in
[rmkit](https://github.com/haobogu/rmkit) wraps it.

## Concepts

- **`RynkDevice`** is a discovered device handle. Its `connect` method opens the
  byte link, completes the version and capability handshake, and returns a
  `(Client, Driver)` session.
- **`Client`** provides typed requests and pull-based topic delivery through
  `next_topic`. Its methods take `&self`, so one task can issue requests while
  another consumes topics. Topics are best-effort; re-read missed state with
  the matching `get_*` method.
- **`Driver`** owns the link's reader and writer. The caller must run
  `Driver::run` alongside every future waiting on the client and stop using the
  client when the driver returns.
- **The byte link uses embedded-io-async `Read` and `Write`**, re-exported as
  `rynk::io` so transport crates use the same trait version. A third-party
  transport implements `RynkDevice::open`, returning `(Read, Write)` halves.

## Example

```rust,no_run
# async fn run() -> Result<(), Box<dyn std::error::Error>> {
// Discover marked Rynk keyboards, pick one, and open it (the handshake runs
// inside `connect`). `rynk-ble` mirrors this flow.
use embassy_futures::select::{Either, select};
use rynk::RynkDevice;
use rynk_serial::SerialDevice;
let device = SerialDevice::discover()?
    .into_iter()
    .next()
    .ok_or("no Rynk keyboard found")?;
let (client, mut driver) = device.connect().await?;

let work = async {
    let caps = client.get_capabilities().await?;
    println!("{}×{}×{} keymap", caps.num_layers, caps.num_rows, caps.num_cols);

    let key = client.get_key(0, 0, 0).await?;
    println!("L0(0,0) = {key:?}");
    Ok::<_, rynk::RynkHostError>(())
};
match select(driver.run(&client), work).await {
    Either::First(err) => return Err(err.into()),
    Either::Second(result) => result?,
}
# Ok(()) }
```

Each typed method returns its response value directly. Device rejections surface
as `RynkHostError::Rejected`; link failures return independently from
`Driver::run`.

## License

MIT OR Apache-2.0
