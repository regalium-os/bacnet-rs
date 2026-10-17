// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! A BVLL-IPv6 message on the wire: decoded as sent, encoded back the same.

use super::error::{Bvll6Error, Bvll6ErrorKind};
use super::{Bvll6, Function6, TYPE, Vmac};
use std::net::{Ipv6Addr, SocketAddrV6};

/// Octets of a B/IPv6 address: sixteen of address, two of port.
const ADDRESS: usize = 18;

fn address(octets: &[u8]) -> Option<SocketAddrV6> {
    let ip: [u8; 16] = octets.get(..16)?.try_into().ok()?;
    let port: [u8; 2] = octets.get(16..18)?.try_into().ok()?;
    Some(SocketAddrV6::new(
        Ipv6Addr::from(ip),
        u16::from_be_bytes(port),
        0,
        0,
    ))
}

fn put_address(a: &SocketAddrV6, out: &mut Vec<u8>) {
    out.extend_from_slice(&a.ip().octets());
    out.extend_from_slice(&a.port().to_be_bytes());
}

impl<'a> Bvll6<'a> {
    /// Decodes the BVLL-IPv6 message that fills `datagram`. Borrows it.
    ///
    /// # Errors
    ///
    /// [`Bvll6ErrorKind::Truncated`] without a header and virtual MAC,
    /// [`Bvll6ErrorKind::NotBacnetIpv6`] for another type octet, and
    /// [`Bvll6ErrorKind::WrongLength`] for fields that do not fit their
    /// function.
    pub fn decode(datagram: &'a [u8]) -> Result<Self, Bvll6Error<'a>> {
        let fail = |kind| Bvll6Error {
            kind,
            bytes: datagram,
        };
        let short = |needed| {
            fail(Bvll6ErrorKind::Truncated {
                needed,
                available: datagram.len(),
            })
        };
        let [kind, function, hi, lo, ..] = *datagram else {
            return Err(short(4));
        };
        if kind != TYPE {
            return Err(fail(Bvll6ErrorKind::NotBacnetIpv6(kind)));
        }
        let length = u16::from_be_bytes([hi, lo]);
        if function == 0x0B {
            let data = datagram.get(4..).unwrap_or(&[]);
            return Ok(Self {
                length,
                source: None,
                function: Function6::SecureBvll(data),
            });
        }
        let source = datagram
            .get(4..7)
            .and_then(Vmac::from_octets)
            .ok_or_else(|| short(7))?;
        let d = datagram.get(7..).unwrap_or(&[]);
        let wrong = || {
            fail(Bvll6ErrorKind::WrongLength {
                function,
                len: d.len(),
            })
        };
        let vmac = |at: usize| {
            d.get(at..at + 3)
                .and_then(Vmac::from_octets)
                .ok_or_else(wrong)
        };
        let exactly = |n: usize| if d.len() == n { Ok(()) } else { Err(wrong()) };
        let word = || match d {
            [hi, lo] => Ok(u16::from_be_bytes([*hi, *lo])),
            _ => Err(wrong()),
        };
        let function = match function {
            0x00 => Function6::Result(word()?),
            0x01 => Function6::OriginalUnicast {
                destination: vmac(0)?,
                npdu: d.get(3..).unwrap_or(&[]),
            },
            0x02 => Function6::OriginalBroadcast(d),
            0x03 => exactly(3)
                .and_then(|()| vmac(0))
                .map(Function6::AddressResolution)?,
            0x04 => exactly(3 + ADDRESS).and_then(|()| {
                Ok(Function6::ForwardedAddressResolution {
                    target: vmac(0)?,
                    origin: d.get(3..).and_then(address).ok_or_else(wrong)?,
                })
            })?,
            0x05 => exactly(3)
                .and_then(|()| vmac(0))
                .map(Function6::AddressResolutionAck)?,
            0x06 => exactly(0).map(|()| Function6::VirtualAddressResolution)?,
            0x07 => exactly(3)
                .and_then(|()| vmac(0))
                .map(Function6::VirtualAddressResolutionAck)?,
            0x08 => Function6::Forwarded {
                origin: address(d).ok_or_else(wrong)?,
                npdu: d.get(ADDRESS..).unwrap_or(&[]),
            },
            0x09 => Function6::RegisterForeignDevice { ttl: word()? },
            0x0A => exactly(ADDRESS)
                .and_then(|()| address(d).ok_or_else(wrong))
                .map(Function6::DeleteFdtEntry)?,
            0x0C => Function6::DistributeBroadcast(d),
            function => Function6::Unknown { function, data: d },
        };
        Ok(Self {
            length,
            source: Some(source),
            function,
        })
    }

    /// Appends the message: the length field as held, then the fields.
    pub fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&[TYPE, code(&self.function)]);
        out.extend_from_slice(&self.length.to_be_bytes());
        if let Some(s) = self.source {
            out.extend_from_slice(&s.octets());
        }
        body(&self.function, out);
    }
}

const fn code(f: &Function6<'_>) -> u8 {
    match f {
        Function6::Result(_) => 0x00,
        Function6::OriginalUnicast { .. } => 0x01,
        Function6::OriginalBroadcast(_) => 0x02,
        Function6::AddressResolution(_) => 0x03,
        Function6::ForwardedAddressResolution { .. } => 0x04,
        Function6::AddressResolutionAck(_) => 0x05,
        Function6::VirtualAddressResolution => 0x06,
        Function6::VirtualAddressResolutionAck(_) => 0x07,
        Function6::Forwarded { .. } => 0x08,
        Function6::RegisterForeignDevice { .. } => 0x09,
        Function6::DeleteFdtEntry(_) => 0x0A,
        Function6::SecureBvll(_) => 0x0B,
        Function6::DistributeBroadcast(_) => 0x0C,
        Function6::Unknown { function, .. } => *function,
    }
}

/// Appends a function's fields, after the virtual MAC.
pub(super) fn body(f: &Function6<'_>, out: &mut Vec<u8>) {
    match f {
        Function6::Result(c) => out.extend_from_slice(&c.to_be_bytes()),
        Function6::RegisterForeignDevice { ttl } => out.extend_from_slice(&ttl.to_be_bytes()),
        Function6::OriginalUnicast { destination, npdu } => {
            out.extend_from_slice(&destination.octets());
            out.extend_from_slice(npdu);
        }
        Function6::AddressResolution(v)
        | Function6::AddressResolutionAck(v)
        | Function6::VirtualAddressResolutionAck(v) => {
            out.extend_from_slice(&v.octets());
        }
        Function6::ForwardedAddressResolution { target, origin } => {
            out.extend_from_slice(&target.octets());
            put_address(origin, out);
        }
        Function6::Forwarded { origin, npdu } => {
            put_address(origin, out);
            out.extend_from_slice(npdu);
        }
        Function6::DeleteFdtEntry(a) => put_address(a, out),
        Function6::OriginalBroadcast(d)
        | Function6::DistributeBroadcast(d)
        | Function6::SecureBvll(d)
        | Function6::Unknown { data: d, .. } => out.extend_from_slice(d),
        Function6::VirtualAddressResolution => {}
    }
}
