// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The NPDU on the wire (§6.2): version, control octet, the optional
//! addresses and hop count, then an APDU or a network layer message.

use super::address::{Address, Destination};
use super::message::Message;
use crate::cursor::Cursor;
use crate::error::{Error, ErrorKind};

/// Control bit 7: a network layer message follows, not an APDU.
pub(crate) const MESSAGE: u8 = 0x80;
/// Control bit 5: `DNET`, `DLEN`, `DADR` and the hop count are present.
pub(crate) const DESTINATION: u8 = 0x20;
/// Control bit 3: `SNET`, `SLEN` and `SADR` are present.
pub(crate) const SOURCE: u8 = 0x08;
/// Control bit 2: the APDU is a confirmed request, or a message that
/// wants a reply.
pub(crate) const EXPECTING_REPLY: u8 = 0x04;
/// Control bits 6 and 4, which §6.2.2 reserves and requires to be 0.
pub const RESERVED: u8 = 0x50;

/// What an NPDU carries after its header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Body<'a> {
    /// An application layer PDU, undecoded: see [`crate::apdu`].
    Apdu(&'a [u8]),
    /// A network layer message, for routers (§6.4).
    Message(Message<'a>),
}

/// One NPDU, decoded without copying: every slice in it borrows the buffer
/// [`Npdu::decode`] was given, so it lives no longer than that buffer.
///
/// # Lossless
///
/// [`Npdu::control`] keeps the octet as sent, reserved bits included, and
/// [`Npdu::encode`] writes it back. Only the three presence bits — message,
/// destination, source — are written from the fields instead, so that what
/// is written can always be read back. For a decoded NPDU the two agree, and
/// `encode(decode(x)) == x` for every `x` that decodes.
///
/// `Copy`, holding only borrows: safe to share and send across threads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Npdu<'a> {
    /// The control octet as sent (§6.2.2).
    pub control: u8,
    /// `DNET`/`DLEN`/`DADR` and the hop count, when routed to a network.
    pub destination: Option<Destination<'a>>,
    /// `SNET`/`SLEN`/`SADR`, added by the router that forwarded it.
    pub source: Option<Address<'a>>,
    /// The APDU or network message.
    pub body: Body<'a>,
}

impl<'a> Npdu<'a> {
    /// The protocol version, X'01', the only one Clause 6 defines.
    pub const VERSION: u8 = 1;

    /// Decodes an NPDU from the start of `buf`, which must hold exactly
    /// one: everything after the header is the body. Borrows `buf`.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::Version`] for a version other than 1;
    /// [`ErrorKind::Truncated`] when the buffer ends inside the header.
    pub fn decode(buf: &'a [u8]) -> Result<Self, Error<'a>> {
        let mut c = Cursor::new(buf);
        let version = c.u8()?;
        if version != Self::VERSION {
            return Err(Error::new(ErrorKind::Version(version), buf, 0));
        }
        let control = c.u8()?;
        let mut address = |present: bool| -> Result<_, Error<'a>> {
            if !present {
                return Ok(None);
            }
            let network = c.u16()?;
            let len = c.u8()?;
            let mac = c.take(usize::from(len))?;
            Ok(Some(Address { network, mac }))
        };
        let to = address(control & DESTINATION != 0)?;
        let source = address(control & SOURCE != 0)?;
        let destination = match to {
            Some(address) => Some(Destination {
                address,
                hop_count: c.u8()?,
            }),
            None => None,
        };
        let body = if control & MESSAGE == 0 {
            Body::Apdu(c.rest())
        } else {
            let kind = c.u8()?;
            let vendor = if kind >= 0x80 { Some(c.u16()?) } else { None };
            Body::Message(Message {
                kind,
                vendor,
                data: c.rest(),
            })
        };
        Ok(Self {
            control,
            destination,
            source,
            body,
        })
    }

    /// Appends the NPDU's octets to `out`.
    pub fn encode(&self, out: &mut Vec<u8>) {
        let mut control = self.control & !(MESSAGE | DESTINATION | SOURCE);
        if matches!(self.body, Body::Message(_)) {
            control |= MESSAGE;
        }
        if self.destination.is_some() {
            control |= DESTINATION;
        }
        if self.source.is_some() {
            control |= SOURCE;
        }
        out.extend_from_slice(&[Self::VERSION, control]);
        if let Some(d) = &self.destination {
            d.address.encode(out);
        }
        if let Some(s) = &self.source {
            s.encode(out);
        }
        if let Some(d) = &self.destination {
            out.push(d.hop_count);
        }
        match &self.body {
            Body::Apdu(apdu) => out.extend_from_slice(apdu),
            Body::Message(m) => m.encode(out),
        }
    }

    /// Whether the sender expects a reply (§6.2.2): set for a confirmed
    /// request, so a router knows to hold a path open for the answer.
    #[must_use]
    pub const fn expects_reply(&self) -> bool {
        self.control & EXPECTING_REPLY != 0
    }

    /// The network priority (§6.2.2, bits 1 and 0).
    #[must_use]
    pub const fn priority(&self) -> Priority {
        match self.control & 0x03 {
            0 => Priority::Normal,
            1 => Priority::Urgent,
            2 => Priority::CriticalEquipment,
            _ => Priority::LifeSafety,
        }
    }

    /// The reserved control bits that were set, or 0 for a conforming
    /// sender. Kept rather than refused: a router passing them on is how a
    /// site discovers which device sets them.
    #[must_use]
    pub const fn reserved_bits(&self) -> u8 {
        self.control & RESERVED
    }
}

/// A message's network priority (§6.2.2). A router serves higher priorities
/// first when it is congested; it says nothing about the application.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Priority {
    /// B'00', the default.
    #[default]
    Normal,
    /// B'01'.
    Urgent,
    /// B'10'.
    CriticalEquipment,
    /// B'11'.
    LifeSafety,
}
