// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! What the client knows about each device, and what it reports about them.

use regalium_bacnet_core::apdu::Segments;
use regalium_bacnet_core::enumerated::Segmentation;
use regalium_bacnet_core::tsm::{Abandoned, Remote};

/// The far side of a router (§6.2.2): the network a device is on and its
/// MAC there, which the NPDU names so the router can deliver.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Route {
    /// The device's network number.
    pub network: u16,
    /// Its MAC on that network: one octet on MS/TP, six on BACnet/IP.
    pub mac: Vec<u8>,
}

/// A bound device: how to reach it, and what it said it can take.
///
/// Plain data: `Send` and `Sync` when the MAC is.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Device<M> {
    /// The device instance.
    pub instance: u32,
    /// Where datagrams for it go on this datalink: the device itself, or
    /// the router in front of it.
    pub address: M,
    /// Set when it is behind a router.
    pub route: Option<Route>,
    /// Max-APDU-length-accepted, from its I-Am.
    pub max_apdu: u32,
    /// Segmentation-supported, from its I-Am.
    pub segmentation: Segmentation,
    /// Vendor-identifier, from its I-Am.
    pub vendor_id: u32,
}

impl<M> Device<M> {
    /// The limits a request to it must respect, given the most this
    /// datalink carries.
    #[must_use]
    pub fn remote(&self, link_max: u16) -> Remote {
        let own = u16::try_from(self.max_apdu).unwrap_or(u16::MAX);
        Remote {
            max_apdu: own.min(link_max),
            segmented_requests: self.segmentation.receives(),
            max_segments: Segments::Unspecified,
        }
    }
}

/// Something that happened to a device, as the client saw it. Offline is
/// here and not an error: on a building network a device going quiet is a
/// fact to report, and the caller decides what it means.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Event {
    /// A device answered a Who-Is, or announced itself, for the first time.
    Discovered(u32),
    /// A request to a reachable device went unanswered.
    Offline {
        /// The device.
        instance: u32,
        /// Why the request was given up.
        reason: Abandoned,
    },
    /// A device reported offline answered again.
    Reachable(u32),
}
