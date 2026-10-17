// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `BACnet` Virtual Link Layer messages (Annex J.2): the four-octet header and
//! the thirteen functions BACnet/IP adds to UDP.
//!
//! UDP has no broadcast that crosses an IP router, and `BACnet` discovery is
//! built on broadcasts. BVLL is the patch: a BBMD on each subnet relays
//! broadcasts to its peers as Forwarded-NPDUs, and a host on a subnet with
//! no BBMD registers as a foreign device with one that has.
//!
//! # Lossless
//!
//! [`Bvll::length`] keeps the length field as sent, even when it disagrees
//! with the datagram, and [`Bvll::encode`] writes it back. Bodies borrow the
//! datagram.

mod entries;
mod error;
mod wire;

pub use entries::{ADDRESS_LEN, Bdt, BdtEntry, Fdt, FdtEntry, address, put_address};
pub use error::{BvllError, BvllErrorKind};

use std::net::SocketAddrV4;

/// The BVLL type octet of BACnet/IP (Annex J.2.1).
pub const TYPE: u8 = 0x81;

/// BVLC-Result codes (Annex J.2.1.1).
pub mod result {
    /// Successful completion.
    pub const SUCCESS: u16 = 0x0000;
    /// Write-Broadcast-Distribution-Table refused.
    pub const WRITE_BDT_NAK: u16 = 0x0010;
    /// Read-Broadcast-Distribution-Table refused.
    pub const READ_BDT_NAK: u16 = 0x0020;
    /// Register-Foreign-Device refused: the table is full, or the BBMD takes
    /// no foreign devices.
    pub const REGISTER_FOREIGN_DEVICE_NAK: u16 = 0x0030;
    /// Read-Foreign-Device-Table refused.
    pub const READ_FDT_NAK: u16 = 0x0040;
    /// Delete-Foreign-Device-Table-Entry refused: no such entry.
    pub const DELETE_FDT_ENTRY_NAK: u16 = 0x0050;
    /// Distribute-Broadcast-To-Network refused: the sender is not registered.
    pub const DISTRIBUTE_BROADCAST_NAK: u16 = 0x0060;
}

/// One BVLL message. Borrows the datagram it was decoded from.
///
/// `Copy`, holding only borrows: safe to share and send across threads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bvll<'a> {
    /// The length field as sent: the whole message, header included.
    pub length: u16,
    /// The function and its data.
    pub function: Function<'a>,
}

/// The BVLC functions (Annex J.2.1–J.2.13), and their data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Function<'a> {
    /// X'00' BVLC-Result: a code from [`result`].
    Result(u16),
    /// X'01' Write-Broadcast-Distribution-Table.
    WriteBdt(Bdt<'a>),
    /// X'02' Read-Broadcast-Distribution-Table.
    ReadBdt,
    /// X'03' Read-Broadcast-Distribution-Table-Ack.
    ReadBdtAck(Bdt<'a>),
    /// X'04' Forwarded-NPDU: a broadcast a BBMD relays, with the address of
    /// the station that first sent it — the one an answer goes to.
    Forwarded {
        /// The original sender.
        origin: SocketAddrV4,
        /// The NPDU.
        npdu: &'a [u8],
    },
    /// X'05' Register-Foreign-Device, for `ttl` seconds.
    RegisterForeignDevice {
        /// Time-to-live, seconds.
        ttl: u16,
    },
    /// X'06' Read-Foreign-Device-Table.
    ReadFdt,
    /// X'07' Read-Foreign-Device-Table-Ack.
    ReadFdtAck(Fdt<'a>),
    /// X'08' Delete-Foreign-Device-Table-Entry.
    DeleteFdtEntry(SocketAddrV4),
    /// X'09' Distribute-Broadcast-To-Network: a foreign device's broadcast,
    /// for its BBMD to distribute.
    DistributeBroadcast(&'a [u8]),
    /// X'0A' Original-Unicast-NPDU.
    OriginalUnicast(&'a [u8]),
    /// X'0B' Original-Broadcast-NPDU.
    OriginalBroadcast(&'a [u8]),
    /// X'0C' Secure-BVLL (Clause 24), carried undecoded.
    SecureBvll(&'a [u8]),
    /// A function code Annex J does not define, kept as sent.
    Unknown {
        /// The code.
        function: u8,
        /// Its data.
        data: &'a [u8],
    },
}

impl<'a> Bvll<'a> {
    /// A message whose length field is the length it encodes to.
    #[must_use]
    pub fn new(function: Function<'a>) -> Self {
        let mut body = Vec::new();
        wire::body(&function, &mut body);
        Self {
            length: u16::try_from(body.len() + 4).unwrap_or(u16::MAX),
            function,
        }
    }

    /// The message's octets, with a correct length: what a sender builds.
    #[must_use]
    pub fn to_vec(function: Function<'a>) -> Vec<u8> {
        let mut out = Vec::new();
        Self::new(function).encode(&mut out);
        out
    }

    /// The function's name as Annex J spells it, lower-case and hyphenated.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self.function {
            Function::Result(_) => "bvlc-result",
            Function::WriteBdt(_) => "write-broadcast-distribution-table",
            Function::ReadBdt => "read-broadcast-distribution-table",
            Function::ReadBdtAck(_) => "read-broadcast-distribution-table-ack",
            Function::Forwarded { .. } => "forwarded-npdu",
            Function::RegisterForeignDevice { .. } => "register-foreign-device",
            Function::ReadFdt => "read-foreign-device-table",
            Function::ReadFdtAck(_) => "read-foreign-device-table-ack",
            Function::DeleteFdtEntry(_) => "delete-foreign-device-table-entry",
            Function::DistributeBroadcast(_) => "distribute-broadcast-to-network",
            Function::OriginalUnicast(_) => "original-unicast-npdu",
            Function::OriginalBroadcast(_) => "original-broadcast-npdu",
            Function::SecureBvll(_) => "secure-bvll",
            Function::Unknown { .. } => "unknown",
        }
    }

    /// The NPDU the message carries, for the functions that carry one.
    #[must_use]
    pub const fn npdu(&self) -> Option<&'a [u8]> {
        match self.function {
            Function::Forwarded { npdu, .. }
            | Function::DistributeBroadcast(npdu)
            | Function::OriginalUnicast(npdu)
            | Function::OriginalBroadcast(npdu) => Some(npdu),
            _ => None,
        }
    }
}
