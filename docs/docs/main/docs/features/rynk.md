# Rynk

Rynk is RMK's own protocol for configuring a keyboard while it's running — change
your keymap, layers, combos, tap-dance, macros and more without reflashing the
firmware. It plays the same role as [Vial](./vial_support), but it is built
natively for RMK and understands every RMK feature.

::: tip Rynk or Vial — which should I use?
Rynk speaks RMK's full feature set, but it is opt-in and its host tools are still
young. [Vial](./vial_support) is enabled by default and
has a polished cross-platform app, but only supports the features in the Vial
standard.

If you want a click-and-go app today, choose Vial. If you want access to every
RMK feature and don't mind newer tooling, choose Rynk.
:::

## What you can configure

Through Rynk, a host tool can read and change:

- Keymap entries per layer, plus encoders and the default layer
- Combos, forks, tap-dance / morse keys, and macros
- Tap-hold and other behavior settings

It can also watch live status — the current layer, a key tester (matrix state),
typing speed (WPM), the caps-lock/num-lock indicators, battery level, and, on
wireless boards, the connection and BLE profile (including switching or clearing
a profile). Finally it can reboot the keyboard, enter the bootloader, and reset
stored settings.

## Enable Rynk

Rynk and Vial are mutually exclusive — pick one. For `keyboard.toml` projects,
select Rynk in `[host]`:

```toml title="keyboard.toml"
[host]
vial_enabled = false
rynk_enabled = true
```

Then disable RMK's default features, which include Vial, and enable `rynk`
explicitly. Add the other features your keyboard needs:

```toml title="Cargo.toml"
rmk = { version = "...", default-features = false, features = [
    "defmt",
    "storage",
    "rynk",
    "watchdog",
    "rp2040",
] }
```

::: warning
The `[host]` setting and the Cargo feature must agree. For `keyboard.toml`
projects RMK checks this when it builds: if the `rynk` feature is on,
`rynk_enabled` must be `true` (and `vial_enabled` `false`), otherwise the build
stops with an error. To use Vial instead, flip both settings and swap the `rynk`
feature for `vial`.
:::

::: warning
Keep the `storage` feature enabled (it's on by default) so your changes survive
a reboot. Without it, anything you set over Rynk is lost when the keyboard
restarts. See [Storage](./storage).
:::

## Connecting to your keyboard

Rynk works over the connection you already use:

- **USB** — plug the keyboard in. Host tools find RMK keyboards automatically, so
  you don't have to hunt for the right serial port.
- **Bluetooth** — native tools connect to an already-connected keyboard (the OS
  must have it bonded and currently connected).
- **Browser** — Chromium browsers (Chrome or Edge) connect over USB (Web Serial)
  or Bluetooth (WebHID). Firefox and Safari are not supported.

Rynk tooling is still young. Today you have two options:

- **A browser demo** — the `rynk-wasm` package ships a reference web page
  (`index.html`) you build and serve locally; follow the steps in its README.
- **Rust libraries** — the `rynk`, `rynk-serial`, and `rynk-ble` crates let you
  build a native tool (see [For tool authors](#for-tool-authors) below). The
  browser build is a separate package, `rynk-wasm`.

A ready-made desktop app is not available yet.

## Locking dangerous operations

By default Rynk locks the operations that could plant a persistent implant or
leak what you type, so a background process — or a web page you granted access
to once — can't quietly reflash your keyboard or read your keystrokes. To use a
locked operation you prove you're physically present by holding a key combo.

There are three tiers:

- **Open** — everything you normally reach for: read the keymap, change keys,
  layers, combos, macros, switch BLE profiles, reboot. These stay available so
  on-the-fly configuration is friction-free.
- **Locked** — the dangerous ones, always gated: entering the bootloader,
  resetting stored settings and bonds, reading the live key matrix (a keylogger
  if left open), and clearing a BLE bond. A host tool gets a "locked" error
  until you unlock.
- **Config writes** — open by default, because on-the-fly configuration is the
  point. Set `write_requires_unlock = true` to move every write (keymap, macros,
  …) into the locked tier as well.

### Unlocking

Set the keys the owner holds to unlock in `[host].unlock_keys` (up to four):

```toml title="keyboard.toml"
[host]
rynk_enabled = true
unlock_keys = [[0, 0], [3, 12]]  # hold (row 0, col 0) and (row 3, col 12)
```

When you trigger a locked action, the host tool reads the challenge and shows
"hold these keys". Hold them together; the tool polls briefly until the device
unlocks, and the session stays unlocked until it ends. Unplugging, a Bluetooth
disconnect, or an explicit lock re-locks the device.

If you leave `unlock_keys` unset, the locked operations can never be unlocked —
a safe default, but it means a fresh config can't enter the bootloader over Rynk
or use the matrix tester until you add the keys.

For local development you can bypass the gate entirely:

```toml title="keyboard.toml"
[host]
insecure = true  # start and stay unlocked — don't ship this
```

::: warning Macros and combos are readable
Config _reads_ stay open, so anything that can reach the protocol can read your
stored macros. Don't put passwords or other secrets in a macro.
:::

::: tip Two different `unlock_keys`
`[host].unlock_keys` guards this Rynk lock gate. A separate `[dfu].unlock_keys`
guards the firmware _download_ once the device is already in the bootloader (the
`dfu_lock` feature). They're independent — set either, both, or neither. A setup
that cares most about firmware replacement wants both: the Rynk gate blocks the
remote route into the bootloader, and `dfu_lock` covers a physically-present
attacker who reaches it another way.
:::

## Advanced tuning

Rynk's firmware buffers size themselves automatically and rarely need touching.
There are several parameters in `keyboard.toml`'s [`[rmk]`](../configuration/rmk_config#rynk-protocol-configuration) section that you can adjust:

- `rynk_buffer_size`: the buffer size used for encoding/decoding Rynk message. A larger buffer moves more per round-trip at the cost of RAM.
- `protocol_macro_chunk_size`: macro chunk size.

The optional `bulk` Cargo feature turns on faster bulk transfers. The bulk size is limited by `rynk_buffer_size`. Enable it on boards that have room to spare:

```toml title="Cargo.toml"
rmk = { version = "...", features = ["bulk", "rp2040"] }
```

Host tools work with or without `bulk` — the keyboard tells the tool whether it
supports bulk transfers, so the same tool works either way.

## For tool authors

If you're building your own host tool, RMK ships ready-made client crates so you
don't have to implement the protocol yourself:

- `rynk` — the core typed client; talks to a keyboard over any byte link.
- `rynk-serial` — USB discovery and connection.
- `rynk-ble` — native Bluetooth discovery and connection.
- `rynk-wasm` — the browser build, driven from JavaScript over Web Serial / WebHID.

A minimal native USB tool looks like this:

```rust
use embassy_futures::select::{Either, select};
use rynk::RynkDevice;
use rynk_serial::SerialDevice;

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Find a connected Rynk keyboard and open it (the handshake runs in `connect`).
    let device = SerialDevice::discover()?
        .into_iter()
        .next()
        .ok_or("no Rynk keyboard found")?;

    let (client, mut driver) = device.connect().await?;
    let work = async {
        let caps = client.get_capabilities().await?;
        println!("{}x{}x{} keymap", caps.num_layers, caps.num_rows, caps.num_cols);

        let key = client.get_key(0, 0, 0).await?;
        println!("L0(0,0) = {key:?}");
        Ok::<_, rynk::RynkHostError>(())
    };

    match select(driver.run(&client), work).await {
        Either::First(err) => return Err(err.into()),
        Either::Second(result) => result?,
    }
    Ok(())
}
```

Live status arrives as "topics" you poll for in work running alongside the
driver:

```rust
loop {
    let topic = client.next_topic().await;
    println!("{topic:?}");
}
```

Topics are best-effort. If you miss one, just read the current value again with
the matching `get_*` method.

Firmware and host share the same message types (via `rmk-types`), so the two ends
can never disagree about a message's format. The client crates handle all the
encoding for you, so you never touch the raw bytes on the wire.
