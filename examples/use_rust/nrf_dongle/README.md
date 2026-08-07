# Tri-mode dongle, end to end

The complete setup from the tri-mode dongle design
(`docs/docs/main/docs/development/dongle_mode_design.md`): a split BLE keyboard
whose central is an ordinary `rynk` BLE keyboard, relayed to the host by a USB
dongle. Three firmwares, one per board:

| Crate | Board | Role |
| --- | --- | --- |
| [`dongle/`](dongle/) | nRF54LM20A | USB dongle: relays bonded keyboards (HID + Rynk CDC) |
| [`central/`](central/) | nRF52833 | Split central: the keyboard itself (`rynk` + `split`) |
| [`peripheral/`](peripheral/) | nRF54L15 | Split peripheral: the other half |

Each crate builds independently (`cargo build --release` inside it) and flashes
via J-Link (`./run.sh <elf>`).

## Bring-up order

1. Flash all three. Both keyboard halves use their DK buttons as a 2x2 matrix;
   the central's Button 4 is the dongle key (`User8`).
2. The split halves find each other on their own (central scans, peripheral
   advertises).
3. Press the dongle key on the central to switch to the dongle slot, then
   power-cycle (reset) the dongle: its 30s pairing window picks up the seeking
   central and pairs. From then on everything reconnects automatically.
4. Typing from either half arrives on the host through the dongle; a Rynk host
   tool on the dongle's CDC port reaches the central's keymap transparently.

Holding the dongle key for 5s clears the central's dongle bond (or, while the
dongle link is up, authorizes pairing a second keyboard).
