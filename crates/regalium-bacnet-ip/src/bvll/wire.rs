// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! A BVLL message on the wire: decoded as sent, encoded back the same.

use super::entries::{Bdt, Fdt, address, put_address};
use super::{Bvll, BvllError, BvllErrorKind, Function, TYPE};

impl<'a> Bvll<'a> {
    /// Decodes the BVLL message that fills `datagram`. Borrows it.
    ///
    /// # Errors
    ///
    /// [`BvllErrorKind::Truncated`] under four octets,
    /// [`BvllErrorKind::NotBacnetIp`] for another type octet, and
    /// [`BvllErrorKind::WrongLength`] for data that does not fit its function.
    pub fn decode(datagram: &'a [u8]) -> Result<Self, BvllError<'a>> {
        let fail = |kind| BvllError {
            kind,
            bytes: datagram,
        };
        let [kind, function, hi, lo, ..] = *datagram else {
            return Err(fail(BvllErrorKind::Truncated {
                needed: 4,
                available: datagram.len(),
            }));
        };
        if kind != TYPE {
            return Err(fail(BvllErrorKind::NotBacnetIp(kind)));
        }
        let data = datagram.get(4..).unwrap_or(&[]);
        let wrong = || {
            fail(BvllErrorKind::WrongLength {
                function,
                len: data.len(),
            })
        };
        let word = || match data {
            [hi, lo] => Ok(u16::from_be_bytes([*hi, *lo])),
            _ => Err(wrong()),
        };
        let none = || {
            if data.is_empty() {
                Ok(())
            } else {
                Err(wrong())
            }
        };
        let function = match function {
            0x00 => Function::Result(word()?),
            0x01 => Function::WriteBdt(Bdt::new(data).ok_or_else(wrong)?),
            0x02 => none().map(|()| Function::ReadBdt)?,
            0x03 => Function::ReadBdtAck(Bdt::new(data).ok_or_else(wrong)?),
            0x04 => Function::Forwarded {
                origin: address(data).ok_or_else(wrong)?,
                npdu: data.get(6..).unwrap_or(&[]),
            },
            0x05 => Function::RegisterForeignDevice { ttl: word()? },
            0x06 => none().map(|()| Function::ReadFdt)?,
            0x07 => Function::ReadFdtAck(Fdt::new(data).ok_or_else(wrong)?),
            0x08 if data.len() == 6 => Function::DeleteFdtEntry(address(data).ok_or_else(wrong)?),
            0x08 => return Err(wrong()),
            0x09 => Function::DistributeBroadcast(data),
            0x0A => Function::OriginalUnicast(data),
            0x0B => Function::OriginalBroadcast(data),
            0x0C => Function::SecureBvll(data),
            function => Function::Unknown { function, data },
        };
        Ok(Self {
            length: u16::from_be_bytes([hi, lo]),
            function,
        })
    }

    /// Appends the message: the length field as held, then the data.
    pub fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&[TYPE, code(&self.function)]);
        out.extend_from_slice(&self.length.to_be_bytes());
        body(&self.function, out);
    }
}

const fn code(f: &Function<'_>) -> u8 {
    match f {
        Function::Result(_) => 0x00,
        Function::WriteBdt(_) => 0x01,
        Function::ReadBdt => 0x02,
        Function::ReadBdtAck(_) => 0x03,
        Function::Forwarded { .. } => 0x04,
        Function::RegisterForeignDevice { .. } => 0x05,
        Function::ReadFdt => 0x06,
        Function::ReadFdtAck(_) => 0x07,
        Function::DeleteFdtEntry(_) => 0x08,
        Function::DistributeBroadcast(_) => 0x09,
        Function::OriginalUnicast(_) => 0x0A,
        Function::OriginalBroadcast(_) => 0x0B,
        Function::SecureBvll(_) => 0x0C,
        Function::Unknown { function, .. } => *function,
    }
}

/// Appends a function's data, after the header.
pub(super) fn body(f: &Function<'_>, out: &mut Vec<u8>) {
    match f {
        Function::Result(code) => out.extend_from_slice(&code.to_be_bytes()),
        Function::RegisterForeignDevice { ttl } => out.extend_from_slice(&ttl.to_be_bytes()),
        Function::WriteBdt(t) | Function::ReadBdtAck(t) => out.extend_from_slice(t.octets()),
        Function::ReadFdtAck(t) => out.extend_from_slice(t.octets()),
        Function::Forwarded { origin, npdu } => {
            put_address(*origin, out);
            out.extend_from_slice(npdu);
        }
        Function::DeleteFdtEntry(a) => put_address(*a, out),
        Function::DistributeBroadcast(d)
        | Function::OriginalUnicast(d)
        | Function::OriginalBroadcast(d)
        | Function::SecureBvll(d)
        | Function::Unknown { data: d, .. } => out.extend_from_slice(d),
        Function::ReadBdt | Function::ReadFdt => {}
    }
}
