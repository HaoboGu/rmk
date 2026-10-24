//! Every advertisement RMK sends, in one place.

use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_time::{Duration, Instant, with_timeout};
use trouble_host::prelude::appearance::human_interface_device::KEYBOARD;
use trouble_host::prelude::service::{BATTERY, HUMAN_INTERFACE_DEVICE};
use trouble_host::prelude::*;

#[cfg(feature = "_ext_adv")]
use crate::ble::ExtAdvCmds;
use crate::ble::HOST_LINKS;
use crate::ble::profile::{ProfileInfo, profile_address};

/// Company identifier marking an advertisement as RMK's own.
const RMK_ADV_COMPANY_ID: u16 = 0x5253;

// First payload byte of an RMK advertisement, naming the kind. This is wire
// format between two RMK devices: append kinds, never renumber.
const SPLIT_PERIPHERAL: u8 = 0;
const DONGLE_SEEKING: u8 = 1;

/// An advertisement, named for the peer meant to answer it.
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(test, derive(Debug))]
pub(crate) enum Adv<'a> {
    /// A peer that already knows us: ADV_DIRECT_IND carries no data, only this address.
    Directed(Address),
    /// Any BLE host, which finds us as a standard HID keyboard.
    Host { name: &'a str },
    /// A bonded host reconnecting without HID discovery data.
    HostReconnect,
    /// The split central that owns peripheral `id`.
    SplitPeripheral { id: u8 },
    /// An RMK dongle whose pairing window is open.
    DongleSeeking,
}

/// The advertisement and local address selected for a profile.
pub(crate) struct AdvRequest<'a> {
    pub(crate) adv: Adv<'a>,
    pub(crate) profile: u8,
    pub(crate) addr: [u8; 6],
}

/// Select the active profile's advertisement while its slot and session are available.
/// Only occupancy matters here; connection handles stay owned by the accept loop.
pub(crate) fn advertising_request<'a, L>(
    profile: u8,
    links: &[Option<L>],
    now: Instant,
    deadline: Instant,
    bond: Option<&ProfileInfo>,
    base: [u8; 6],
    name: &'a str,
) -> Option<AdvRequest<'a>> {
    if now >= deadline
        || links.get(profile as usize).is_none_or(Option::is_some)
        || links.iter().flatten().count() >= HOST_LINKS
    {
        return None;
    }
    let adv = match bond {
        Some(_) => Adv::HostReconnect,
        None => Adv::Host { name },
    };
    #[cfg(feature = "dongle")]
    let adv = if profile == crate::ble::profile::DONGLE_PROFILE {
        bond.map_or(Adv::DongleSeeking, |bond| Adv::Directed(bond.info.identity.addr))
    } else {
        adv
    };
    Some(AdvRequest {
        adv,
        profile,
        addr: if cfg!(feature = "_ext_adv") {
            profile_address(base, profile)
        } else {
            base
        },
    })
}

impl Adv<'_> {
    /// Encode into `buf`, which the returned advertisement borrows.
    fn build<'b>(&self, buf: &'b mut [u8; 31]) -> Result<Advertisement<'b>, Error> {
        let adv_data: &[AdStructure] = match *self {
            Self::Directed(peer) => return Ok(Advertisement::ConnectableNonscannableDirected { peer }),
            Self::Host { name } => &[
                AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
                AdStructure::CompleteServiceUuids16(&[BATTERY.to_le_bytes(), HUMAN_INTERFACE_DEVICE.to_le_bytes()]),
                AdStructure::CompleteLocalName(name.as_bytes()),
                AdStructure::Unknown {
                    ty: 0x19, // Appearance, which trouble-host has no variant for
                    data: &KEYBOARD.to_le_bytes(),
                },
            ],
            Self::HostReconnect => &[AdStructure::Flags(BR_EDR_NOT_SUPPORTED)],
            // The two kinds below name themselves to another RMK device and nothing
            // else, undiscoverable: only RMK should act on these, and no host should
            // list them.
            Self::SplitPeripheral { id } => &[
                AdStructure::Flags(BR_EDR_NOT_SUPPORTED),
                AdStructure::ManufacturerSpecificData {
                    company_identifier: RMK_ADV_COMPANY_ID,
                    payload: &[SPLIT_PERIPHERAL, id],
                },
            ],
            Self::DongleSeeking => &[
                AdStructure::Flags(BR_EDR_NOT_SUPPORTED),
                AdStructure::ManufacturerSpecificData {
                    company_identifier: RMK_ADV_COMPANY_ID,
                    payload: &[DONGLE_SEEKING],
                },
            ],
        };
        let len = AdStructure::encode_slice(adv_data, &mut buf[..])?;
        Ok(Advertisement::ConnectableScannableUndirected {
            adv_data: &buf[..len],
            scan_data: &[],
        })
    }

    /// Read an RMK advertisement out of a scan report, or `None` if the report
    /// is not one of ours.
    pub(crate) fn decode(adv_data: &[u8]) -> Option<Adv<'static>> {
        let mut rest = adv_data;
        loop {
            // Every AD structure is a length byte covering the type byte and the data.
            let (&len, tail) = rest.split_first()?;
            let (structure, tail) = tail.split_at_checked(len as usize)?;
            rest = tail;
            // 0xFF is manufacturer-specific data: company id (little-endian), then us.
            let [0xFF, lo, hi, payload @ ..] = structure else {
                continue;
            };
            if u16::from_le_bytes([*lo, *hi]) != RMK_ADV_COMPANY_ID {
                continue;
            }
            return match payload {
                [SPLIT_PERIPHERAL, id] => Some(Adv::SplitPeripheral { id: *id }),
                [DONGLE_SEEKING] => Some(Adv::DongleSeeking),
                // A kind only a newer firmware knows.
                _ => None,
            };
        }
    }

    /// All variants use legacy PDUs, which require the 1M primary PHY.
    fn params(&self) -> AdvertisementParameters {
        let (phy, interval) = match self {
            Self::Host { .. } | Self::HostReconnect => (PhyKind::Le1M, Duration::from_millis(200)),
            _ => (PhyKind::Le1M, Duration::from_millis(50)),
        };
        AdvertisementParameters {
            primary_phy: phy,
            secondary_phy: phy,
            tx_power: TxPower::Plus8dBm,
            interval_min: interval,
            interval_max: interval,
            ..Default::default()
        }
    }
}

#[cfg(feature = "_ext_adv")]
fn advertisement_set<'b>(req: &AdvRequest<'_>, buf: &'b mut [u8; 31]) -> Result<AdvertisementSet<'b>, Error> {
    Ok(AdvertisementSet {
        params: req.adv.params(),
        data: req.adv.build(buf)?,
        address: Some(BdAddr::new(req.addr)),
    })
}

/// Advertise using the profile's local address and return its incoming connection.
#[cfg(feature = "_ext_adv")]
pub(crate) async fn advertise_profile<'a, 'b, C: Controller + ExtAdvCmds, const ATT: usize, const CONN: usize>(
    peripheral: &mut Peripheral<'a, C, DefaultPacketPool>,
    server: &'b AttributeServer<'_, NoopRawMutex, DefaultPacketPool, ATT, CONN>,
    req: &AdvRequest<'_>,
    timeout: Duration,
) -> Result<GattConnection<'a, 'b, DefaultPacketPool>, BleHostError<C::Error>> {
    let mut buf = [0; 31];
    let sets = [advertisement_set(req, &mut buf)?];
    let mut handles = AdvertisementSet::handles(&sets);
    let advertiser = peripheral.advertise_ext(&sets, &mut handles).await?;
    let conn = with_timeout(timeout, advertiser.accept())
        .await
        .map_err(|_| Error::Timeout)??;
    Ok(conn.with_attribute_server(server)?)
}

/// Broadcast `adv` and hand back the connection a central makes on it, or
/// [`Error::Timeout`] if none does within `timeout`.
pub(crate) async fn advertise<'a, 'b, C: Controller, const ATT: usize, const CONN: usize>(
    peripheral: &mut Peripheral<'a, C, DefaultPacketPool>,
    server: &'b AttributeServer<'_, NoopRawMutex, DefaultPacketPool, ATT, CONN>,
    adv: Adv<'_>,
    timeout: Duration,
) -> Result<GattConnection<'a, 'b, DefaultPacketPool>, BleHostError<C::Error>> {
    let mut buf = [0; 31];
    let advertiser = peripheral.advertise(&adv.params(), adv.build(&mut buf)?).await?;
    let conn = with_timeout(timeout, advertiser.accept())
        .await
        .map_err(|_| Error::Timeout)??;
    Ok(conn.with_attribute_server(server)?)
}

#[cfg(test)]
mod tests {
    use rmk_types::ble::BLE_ADV_NAME_MAX_LEN;

    use super::{Adv, Duration, HOST_LINKS, Instant, ProfileInfo, advertising_request};

    #[test]
    fn background_disconnect_does_not_advertise_an_already_connected_active_profile() {
        let now = Instant::from_millis(0);
        let deadline = Instant::from_millis(300_000);
        let mut links = [Some(()), Some(()), None];
        let base = [1, 2, 3, 4, 5, 0xC6];
        assert!(advertising_request(0, &links, now, deadline, None, base, "RMK").is_none());
        links[1] = None;
        assert!(advertising_request(0, &links, now, deadline, None, base, "RMK").is_none());
        links[0] = None;
        assert!(advertising_request(0, &links, now, deadline, None, base, "RMK").is_some());
    }

    #[test]
    fn advertising_waits_for_capacity_and_captures_the_selected_profile() {
        let now = Instant::from_millis(0);
        let deadline = Instant::from_millis(300_000);
        let mut links = [Some(()), None, None];
        let base = [1, 2, 3, 4, 5, 0xC6];
        assert_eq!(
            advertising_request(1, &links, now, deadline, None, base, "RMK").is_some(),
            HOST_LINKS > 1,
        );
        links[0] = None;
        let req = advertising_request(1, &links, now, deadline, None, base, "RMK").unwrap();
        assert_eq!(req.profile, 1);
        assert_eq!(req.adv, Adv::Host { name: "RMK" });
        assert_eq!(
            req.addr,
            if cfg!(feature = "_ext_adv") {
                [1, 2, 3, 5, 5, 0xC6]
            } else {
                base
            }
        );
        assert!(advertising_request(3, &links, now, deadline, None, base, "RMK").is_none());
    }

    #[test]
    fn expired_sessions_stop_reconnect_advertising() {
        let now = Instant::from_millis(300_000);
        let links = [None::<()>; 3];
        let bond = ProfileInfo::default();
        let base = [1, 2, 3, 4, 5, 0xC6];
        assert!(advertising_request(0, &links, now, now, Some(&bond), base, "RMK").is_none());
        let req =
            advertising_request(0, &links, now, now + Duration::from_secs(300), Some(&bond), base, "RMK").unwrap();
        assert_eq!(req.adv, Adv::HostReconnect);
    }

    #[cfg(feature = "dongle")]
    #[test]
    fn dongle_profile_uses_its_own_advertising_kind() {
        use crate::ble::profile::{BOND_SLOTS, DONGLE_PROFILE};

        let links = [None::<()>; BOND_SLOTS];
        let base = [1, 2, 3, 4, 5, 0xC6];
        let now = Instant::from_millis(0);
        let deadline = Instant::from_millis(300_000);
        let seeking = advertising_request(DONGLE_PROFILE, &links, now, deadline, None, base, "RMK").unwrap();
        assert_eq!(seeking.adv, Adv::DongleSeeking);
        let bond = ProfileInfo::default();
        let directed = advertising_request(DONGLE_PROFILE, &links, now, deadline, Some(&bond), base, "RMK").unwrap();
        assert_eq!(directed.adv, Adv::Directed(bond.info.identity.addr));
    }

    /// Overrunning the 31-byte legacy advertisement only fails at runtime.
    fn fits(adv: Adv<'_>) -> bool {
        adv.build(&mut [0; 31]).is_ok()
    }

    #[test]
    fn every_advertisement_fits_the_legacy_budget() {
        assert!(fits(Adv::SplitPeripheral { id: 0xFF }));
        assert!(fits(Adv::DongleSeeking));
        assert!(fits(Adv::HostReconnect));
        // Where BLE_ADV_NAME_MAX_LEN comes from: this name fills the budget exactly.
        const LONGEST_NAME: &str = "0123456789abcdef";
        assert_eq!(LONGEST_NAME.len(), BLE_ADV_NAME_MAX_LEN);
        assert!(fits(Adv::Host { name: LONGEST_NAME }));
        assert!(!fits(Adv::Host {
            name: "0123456789abcdefg"
        }));
    }

    #[test]
    fn host_reconnect_has_only_flags() {
        let mut buf = [0; 31];
        let trouble_host::prelude::Advertisement::ConnectableScannableUndirected { adv_data, scan_data } =
            Adv::HostReconnect.build(&mut buf).unwrap()
        else {
            panic!("reconnection must use undirected advertising");
        };
        assert_eq!(adv_data, [0x02, 0x01, 0x04]);
        assert!(scan_data.is_empty());
    }

    #[cfg(feature = "_ext_adv")]
    #[test]
    fn advertisement_set_uses_profile_address() {
        use embassy_time::Duration;
        use trouble_host::prelude::{Address, BdAddr, PhyKind};

        let addr = [1, 2, 3, 4, 5, 0xC6];
        for (adv, interval_ms) in [
            (Adv::Host { name: "RMK" }, 200),
            (Adv::HostReconnect, 200),
            (Adv::Directed(Address::random([6, 5, 4, 3, 2, 0xC1])), 50),
            (Adv::SplitPeripheral { id: 0 }, 50),
            (Adv::DongleSeeking, 50),
        ] {
            let req = super::AdvRequest { adv, profile: 1, addr };
            let mut buf = [0; 31];
            let set = super::advertisement_set(&req, &mut buf).unwrap();
            assert_eq!(set.address, Some(BdAddr::new(addr)));
            assert_eq!(set.params.primary_phy, PhyKind::Le1M);
            assert_eq!(set.params.secondary_phy, PhyKind::Le1M);
            assert_eq!(set.params.interval_min, Duration::from_millis(interval_ms));
            assert_eq!(set.params.interval_max, Duration::from_millis(interval_ms));
        }
    }

    #[test]
    fn every_rmk_kind_round_trips_through_an_advertisement() {
        for adv in [Adv::SplitPeripheral { id: 2 }, Adv::DongleSeeking] {
            let mut buf = [0; 31];
            adv.build(&mut buf).unwrap();
            assert_eq!(Adv::decode(&buf), Some(adv));
        }
    }

    #[test]
    fn decode_skips_preceding_structures() {
        // An 18-byte 128-bit service UUID list sits between the flags and the MSD.
        let mut data = [0u8; 27];
        data[..5].copy_from_slice(&[0x02, 0x01, 0x06, 0x11, 0x07]);
        data[21..].copy_from_slice(&[0x05, 0xFF, 0x53, 0x52, 0x00, 0x02]);
        assert_eq!(Adv::decode(&data), Some(Adv::SplitPeripheral { id: 2 }));
    }

    #[test]
    fn decode_rejects_foreign_unknown_and_malformed_reports() {
        // Another vendor's company id.
        assert_eq!(
            Adv::decode(&[0x02, 0x01, 0x04, 0x05, 0xFF, 0x4C, 0x00, 0x00, 0x02]),
            None
        );
        // A kind a newer firmware knows and this one does not.
        assert_eq!(Adv::decode(&[0x04, 0xFF, 0x53, 0x52, 0x7F]), None);
        // A length running past the end of the report.
        assert_eq!(Adv::decode(&[0x02, 0x01, 0x04, 0x09, 0xFF, 0x53]), None);
    }
}
