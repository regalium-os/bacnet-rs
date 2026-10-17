// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! BVLL for BACnet/IPv6 (Annex U.2): a four-octet header, then — for every
//! function but Secure-BVLL — the sender's virtual MAC address, then the
//! function's fields.
//!
//! The layouts are those of Wireshark's `packet-bvlc.c`, the dissector real
//! captures are read with; the corpus under `fixtures/bvll6/` states each
//! one octet by octet.
//!
//! # Lossless
//!
//! [`Bvll6::length`] keeps the length field as sent and [`Bvll6::encode`]
//! writes it back; bodies borrow the datagram.

mod error;
mod vmac;
mod wire;

pub use error::{Bvll6Error, Bvll6ErrorKind};
pub use vmac::Vmac;

use std::net::SocketAddrV6;

/// The BVLL type octet of BACnet/IPv6 (Annex U.2), where Annex J's is X'81'.
pub const TYPE: u8 = 0x82;

/// BVLC-Result codes (Annex U.2.1.1).
pub mod result {
    /// Successful completion.
    pub const SUCCESS: u16 = 0x0000;
    /// Address-Resolution refused.
    pub const ADDRESS_RESOLUTION_NAK: u16 = 0x0030;
    /// Virtual-Address-Resolution refused.
    pub const VIRTUAL_ADDRESS_RESOLUTION_NAK: u16 = 0x0060;
    /// Register-Foreign-Device refused.
    pub const REGISTER_FOREIGN_DEVICE_NAK: u16 = 0x0090;
    /// Delete-Foreign-Device-Table-Entry refused.
    pub const DELETE_FDT_ENTRY_NAK: u16 = 0x00A0;
    /// Distribute-Broadcast-To-Network refused.
    pub const DISTRIBUTE_BROADCAST_NAK: u16 = 0x00C0;
}

/// One BVLL-IPv6 message. Borrows the datagram it was decoded from.
///
/// `Copy`, holding only borrows: safe to share and send across threads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bvll6<'a> {
    /// The length field as sent: the whole message, header included.
    pub length: u16,
    /// The sender's virtual MAC; absent only from Secure-BVLL.
    pub source: Option<Vmac>,
    /// The function and its fields.
    pub function: Function6<'a>,
}

/// The BVLC-IPv6 functions (Annex U.2.1–U.2.13).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Function6<'a> {
    /// X'00' BVLC-Result.
    Result(u16),
    /// X'01' Original-Unicast-NPDU, to one virtual MAC.
    OriginalUnicast {
        /// The destination.
        destination: Vmac,
        /// The NPDU.
        npdu: &'a [u8],
    },
    /// X'02' Original-Broadcast-NPDU, to the multicast group.
    OriginalBroadcast(&'a [u8]),
    /// X'03' Address-Resolution: who has this virtual MAC?
    AddressResolution(Vmac),
    /// X'04' Forwarded-Address-Resolution: the same, relayed by a BBMD.
    ForwardedAddressResolution {
        /// The virtual MAC sought.
        target: Vmac,
        /// The B/IPv6 address of whoever asked.
        origin: SocketAddrV6,
    },
    /// X'05' Address-Resolution-ACK: I do; to the asker.
    AddressResolutionAck(Vmac),
    /// X'06' Virtual-Address-Resolution: what is your virtual MAC?
    VirtualAddressResolution,
    /// X'07' Virtual-Address-Resolution-ACK: this, to the asker.
    VirtualAddressResolutionAck(Vmac),
    /// X'08' Forwarded-NPDU: a broadcast relayed by a BBMD, with the B/IPv6
    /// address of the station that sent it.
    Forwarded {
        /// The original sender's address.
        origin: SocketAddrV6,
        /// The NPDU.
        npdu: &'a [u8],
    },
    /// X'09' Register-Foreign-Device, for `ttl` seconds.
    RegisterForeignDevice {
        /// Time-to-live, seconds.
        ttl: u16,
    },
    /// X'0A' Delete-Foreign-Device-Table-Entry.
    DeleteFdtEntry(SocketAddrV6),
    /// X'0B' Secure-BVLL (Clause 24), undecoded; it carries no virtual MAC.
    SecureBvll(&'a [u8]),
    /// X'0C' Distribute-Broadcast-To-Network: a foreign device's broadcast.
    DistributeBroadcast(&'a [u8]),
    /// A function code Annex U does not define, kept as sent.
    Unknown {
        /// The code.
        function: u8,
        /// Its data, after the virtual MAC.
        data: &'a [u8],
    },
}

impl<'a> Bvll6<'a> {
    /// A message from `source` whose length field is the length it encodes to.
    #[must_use]
    pub fn new(source: Vmac, function: Function6<'a>) -> Self {
        let source = (!matches!(function, Function6::SecureBvll(_))).then_some(source);
        let mut body = Vec::new();
        wire::body(&function, &mut body);
        let vmac = if source.is_some() { 3 } else { 0 };
        Self {
            length: u16::try_from(4 + vmac + body.len()).unwrap_or(u16::MAX),
            source,
            function,
        }
    }

    /// The message's octets, with a correct length: what a sender builds.
    #[must_use]
    pub fn to_vec(source: Vmac, function: Function6<'a>) -> Vec<u8> {
        let mut out = Vec::new();
        Self::new(source, function).encode(&mut out);
        out
    }

    /// The NPDU the message carries, for the functions that carry one.
    #[must_use]
    pub const fn npdu(&self) -> Option<&'a [u8]> {
        match self.function {
            Function6::OriginalUnicast { npdu, .. }
            | Function6::Forwarded { npdu, .. }
            | Function6::OriginalBroadcast(npdu)
            | Function6::DistributeBroadcast(npdu) => Some(npdu),
            _ => None,
        }
    }
}
