// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The envelope around every APDU: the Annex J BVLL header, then the Clause 6
//! NPDU with its optional destination and source network addresses.
//!
//! `bvll` holds the BVLL messages that carry no NPDU at all: foreign device
//! registration and its result.
//!
//! Since Phase 8 the octets are the driver's: these functions keep the
//! probe's shape — a route, a delivery, a borrowed APDU — and hand the
//! encoding and decoding to `regalium_bacnet::{ip::bvll, npdu}`.

pub mod bvll;

use regalium_bacnet::ip::bvll::{Bvll, Function};
use regalium_bacnet::npdu::{Address, Body, Destination, Npdu};
use std::net::SocketAddr;

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
    let npdu = Npdu {
        control: if expecting_reply { 0x04 } else { 0 },
        // Hop count 255: routers decrement it to kill loops.
        destination: dest.map(|d| Destination {
            address: Address {
                network: d.net,
                mac: &d.mac,
            },
            hop_count: 0xFF,
        }),
        source: None,
        body: Body::Apdu(apdu),
    };
    let mut octets = Vec::new();
    npdu.encode(&mut octets);
    Bvll::to_vec(match delivery {
        Delivery::Unicast => Function::OriginalUnicast(&octets),
        Delivery::Broadcast => Function::OriginalBroadcast(&octets),
        Delivery::Distribute => Function::DistributeBroadcast(&octets),
    })
}

/// Unwraps a datagram, borrowing it. `None` for anything that is not an APDU
/// we can use: another BVLC function, a network-layer message, or a
/// malformed frame. Octets past the BVLL length field are ignored, as the
/// probe always has; a length past the datagram is malformed.
#[must_use]
pub fn decode(datagram: &[u8], peer: SocketAddr) -> Option<Incoming<'_>> {
    let declared = usize::from(u16::from_be_bytes([*datagram.get(2)?, *datagram.get(3)?]));
    let bvll = Bvll::decode(datagram.get(..declared)?).ok()?;
    let (from, delivery, npdu) = match bvll.function {
        Function::OriginalUnicast(n) => (peer, Delivery::Unicast, n),
        Function::OriginalBroadcast(n) => (peer, Delivery::Broadcast, n),
        Function::DistributeBroadcast(n) => (peer, Delivery::Distribute, n),
        Function::Forwarded { origin, npdu } => (SocketAddr::V4(origin), Delivery::Broadcast, npdu),
        _ => return None,
    };
    let npdu = Npdu::decode(npdu).ok()?;
    // A network-layer message, e.g. I-Am-Router-To-Network, carries no APDU.
    let Body::Apdu(apdu) = npdu.body else {
        return None;
    };
    Some(Incoming {
        from,
        source: npdu.source.map(|s| Route {
            net: s.network,
            mac: s.mac.to_vec(),
        }),
        delivery,
        apdu,
    })
}

#[cfg(test)]
mod tests;
