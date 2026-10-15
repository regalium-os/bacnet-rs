// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The envelope around every APDU: the Annex J BVLL header, then the Clause 6
//! NPDU with its optional destination and source network addresses.
//!
//! `bvll` holds the BVLL messages that carry no NPDU at all: foreign device
//! registration and its result.

pub mod bvll;

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

pub(crate) const BVLC_TYPE: u8 = 0x81;
const FORWARDED_NPDU: u8 = 0x04;
const DISTRIBUTE_BROADCAST: u8 = 0x09;
const ORIGINAL_UNICAST: u8 = 0x0A;
const ORIGINAL_BROADCAST: u8 = 0x0B;

/// A `BACnet` network address beyond the IP hop: a device behind a router,
/// such as an MS/TP controller behind a `BACnet`/IP gateway (§6.2.2).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Route {
    /// DNET/SNET: the network number.
    pub net: u16,
    /// DADR/SADR: the station's MAC on that network; empty is a broadcast.
    pub mac: Vec<u8>,
}

/// DNET 0xFFFF with no MAC: every network, so routers forward the request.
pub const GLOBAL_BROADCAST: Route = Route {
    net: 0xFFFF,
    mac: Vec::new(),
};

/// How a datagram leaves this host (Annex J.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
    /// Original-Unicast-NPDU, to one device or router.
    Unicast,
    /// Original-Broadcast-NPDU, to a subnet broadcast address.
    Broadcast,
    /// Distribute-Broadcast-To-Network: a registered foreign device asks its
    /// BBMD to broadcast on its behalf, because its own broadcasts never
    /// reach the subnet the BBMD serves.
    Distribute,
}

/// A received NPDU carrying an APDU, borrowed from the datagram.
#[derive(Debug)]
pub struct Incoming<'a> {
    /// Where to reply: the UDP peer, or the original sender of a
    /// Forwarded-NPDU relayed by a BBMD.
    pub from: SocketAddr,
    /// SNET/SADR, present when the sender sits behind a router.
    pub source: Option<Route>,
    /// How it arrived; a simulator acting as a BBMD answers distributes.
    pub delivery: Delivery,
    /// The APDU, borrowed from the datagram.
    pub apdu: &'a [u8],
}

/// Wraps `apdu` in an NPDU and a BVLL header.
#[must_use]
pub fn encode(
    delivery: Delivery,
    dest: Option<&Route>,
    expecting_reply: bool,
    apdu: &[u8],
) -> Vec<u8> {
    let mut control = 0u8;
    let mut addressing = Vec::new();
    if let Some(dest) = dest {
        control |= 0x20;
        addressing.extend(dest.net.to_be_bytes());
        addressing.push(u8::try_from(dest.mac.len()).unwrap_or(u8::MAX));
        addressing.extend(&dest.mac);
        addressing.push(0xFF); // Hop count: routers decrement it to kill loops.
    }
    if expecting_reply {
        control |= 0x04;
    }
    let function = match delivery {
        Delivery::Unicast => ORIGINAL_UNICAST,
        Delivery::Broadcast => ORIGINAL_BROADCAST,
        Delivery::Distribute => DISTRIBUTE_BROADCAST,
    };
    let total = 4 + 2 + addressing.len() + apdu.len();
    let mut out = vec![BVLC_TYPE, function];
    out.extend(u16::try_from(total).unwrap_or(u16::MAX).to_be_bytes());
    out.extend([0x01, control]);
    out.extend(addressing);
    out.extend_from_slice(apdu);
    out
}

/// Unwraps a datagram, borrowing it. `None` for anything that is not an APDU
/// we can use: another BVLC function, a network-layer message, or a
/// malformed frame.
#[must_use]
pub fn decode(datagram: &[u8], peer: SocketAddr) -> Option<Incoming<'_>> {
    if *datagram.first()? != BVLC_TYPE {
        return None;
    }
    let len = usize::from(u16::from_be_bytes([*datagram.get(2)?, *datagram.get(3)?]));
    let body = datagram.get(4..len)?;
    let (from, delivery, npdu) = match *datagram.get(1)? {
        ORIGINAL_UNICAST => (peer, Delivery::Unicast, body),
        ORIGINAL_BROADCAST => (peer, Delivery::Broadcast, body),
        DISTRIBUTE_BROADCAST => (peer, Delivery::Distribute, body),
        FORWARDED_NPDU => {
            let [a, b, c, d, p0, p1] = *body.get(..6)? else {
                return None;
            };
            let origin = SocketAddrV4::new(Ipv4Addr::new(a, b, c, d), u16::from_be_bytes([p0, p1]));
            (SocketAddr::V4(origin), Delivery::Broadcast, body.get(6..)?)
        }
        _ => return None,
    };
    if *npdu.first()? != 0x01 {
        return None;
    }
    let control = *npdu.get(1)?;
    let mut pos = 2;
    if control & 0x20 != 0 {
        pos += address(npdu.get(pos..)?)?.1;
    }
    let source = if control & 0x08 != 0 {
        let (route, size) = address(npdu.get(pos..)?)?;
        pos += size;
        Some(route)
    } else {
        None
    };
    if control & 0x20 != 0 {
        pos += 1; // Hop count.
    }
    if control & 0x80 != 0 {
        return None; // Network-layer message, e.g. I-Am-Router-To-Network.
    }
    Some(Incoming {
        from,
        source,
        delivery,
        apdu: npdu.get(pos..)?,
    })
}

fn address(buf: &[u8]) -> Option<(Route, usize)> {
    let net = u16::from_be_bytes([*buf.first()?, *buf.get(1)?]);
    let len = usize::from(*buf.get(2)?);
    let mac = buf.get(3..3 + len)?.to_vec();
    Some((Route { net, mac }, 3 + len))
}

#[cfg(test)]
mod tests;
