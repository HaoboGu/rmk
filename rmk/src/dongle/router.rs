//! The dongle's CDC session: self-answers the `0x09xx` dongle command segment
//! and passes every other frame through to the target keyboard as raw bytes —
//! no re-encoding in either direction (design D4).

use core::sync::atomic::{AtomicBool, Ordering};

use embassy_futures::select::{Either3, select3};
use embassy_sync::channel::Channel;
use embassy_sync::pipe::Pipe;
use embedded_io_async::{Read, Write};
use rmk_types::constants::RYNK_BUFFER_SIZE;
use rmk_types::protocol::rynk::{
    Cmd, Deframer, RYNK_HEADER_SIZE, RynkError, RynkHeader, TopicEvent, decode_header, encode_frame,
};

use super::LinkState;
use crate::{DONGLE_LINKS_NUM, DONGLE_SLOTS_NUM, RawMutex};

/// Whole encoded frames (delimiter included) from the router to a link's
/// `output_data` writes.
pub(crate) type RouterFrame = heapless::Vec<u8, RYNK_BUFFER_SIZE>;
pub(crate) static ROUTER_TX: [Channel<RawMutex, RouterFrame, 1>; DONGLE_LINKS_NUM] =
    [const { Channel::new() }; DONGLE_LINKS_NUM];

/// Raw keyboard→host bytes from the target link's `input_data` notifies. Sized
/// for a few MTUs of slack so a briefly stalled host doesn't drop bytes.
pub(crate) static ROUTER_RX: Pipe<RawMutex, 1024> = Pipe::new();

static SESSION_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Called by a link task for every `input_data` notify: forward the raw bytes
/// when a session is active and this slot is the config target, drop otherwise.
/// Never blocks — the typing path shares the link's notification queue.
pub(crate) fn forward_to_host(slot: u8, data: &[u8]) {
    if !SESSION_ACTIVE.load(Ordering::Relaxed) {
        return;
    }
    if super::read_slots(|t| t.config_target()) != Some(slot) {
        return;
    }
    match ROUTER_RX.try_write(data) {
        Ok(n) if n == data.len() => {}
        // A partial write corrupts the frame; the host's deframer resyncs at
        // the next delimiter and the exchange fails visibly instead of hanging.
        _ => warn!("[dongle] host config stream overflow, dropping bytes"),
    }
}

/// The USB host service (`HostService` alias under the `dongle` feature);
/// [`crate::usb::rynk::run_host_usb`] drives one session per CDC connection.
pub struct DongleRouter;

impl DongleRouter {
    pub async fn run_session<R: Read, T: Write>(&self, rx: &mut R, tx: &mut T) {
        ROUTER_RX.clear();
        for ch in &ROUTER_TX {
            ch.clear();
        }
        super::SLOTS_CHANGED.reset();
        SESSION_ACTIVE.store(true, Ordering::Relaxed);
        self.session(rx, tx).await;
        SESSION_ACTIVE.store(false, Ordering::Relaxed);
        // The next session re-resolves its target from scratch.
        super::update_slots_quiet(|t| t.explicit_target = None);
    }

    async fn session<R: Read, T: Write>(&self, rx: &mut R, tx: &mut T) {
        // Host→dongle raw frames; sized like the keyboard's own session buffer,
        // so anything a keyboard could parse fits here too.
        let mut host_buf = [0u8; RYNK_BUFFER_SIZE];
        let mut host_len = 0;
        let mut host_discard = false;
        // Keyboard→host bytes, reassembled to frame boundaries so a dongle
        // self-answer never lands inside a split keyboard frame.
        let mut kb_buf = [0u8; RYNK_BUFFER_SIZE];
        let mut kb_len = 0;
        // Topics stay muted until the host has read the slot table: only then
        // does a `DongleSlotsChange` push have a baseline to be a delta of.
        let mut probed = false;

        loop {
            match select3(
                rx.read(&mut host_buf[host_len..]),
                ROUTER_RX.read(&mut kb_buf[kb_len..]),
                super::SLOTS_CHANGED.wait(),
            )
            .await
            {
                Either3::First(Ok(0)) | Either3::First(Err(_)) => return,
                Either3::First(Ok(n)) => {
                    host_len += n;
                    let mut start = 0;
                    while let Some(pos) = host_buf[start..host_len].iter().position(|&b| b == 0) {
                        let end = start + pos + 1;
                        if host_discard {
                            host_discard = false; // the oversized frame's delimiter: resync
                        } else if end - start > 1
                            && !self.handle_host_frame(&host_buf[start..end], tx, &mut probed).await
                        {
                            return;
                        }
                        start = end;
                    }
                    host_buf.copy_within(start..host_len, 0);
                    host_len -= start;
                    if host_len == host_buf.len() {
                        // No delimiter in a full buffer: drop and drain to the next one.
                        warn!("[dongle] oversized host frame dropped");
                        host_len = 0;
                        host_discard = true;
                    }
                }
                Either3::Second(n) => {
                    kb_len += n;
                    let mut start = 0;
                    while let Some(pos) = kb_buf[start..kb_len].iter().position(|&b| b == 0) {
                        let end = start + pos + 1;
                        if end - start > 1 && tx.write_all(&kb_buf[start..end]).await.is_err() {
                            return;
                        }
                        start = end;
                    }
                    kb_buf.copy_within(start..kb_len, 0);
                    kb_len -= start;
                    if kb_len == kb_buf.len() {
                        warn!("[dongle] oversized keyboard frame dropped");
                        kb_len = 0;
                    }
                }
                Either3::Third(()) => {
                    // An explicit target whose slot lost its bond or link is
                    // dead: drop it rather than silently retargeting (§2.5).
                    super::update_slots_quiet(|t| {
                        if let Some(s) = t.explicit_target
                            && !matches!(t.slots[s as usize].link, LinkState::Connected(_))
                        {
                            t.explicit_target = None;
                        }
                    });
                    if probed {
                        let mut buf = [0u8; RYNK_BUFFER_SIZE];
                        match TopicEvent::DongleSlotsChange(super::slots_snapshot()).encode(&mut buf) {
                            Ok(n) => {
                                if tx.write_all(&buf[..n]).await.is_err() {
                                    return;
                                }
                            }
                            Err(e) => warn!("[dongle] topic encode failed: {:?}", e),
                        }
                    }
                }
            }
        }
    }

    /// Route one whole encoded frame (delimiter included). Returns `false`
    /// when the transport died.
    async fn handle_host_frame<T: Write>(&self, frame: &[u8], tx: &mut T, probed: &mut bool) -> bool {
        let Some(header) = decode_header(&frame[..frame.len() - 1]) else {
            warn!("[dongle] undecodable host frame dropped");
            return true;
        };
        if header.cmd.is_topic() {
            warn!("[dongle] dropping topic-range request {:?}", header.cmd);
            return true;
        }
        if header.cmd.raw() & 0xFF00 == 0x0900 {
            return self.answer_dongle_cmd(header, frame, tx, probed).await;
        }

        // Pass-through: raw bytes to the target link, byte-for-byte.
        let target = super::read_slots(|t| {
            let slot = t.config_target()?;
            let link = match t.slots[slot as usize].link {
                LinkState::Connected(link) => Some(link),
                _ => None,
            };
            Some((slot, link))
        });
        match target {
            None => write_reply(tx, header, &Err::<(), RynkError>(RynkError::NoTarget)).await,
            Some((_, None)) => write_reply(tx, header, &Err::<(), RynkError>(RynkError::NotReady)).await,
            Some((_, Some(link))) => {
                let Ok(copy) = RouterFrame::from_slice(frame) else {
                    return write_reply(tx, header, &Err::<(), RynkError>(RynkError::Malformed)).await;
                };
                ROUTER_TX[link as usize].send(copy).await;
                true
            }
        }
    }

    /// The dongle's own command segment; never forwarded.
    async fn answer_dongle_cmd<T: Write>(
        &self,
        header: RynkHeader,
        frame: &[u8],
        tx: &mut T,
        probed: &mut bool,
    ) -> bool {
        // 0x09xx requests are tiny; decode via the shared deframer.
        let mut buf = [0u8; 96];
        let payload_len = if frame.len() <= buf.len() {
            buf[..frame.len()].copy_from_slice(frame);
            let mut df = Deframer::new();
            df.commit(frame.len());
            df.next(&mut buf)
        } else {
            None
        };
        let Some(len) = payload_len else {
            return write_reply(tx, header, &Err::<(), RynkError>(RynkError::Malformed)).await;
        };
        let payload = &buf[RYNK_HEADER_SIZE..len];

        match header.cmd {
            Cmd::GetDongleSlots => {
                *probed = true;
                write_reply(tx, header, &Ok::<_, RynkError>(super::slots_snapshot())).await
            }
            Cmd::SelectDongleTarget => {
                let reply = match postcard::from_bytes::<u8>(payload) {
                    Ok(slot) if (slot as usize) < DONGLE_SLOTS_NUM => super::update_slots_quiet(|t| {
                        if t.slots[slot as usize].bond.is_some() {
                            t.explicit_target = Some(slot);
                            Ok(())
                        } else {
                            Err(RynkError::Invalid)
                        }
                    }),
                    Ok(_) => Err(RynkError::Invalid),
                    Err(_) => Err(RynkError::Malformed),
                };
                write_reply(tx, header, &reply).await
            }
            Cmd::ForgetDongleSlot => {
                let reply = match postcard::from_bytes::<u8>(payload) {
                    Ok(slot) if (slot as usize) < DONGLE_SLOTS_NUM => {
                        let identity = super::update_slots(|t| {
                            let s = &mut t.slots[slot as usize];
                            s.name = heapless::String::new();
                            if t.explicit_target == Some(slot) {
                                t.explicit_target = None;
                            }
                            t.slots[slot as usize].bond.take().map(|b| b.identity)
                        });
                        if let Some(identity) = identity {
                            let _ = super::REMOVED_BONDS.try_send(identity);
                            crate::channel::FLASH_CHANNEL
                                .send(crate::storage::FlashOperationMessage::ClearSlot(slot))
                                .await;
                            Ok(())
                        } else {
                            Err(RynkError::Invalid)
                        }
                    }
                    Ok(_) => Err(RynkError::Invalid),
                    Err(_) => Err(RynkError::Malformed),
                };
                write_reply(tx, header, &reply).await
            }
            _ => write_reply(tx, header, &Err::<(), RynkError>(RynkError::UnknownCmd)).await,
        }
    }
}

/// Encode and send one reply frame; `false` when the transport died.
async fn write_reply<T: Write>(tx: &mut T, header: RynkHeader, value: &impl serde::Serialize) -> bool {
    let mut buf = [0u8; RYNK_BUFFER_SIZE];
    match encode_frame(&mut buf, header, value) {
        Ok(n) => tx.write_all(&buf[..n]).await.is_ok(),
        Err(e) => {
            warn!("[dongle] reply encode failed: {:?}", e);
            true
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use alloc::collections::VecDeque;
    use alloc::vec::Vec;

    use embassy_futures::yield_now;
    use embedded_io_async::{ErrorKind, ErrorType};
    use rmk_types::protocol::rynk::DongleSlots;
    use trouble_host::prelude::*;
    use trouble_host::{BondInformation, LongTermKey};

    use super::super::{LinkState, Slot};
    use super::*;
    use crate::test_support::test_block_on as block_on;

    /// Returns each chunk as one `read`; once drained, yields `idle_reads`
    /// times (so the session's other select arms get to run) and then EOF.
    struct ChunkRead {
        chunks: VecDeque<Vec<u8>>,
        idle_reads: usize,
    }

    impl ErrorType for ChunkRead {
        type Error = ErrorKind;
    }

    impl Read for ChunkRead {
        async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
            loop {
                let Some(chunk) = self.chunks.front_mut() else {
                    if self.idle_reads == 0 {
                        return Ok(0);
                    }
                    self.idle_reads -= 1;
                    yield_now().await;
                    continue;
                };
                let n = chunk.len().min(buf.len());
                buf[..n].copy_from_slice(&chunk[..n]);
                chunk.drain(..n);
                if chunk.is_empty() {
                    self.chunks.pop_front();
                }
                return Ok(n);
            }
        }
    }

    struct VecWrite {
        captured: Vec<u8>,
    }

    impl ErrorType for VecWrite {
        type Error = ErrorKind;
    }

    impl Write for VecWrite {
        async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
            self.captured.extend_from_slice(buf);
            Ok(buf.len())
        }

        async fn flush(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    fn frame(cmd: Cmd, seq: u8, payload: &impl serde::Serialize) -> Vec<u8> {
        let mut buf = [0u8; 128];
        let n = encode_frame(&mut buf, RynkHeader { cmd, seq }, payload).unwrap();
        buf[..n].to_vec()
    }

    /// Decode a captured reply stream into `(cmd, seq, payload)` frames.
    fn decode_frames(bytes: &[u8]) -> Vec<(u16, u8, Vec<u8>)> {
        let mut work = bytes.to_vec();
        work.resize(work.len().max(8), 0);
        let mut df = Deframer::new();
        df.commit(bytes.len());
        let mut out = Vec::new();
        while let Some(n) = df.next(&mut work) {
            out.push((
                u16::from_le_bytes([work[0], work[1]]),
                work[2],
                work[RYNK_HEADER_SIZE..n].to_vec(),
            ));
        }
        out
    }

    fn bond(n: u8) -> BondInformation {
        BondInformation::new(
            Identity {
                addr: Address::random([n; 6]),
                irk: None,
            },
            LongTermKey(n as u128),
            SecurityLevel::NoEncryption,
            false,
        )
    }

    fn set_slot(idx: usize, link: LinkState) {
        super::super::update_slots_quiet(|t| {
            t.slots[idx] = Slot {
                bond: Some(bond(idx as u8 + 1)),
                name: heapless::String::try_from("kb").unwrap(),
                last_seen: idx as u32,
                link,
            };
        });
    }

    fn run(chunks: VecDeque<Vec<u8>>, idle_reads: usize) -> Vec<u8> {
        let mut rx = ChunkRead { chunks, idle_reads };
        let mut tx = VecWrite { captured: Vec::new() };
        block_on(DongleRouter.run_session(&mut rx, &mut tx));
        tx.captured
    }

    #[test]
    fn answers_get_dongle_slots_densely() {
        set_slot(1, LinkState::Connected(0));
        let mut chunks = VecDeque::new();
        chunks.push_back(frame(Cmd::GetDongleSlots, 1, &()));
        let resp = decode_frames(&run(chunks, 0));

        assert_eq!(resp.len(), 1);
        assert_eq!(resp[0].0, Cmd::GetDongleSlots.raw());
        let slots = postcard::from_bytes::<Result<DongleSlots, RynkError>>(&resp[0].2)
            .unwrap()
            .unwrap();
        // Dense: every slot has an entry, so the index addresses it directly.
        assert_eq!(slots.slots.len(), crate::DONGLE_SLOTS_NUM);
        assert!(slots.slots[0].is_none(), "unbonded slots keep their place");
        assert!(slots.slots[1].as_ref().unwrap().connected);
        assert_eq!(slots.target, Some(1), "the only bonded slot is the implicit target");
    }

    #[test]
    fn single_slot_forwards_raw_bytes_to_its_link() {
        set_slot(0, LinkState::Connected(1));
        let request = frame(Cmd::GetVersion, 7, &());
        let mut chunks = VecDeque::new();
        chunks.push_back(request.clone());
        let captured = run(chunks, 0);

        assert!(captured.is_empty(), "forwarded frames get no local reply");
        let forwarded = ROUTER_TX[1].try_receive().expect("frame routed to link 1");
        assert_eq!(&forwarded[..], &request[..], "byte-for-byte pass-through");
    }

    #[test]
    fn multiple_slots_require_an_explicit_target() {
        set_slot(0, LinkState::Connected(0));
        set_slot(1, LinkState::Connected(1));
        let mut chunks = VecDeque::new();
        chunks.push_back(frame(Cmd::GetVersion, 3, &()));
        chunks.push_back(frame(Cmd::SelectDongleTarget, 4, &1u8));
        chunks.push_back(frame(Cmd::GetVersion, 5, &()));
        let resp = decode_frames(&run(chunks, 0));

        assert_eq!(resp.len(), 2, "one NoTarget error + one SelectDongleTarget reply");
        assert_eq!(resp[0].1, 3, "seq echo on the refused frame");
        assert_eq!(
            postcard::from_bytes::<Result<(), RynkError>>(&resp[0].2).unwrap(),
            Err(RynkError::NoTarget),
        );
        assert_eq!(
            postcard::from_bytes::<Result<(), RynkError>>(&resp[1].2).unwrap(),
            Ok(()),
        );
        let forwarded = ROUTER_TX[1].try_receive().expect("post-select frame routed");
        let header = decode_header(&forwarded[..forwarded.len() - 1]).unwrap();
        assert_eq!(header.seq, 5, "the selected target got the seq-5 frame");
    }

    #[test]
    fn disconnected_target_answers_not_ready() {
        set_slot(0, LinkState::Free);
        let mut chunks = VecDeque::new();
        chunks.push_back(frame(Cmd::GetVersion, 9, &()));
        let resp = decode_frames(&run(chunks, 0));

        assert_eq!(resp.len(), 1);
        assert_eq!(resp[0].1, 9);
        assert_eq!(
            postcard::from_bytes::<Result<(), RynkError>>(&resp[0].2).unwrap(),
            Err(RynkError::NotReady),
        );
    }

    #[test]
    fn unknown_dongle_cmd_is_answered_not_forwarded() {
        set_slot(0, LinkState::Connected(0));
        let mut chunks = VecDeque::new();
        chunks.push_back(frame(Cmd::from_raw(0x09FF), 2, &()));
        let resp = decode_frames(&run(chunks, 0));

        assert_eq!(resp.len(), 1);
        assert_eq!(
            postcard::from_bytes::<Result<(), RynkError>>(&resp[0].2).unwrap(),
            Err(RynkError::UnknownCmd),
        );
        assert!(ROUTER_TX[0].try_receive().is_err(), "0x09xx never reaches a keyboard");
    }

    #[test]
    fn forget_slot_clears_the_bond() {
        set_slot(0, LinkState::Free);
        // The forget path persists via FLASH_CHANNEL; drain it so send() never blocks.
        let drain = crate::channel::drain_flash_channel_for_test();
        let session = async {
            let mut rx = ChunkRead {
                chunks: VecDeque::from([frame(Cmd::ForgetDongleSlot, 1, &0u8)]),
                idle_reads: 0,
            };
            let mut tx = VecWrite { captured: Vec::new() };
            DongleRouter.run_session(&mut rx, &mut tx).await;
            let resp = decode_frames(&tx.captured);
            assert_eq!(
                postcard::from_bytes::<Result<(), RynkError>>(&resp[0].2).unwrap(),
                Ok(()),
            );
        };
        block_on(embassy_futures::select::select(session, drain));
        assert!(super::super::read_slots(|t| t.slots[0].bond.is_none()));
    }

    #[test]
    fn keyboard_frames_reassemble_to_boundaries() {
        set_slot(0, LinkState::Connected(0));
        // A keyboard reply split across two notify-sized writes.
        let reply = frame(Cmd::GetVersion, 6, &[1u8, 2, 3]);
        let split = reply.len() / 2;

        let session = async {
            let mut rx = ChunkRead {
                chunks: VecDeque::new(),
                idle_reads: 8,
            };
            let mut tx = VecWrite { captured: Vec::new() };
            DongleRouter.run_session(&mut rx, &mut tx).await;
            tx.captured
        };
        let feeder = async {
            // Both halves arrive after the session is up (it clears the pipe on
            // entry), with a yield between them so the first is read alone.
            yield_now().await;
            assert_eq!(ROUTER_RX.try_write(&reply[..split]).unwrap(), split);
            yield_now().await;
            assert_eq!(ROUTER_RX.try_write(&reply[split..]).unwrap(), reply.len() - split);
        };
        let (captured, ()) = block_on(embassy_futures::join::join(session, feeder));

        let resp = decode_frames(&captured);
        assert_eq!(resp.len(), 1, "one whole frame, no partial writes");
        assert_eq!(resp[0].1, 6);
    }
}
