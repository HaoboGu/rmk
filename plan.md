# Rynk bulk throughput plan

Context (2026-07-25): host-side concurrency landed (54980b722: 4-slot pool, SEQ-routed replies; 6828e49da: firmware serves every coalesced frame, parking the tail). Bulk transfers still page sequentially, so a 960-key keymap read costs 21 round trips: ~60 ms over USB (~80% spent in per-page turnaround, not data) and ~0.63 s over BLE (2 conn intervals per page). Reference points from the 2026-07 protocol survey: postcard-rpc ~5–10 ms (pipelined paging), ZMK Studio USB ~15 ms (single streamed response).

## Stage 0 — prerequisite: client timeout + disconnect wakeup

- Client-level default timeout with per-request override. `SlotGuard` already frees a slot on drop, so timing out a request is just dropping its future.
- Driver exit must wake every slot waiter with a disconnect error; today `slot.resp.wait()` has no disconnect signal and relies on session supervision.

Not throughput work, but every later stage assumes a request cannot hang forever. No other keyboard protocol ships this; combined with the firmware's always-reply guarantee it closes the last hang path.

## Stage 1 — pipeline bulk paging (host-only)

`read_all_*` / `write_all_*` issue pages through the slot pool, keeping up to `MAX_IN_FLIGHT` requests in flight instead of awaiting each page.

Reads — conservative offset spacing:

- Pipelined requests coalesce at the firmware; serving one parks the rest. Each parked request is small (~10–12 B), but it shrinks the reply window, so pages can come back short.
- Space speculative offsets by the page size at the worst-case window: `spacing = bulk_size_for(RYNK_BUFFER_SIZE − MAX_IN_FLIGHT × parked_request_size)`. Every returned page is then ≥ spacing, so a short page produces a small overlap (idempotent re-read), never a gap. No repair pass needed.

Writes: each page nearly fills the firmware buffer, so later pages queue in the transport (USB NAK / BLE pipe) and their turnaround overlaps firmware processing. Nothing to change beyond issuing them through the pool.

Expected (960-key keymap): USB ~60 ms → ~15 ms; BLE ~0.63 s → ~0.2 s. No protocol change, no firmware change.

Also: document `RYNK_BUFFER_SIZE` as a throughput knob. 488 is the BLE-optimal default (2 × 244 B notify chunks); a USB-only board at 2048 gets ~200 keys/page → 5 pages ≈ 16 ms even without pipelining. Cost is per-session RAM.

## Stage 2 — bounded multi-frame responses (later, if needed)

One request → firmware pushes N frames, each ≤ `RYNK_BUFFER_SIZE`, same cmd+seq, terminated by an empty page (the shipped EOF semantics). Collapses a bulk read to one round trip: USB ~10 ms, BLE ~0.15 s.

Keeps every invariant Stage 1 keeps:

- Frames fit the buffer: `MAX_ENDPOINT_PAYLOAD` fold and const asserts stay meaningful; no-alloc hosts (dongle, display) can still receive everything.
- Loss and resync stay per-frame: a dropped chunk costs one frame, COBS resyncs at the next delimiter.
- An error frame can interpose mid-burst; concurrent small requests interleave between frames.

To settle before starting:

- Host routing: the per-slot `Signal` is latest-wins and would drop earlier frames of a burst; it becomes a small bounded channel. Decide how driver-RX backpressure from a slow consumer interacts with other slots and the topic queue.
- Firmware pacing: `write_all` backpressure is the only pacing; confirm head-of-line blocking during a burst is acceptable or bound the burst length.

Trigger: genuinely unbounded transfers (log streaming, full backup), or if the remaining ~1.5× over Stage 1 matters.

## Rejected: ZMK-style unbounded single response

ZMK streams one arbitrarily large response (nanopb callback encode → 64 B ring → SOF/ESC/EOF framing). That shape only works as a package with its other choices — streamed encode, no length field, and per-chunk confirmed GATT indications covering the fragile failure domain (the same choice that caps its BLE at 27 B per conn interval). Adopting the frame shape without the crutches imports the failure modes:

- Breaks the frame ≤ buffer invariants: the compile-time payload fold, the symmetric `max_payload_size` contract, and no-alloc host RX (a fixed 488 B host buffer can never receive one).
- All-or-nothing failure domain: any lost chunk (WebHID report-queue overflow is the realistic case) discards the whole response and restarts from zero; paging retries one page.
- Head-of-line blocking: one long burst starves the other three slots, undoing the slot-pool concurrency.
- No mid-stream retraction: an error discovered after partial transmit needs a frame-poisoning convention; ZMK's own encode-failure path leaves a half-written frame and a held mutex.
- postcard's length-prefixed sequences want a known count per frame; protobuf's self-delimiting repeated fields are what make ZMK's open-ended shape natural.
- Field evidence of the fragility: zmk#3215 (TX-ring-full skips a framing byte, wedging the stream), zmk#3185 (decode overflow under exactly the pipelining its client never does).
