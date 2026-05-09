# Replace postcard-rpc with a custom `rmk_protocol` wire layer

## Context

Decision (from prior research, see end of file): **drop `postcard-rpc` and `cobs`** from `rmk_protocol`, keep `postcard` for payload serialization, replace the RPC layer with a flat command dispatcher and a fixed-shape header. Motivations: DX/dispatch ergonomics, code/binary size, mismatch between postcard-rpc's stream/COBS model and BLE's fixed-MTU world, and ongoing dep-pin churn from postcard-rpc's transitive `embassy-usb-0_5-server` / `embassy-sync 0.7` requirements.

The `host_service` branch has the full postcard-rpc-based implementation (see `rmk/src/host/rmk_protocol/`, ICD in `rmk-types/src/protocol/rmk/`, host client in `rmk-host-tool/`). Handler bodies, transport scaffolding, and ICD struct shapes carry over unchanged. Only the wire layer, dispatch, and host client transport are rewritten.

---

## Wire format

### Header — fixed 5 bytes

```
┌─────────────────┬──────────┬──────────────────┐
│ CMD     u16 LE  │ SEQ  u8  │ LEN     u16 LE   │
└─────────────────┴──────────┴──────────────────┘
   2 bytes           1 byte      2 bytes
```

Followed by `LEN` bytes of `postcard`-encoded payload.

- **CMD** — command tag. `0x0000–0x7FFF` = request/response pair. `0x8000–0xFFFF` = topic / unsolicited notification. Top bit is the discriminator, so dispatch can split request vs. notification with a single mask.
- **SEQ** — opaque echo. Firmware copies the request's SEQ into the response. Topics always send `SEQ = 0`. 8 bits is enough — no in-flight count > 1 today, and the host correlates one-at-a-time per transport.
- **LEN** — `u16 LE`, payload byte count (excludes header). `LEN ≤ 4091` to keep total frame ≤ 4096; the largest current payload (full keymap bulk) fits comfortably.

**No varint, no schema hash, no COBS.** Header is `repr(C, packed)` and parsed with three `from_le_bytes` reads.

### Payload encoding

`postcard` (the encoding crate, **not** postcard-rpc) for every payload struct. Keeps serde derive, terseness, and zero-copy decoding. ICD struct definitions in `rmk-types/src/protocol/rmk/` are unchanged — only their `Endpoint`/`Topic` trait impls are removed.

### Versioning

- `GetVersion` returns `protocol_version: u16` — packed `(major: u8, minor: u8)`. This endpoint's payload shape **is permanent**; never modify.
- `GetCapabilities` returns `caps: u32` — bitmask of optional command groups (`BULK_TRANSFER`, `BLE_TOPICS`, `MORSE`, `FORK`, `COMBO`, `HOST_SECURITY`, …). Host gates calls on caps.
- Per-command struct evolution: when a struct's shape changes, bump the `protocol_version` minor and document the new field in the ICD comment. Adding fields to the **end** of a postcard-encoded struct is forward-compatible (older host stops reading early); reordering or removing fields requires a new `Cmd` variant and major bump.

This is XAP/Vial's model. Loses postcard-rpc's instant hash-mismatch detection; gained: full control over which changes cost a major bump.

### Framing per transport

**USB bulk** (vendor class 0xFF, 64B FS / 512B HS packets):

- Single logical frame = header + payload, sent across as many bulk packets as needed.
- Frame end = short packet (< max packet) or zero-length packet, per existing convention in `wire_usb.rs`.
- RX state machine: read into a 4096B buffer; once `LEN` bytes after the header are accumulated, dispatch.
- Same as today — minimal change.

**BLE GATT** (custom service, write + notify chars, 244B = MTU−3):

- Header always lands in the first packet (header is 5 B ≤ 244 B).
- Continuation packets carry pure payload bytes — no per-packet header, no COBS sentinel.
- Reassembler state: `{ remaining: u16, buf: [u8; 4096] }`. On each notify/write: append; if `remaining == 0`, dispatch.
- Boundary contract: host MUST NOT interleave frames on the write characteristic. Firmware MUST NOT interleave responses on the notify characteristic. Topics may be interleaved with responses **between** complete frames only — enforced by serializing notify writes through a single producer task.

This drops the `cobs` dep, the 512B COBS encode buffer, and the byte-stream parser — replaced by a counter.

---

## `Cmd` enum (canonical tag space)

Defined in `rmk-types/src/protocol/rmk/cmd.rs` (new file):

```rust
#[repr(u16)]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum Cmd {
    // System          0x00xx
    GetVersion         = 0x0001,
    GetCapabilities    = 0x0002,
    GetDeviceInfo      = 0x0003,
    Reset              = 0x0004,
    JumpToBootloader   = 0x0005,
    GetLockStatus      = 0x0006,   // v1: stub returns locked=false
    UnlockRequest      = 0x0007,   // v1: stub
    LockRequest        = 0x0008,   // v1: stub

    // Keymap          0x01xx
    GetLayerCount      = 0x0101,
    GetKeymapKey       = 0x0102,
    SetKeymapKey       = 0x0103,
    GetMatrixState     = 0x0104,
    GetKeymapBulk      = 0x0181,   // feature = "bulk_transfer"
    SetKeymapBulk      = 0x0182,   // feature = "bulk_transfer"

    // Encoder         0x02xx
    GetEncoderAction   = 0x0201,
    SetEncoderAction   = 0x0202,

    // Macro           0x03xx
    GetMacroData       = 0x0301,
    SetMacroData       = 0x0302,

    // Combo           0x04xx
    GetCombo           = 0x0401,
    SetCombo           = 0x0402,
    GetComboBulk       = 0x0481,   // feature = "bulk_transfer"
    SetComboBulk       = 0x0482,   // feature = "bulk_transfer"

    // Morse           0x05xx
    GetMorse           = 0x0501,
    SetMorse           = 0x0502,
    GetMorseBulk       = 0x0581,   // feature = "bulk_transfer"
    SetMorseBulk       = 0x0582,   // feature = "bulk_transfer"

    // Fork            0x06xx
    GetFork            = 0x0601,
    SetFork            = 0x0602,

    // Behavior        0x07xx
    GetBehaviorConfig  = 0x0701,

    // Connection      0x08xx
    GetConnectionInfo  = 0x0801,
    SetConnection      = 0x0802,

    // Status          0x09xx
    GetStatus          = 0x0901,
    GetStorageInfo     = 0x0902,

    // BLE             0x0Axx
    GetBleProfiles     = 0x0A01,
    SetBleProfile      = 0x0A02,
    ClearBleProfile    = 0x0A03,

    // ── Topics / notifications  (0x80xx) ──
    LayerChange        = 0x8001,
    WpmUpdate          = 0x8002,
    ConnectionChange   = 0x8003,
    SleepState         = 0x8004,
    LedIndicator       = 0x8005,
    BatteryStatus      = 0x8006,   // BLE only
    BleStatusChange    = 0x8007,   // BLE only
}

impl Cmd {
    pub fn try_from_u16(v: u16) -> Option<Self> { /* match all variants */ }
    pub fn is_topic(self) -> bool { (self as u16) & 0x8000 != 0 }
    pub fn is_bulk(self) -> bool  { (self as u16) & 0x0080 != 0 }
}
```

`#[non_exhaustive]` so the host can downgrade unknown CMDs to "unsupported" without panicking. Hex grouping (`0x0Nxx`) maps 1:1 to today's handler module split — 11 modules become 11 hex pages.

---

## Crate / module layout

### New files

| File | Purpose |
|---|---|
| `rmk-types/src/protocol/rmk/cmd.rs` | `Cmd` enum, `try_from_u16`, helper masks |
| `rmk-types/src/protocol/rmk/header.rs` | `Header { cmd: Cmd, seq: u8, len: u16 }`, `encode/decode` |
| `rmk/src/host/rmk_protocol/codec.rs` | Frame assembly: header + postcard payload → `[u8]` and back |
| `rmk/src/host/rmk_protocol/dispatch.rs` | The flat `match cmd` dispatcher |
| `rmk-host-tool/src/transport.rs` | New `Transport` trait + USB / BLE impls (replaces postcard-rpc `Client<>`) |

### Files modified

| File | Change |
|---|---|
| `rmk/Cargo.toml:44` | Remove `postcard-rpc`. Keep `postcard` (already transitive). |
| `rmk/Cargo.toml:124` | `rmk_protocol = ["host", "rmk-types/rmk_protocol"]` (drop `dep:postcard-rpc`, `dep:cobs`). |
| `rmk-types/Cargo.toml` | Remove `postcard-rpc` and `postcard-schema`. |
| `rmk-host-tool/Cargo.toml:17` | Remove `postcard-rpc`. Keep `nusb`, add `btleplug` (BLE host). |
| `rmk-types/src/protocol/rmk/endpoints.rs` | Delete `Endpoint` impls. Keep request/response struct definitions. |
| `rmk-types/src/protocol/rmk/topics.rs` | Delete `Topic` impls. Keep payload struct definitions. |
| `rmk/src/host/rmk_protocol/mod.rs` | Replace hand-rolled `Dispatch` impl with thin glue calling `dispatch::handle_frame`. |
| `rmk/src/host/rmk_protocol/wire_usb.rs` | Drop postcard-rpc `WireTx`/`WireRx` glue; emit/consume raw frames into the codec. |
| `rmk/src/host/rmk_protocol/wire_ble.rs` | Replace COBS stream parser with length-prefixed reassembler. |
| `rmk/src/host/rmk_protocol/topics.rs` | Re-tag publishers with `Cmd` enum; same channel topology. |
| `rmk/src/host/rmk_protocol/service.rs` | No structural change; still spawns USB + BLE tasks sharing `&KeyMap`. |
| `plan.md` | Update wire-model section + remove postcard-rpc references. |

### Files unchanged

`rmk/src/host/rmk_protocol/handlers/{system,keymap,encoder,macro_data,combo,morse,fork,behavior,connection,status,ble}.rs` — handler signatures stay `async fn(ctx, request) -> response`.

---

## Dispatch architecture

```rust
// rmk/src/host/rmk_protocol/dispatch.rs

pub async fn handle_frame<T: WireTx>(
    ctx: &Ctx<'_>,
    tx: &mut T,
    frame: &[u8],
) -> Result<(), Error> {
    let (header, payload) = Header::decode(frame)?;
    if header.cmd.is_topic() { return Err(Error::TopicFromHost); }

    let resp = match header.cmd {
        Cmd::GetVersion        => system::get_version(ctx, payload).await,
        Cmd::GetCapabilities   => system::get_capabilities(ctx, payload).await,
        Cmd::GetKeymapKey      => keymap::get_key(ctx, payload).await,
        Cmd::SetKeymapKey      => keymap::set_key(ctx, payload).await,
        #[cfg(feature = "bulk_transfer")]
        Cmd::GetKeymapBulk     => keymap::get_bulk(ctx, payload).await,
        // ... 28 arms total
        _ => Err(Error::UnsupportedCmd(header.cmd as u16)),
    };

    let payload = encode_response(resp)?;
    tx.send(Header { cmd: header.cmd, seq: header.seq, len: payload.len() as u16 }, &payload).await
}
```

Each `handlers::foo` returns `Result<impl Serialize, AppError>`. `encode_response` wraps the `AppError` arm in a `Cmd::Error`-tagged response (or piggybacks on the same CMD with a `result: Result<R, ErrCode>` envelope — TBD during implementation, prefer envelope).

`WireTx`, `WireRx` — minimal new traits in `codec.rs`:

```rust
pub trait WireTx {
    async fn send(&mut self, header: Header, payload: &[u8]) -> Result<(), TxErr>;
}
pub trait WireRx {
    async fn recv<'a>(&mut self, buf: &'a mut [u8]) -> Result<&'a [u8], RxErr>;
}
```

USB and BLE wires implement both. No generics leak past the dispatcher.

---

## Topic publisher

```rust
// rmk/src/host/rmk_protocol/topics.rs (re-shaped)

pub async fn publish<T: Serialize, W: WireTx>(
    tx: &mut W,
    cmd: Cmd,           // 0x80xx range
    msg: &T,
) -> Result<(), TxErr> {
    let payload = postcard::to_slice(msg, &mut SCRATCH)?;
    tx.send(Header { cmd, seq: 0, len: payload.len() as u16 }, payload).await
}
```

The 7 topic publisher tasks keep their existing channel subscriptions; only the encode call changes. BLE-only topics stay gated by `cfg(feature = "_ble")`.

Multi-producer serialization on the BLE notify char: a single dedicated task owns the notify characteristic; topic publishers and the dispatcher both push completed frames into a `heapless::mpmc::Q4<Frame>` (replaces today's `BLE_REPLY_CHANNEL`).

---

## Host tool (`rmk-host-tool`)

Replace `postcard_rpc::host_client::HostClient` with a `Transport` trait:

```rust
// rmk-host-tool/src/transport.rs
pub trait Transport {
    async fn request<Req: Serialize, Resp: DeserializeOwned>(
        &mut self,
        cmd: Cmd,
        req: &Req,
    ) -> Result<Resp, TransportError>;

    fn subscribe(&mut self) -> impl Stream<Item = (Cmd, Vec<u8>)>;
}
```

- `UsbBulkTransport` — wraps `nusb::Interface`. Reads/writes vendor bulk endpoints. Frame boundary = short packet / ZLP. Uses one background task to demux responses (matched by SEQ) from topics (CMD high bit set).
- `BleGattTransport` — wraps `btleplug::Peripheral`. Writes to write char, subscribes to notify char, reassembles by `Header.len`.

A small SEQ allocator (rolling u8) plus a `HashMap<u8, oneshot::Sender<Vec<u8>>>` correlates responses. Topics fan out to a `tokio::sync::broadcast`.

This is ~300–400 LOC of host code, replacing ~600 LOC of postcard-rpc client glue.

---

## Migration sequence (mergeable to `main` at every step)

Each step ends with `sh scripts/test_all.sh` green and the example builds working.

**Step 1 — Add `Cmd` + codec alongside postcard-rpc.** New files only; nothing wired yet. Round-trip unit tests for header encode/decode on host (`cargo test -p rmk-types`).

**Step 2 — New dispatch path behind cfg.** Introduce `dispatch.rs` calling existing handler bodies. Gate with `#[cfg(rmk_protocol_v2)]` (cfg, not feature, since the goal is full replacement). Both paths compile.

**Step 3 — Rewrite USB wire.** Replace `wire_usb.rs` postcard-rpc glue with raw `WireTx`/`WireRx` impls. Verify locally with `examples/use_rust/<usb-board>` that `GetVersion` round-trips via a quick host probe.

**Step 4 — Rewrite BLE wire.** Replace COBS parser with reassembler. Verify with `examples/use_rust/nrf52840_ble` and JLink (per `feedback_jlink_workflow`).

**Step 5 — Rewrite host transport.** Replace postcard-rpc `Client<>` in `rmk-host-tool`. Round-trip all 28 CMDs against a flashed board (USB and BLE). Capture before/after binary size with `cargo bloat --release --features rmk_protocol`.

**Step 6 — Delete postcard-rpc.** Remove deps from all three Cargo.toml files. Remove the cfg gate. Remove old dispatch impl. `cargo update` and verify lockfile shrinks.

**Step 7 — Docs.** Update `plan.md` wire-model section; add a `docs/rmk_protocol_wire.md` with the Header diagram and CMD table.

---

## Risks and mitigations

1. **Schema drift in the field** (no hash mismatch detection).
   - Every response struct gets `pub version: u8` as the leading field where future evolution is plausible (keymap/combo/morse/encoder/fork). Trivial structs (`u32 layer_count`) can skip it.
   - CI snapshot test (Step 7): hash all ICD type definitions; if the hash changes without a `protocol_version` minor bump in `cmd.rs`, fail the build.
2. **Forgotten `Cmd` arm in `match`.** `#[deny(non_exhaustive_omitted_patterns)]` on the dispatcher — but `#[non_exhaustive]` on `Cmd` is for *external* exhaustiveness; internally we get full exhaustiveness checks since the dispatcher lives in the same crate as the consuming code is fine — the `_ => Error::UnsupportedCmd` catch-all is for forward compat with newer hosts.
3. **BLE frame interleaving bug.** The single-producer notify task is the contract. Add a `debug_assert!` that no second frame starts before the previous one's `LEN` bytes have been pushed to `Notify`.
4. **Bulk endpoint > 4096 B.** Today's largest payload is the full keymap bulk; verify it fits within the 4096B limit before committing to `u16` LEN. If it doesn't, expand `LEN` to `u32` (header becomes 7 B).
5. **Lost cycle to ship `host_service`.** Branch is otherwise mergeable. The migration sequence above keeps each step shippable, so partial progress is not blocked.

---

## Verification

- **Unit (host, std):** `cargo test -p rmk-types` for header round-trip, `Cmd::try_from_u16` exhaustive coverage, postcard payload round-trip on every ICD struct.
- **Unit (firmware, no_std):** `cargo nextest run -p rmk --no-default-features --features=rmk_protocol,storage,_ble,async_matrix` for codec + reassembler.
- **End-to-end USB:** flash `examples/use_rust/<usb-board>`; run `rmk-host-tool` to walk all 28 CMDs; confirm responses match expectations.
- **End-to-end BLE:** flash `examples/use_rust/nrf52840_ble`; use JLinkRTTLogger for firmware logs (per memory); same CMD walk via `btleplug` host transport.
- **Binary size:** `cargo size --release` on the same example before/after the migration. Target: ≥5 KB flash reduction. Record actual delta in the final PR description.
- **Vial coexistence:** confirm the existing `vial` feature still builds and runs (mutual exclusion via Cargo features must remain).
- **Format:** `sh scripts/format_all.sh` and `sh scripts/test_all.sh` clean before each PR.

---

## Critical files (single-glance index)

- `rmk-types/src/protocol/rmk/cmd.rs` (new) — `Cmd` enum.
- `rmk-types/src/protocol/rmk/header.rs` (new) — `Header` + encode/decode.
- `rmk-types/src/protocol/rmk/{endpoints,topics}.rs` — strip RPC trait impls.
- `rmk/src/host/rmk_protocol/codec.rs` (new) — `WireTx`/`WireRx` traits + frame assembly.
- `rmk/src/host/rmk_protocol/dispatch.rs` (new) — flat `match cmd`.
- `rmk/src/host/rmk_protocol/{wire_usb,wire_ble,topics,service,mod}.rs` — rewire to new codec.
- `rmk-host-tool/src/transport.rs` (new) — `Transport` trait + USB/BLE impls.
- `rmk/Cargo.toml`, `rmk-types/Cargo.toml`, `rmk-host-tool/Cargo.toml` — drop `postcard-rpc`, `postcard-schema`, `cobs`.
- `plan.md` — update wire-model section; add `docs/rmk_protocol_wire.md`.

---

## Why this design (recap of prior research)

- **Wire format chosen** to match BLE's fixed-MTU reality (length-prefixed, not COBS) and avoid postcard-rpc's variable 3–13 B header overhead. Header is exactly 5 B always; `from_le_bytes` decode.
- **Dispatch chosen** as a flat `match` on a `repr(u16)` enum because there are only 28 commands; FNV1a hash keys are overkill at this scale and add 6+ bytes/message and macro-generated code per endpoint.
- **`postcard` retained** for payloads — small, stable, single-purpose, no transitive `embassy-usb-0_5-server` pin.
- **Versioning chosen** XAP/Vial-style (single `protocol_version` + `GetCapabilities` bitmask + structural append-only rule) because peer firmwares (ZMK Studio's protobuf, QMK XAP, Vial) all use this model and none use a Rust-specific RPC framework.
- **No upstream alternative** is imminent — postcard-rpc issue #26 (BLE / non-USB transports) has been open since June 2024 with no roadmap, so `feedback_prefer_upstream_canonical`'s "imminent canonical" trigger does not apply.
