// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! An APDU on the wire (§20.1): decoded from octets exactly as sent, and
//! encoded back to the same octets.

use super::limits::Limits;
use super::pdu::{Apdu, Pdu, SEG, Segment};
use crate::cursor::Cursor;
use crate::error::{Error, ErrorKind};

impl<'a> Apdu<'a> {
    /// Decodes the APDU that fills `buf`. Borrows `buf`; copies nothing.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::PduType`] for a type of 8–15; [`ErrorKind::Truncated`]
    /// when the buffer ends inside the fixed header.
    pub fn decode(buf: &'a [u8]) -> Result<Self, Error<'a>> {
        let mut c = Cursor::new(buf);
        let first = c.u8()?;
        let flags = first & 0x0F;
        let segment = |c: &mut Cursor<'a>| -> Result<_, Error<'a>> {
            if flags & SEG == 0 {
                return Ok(None);
            }
            Ok(Some(Segment {
                sequence: c.u8()?,
                window: c.u8()?,
            }))
        };
        // Struct fields are evaluated in the order written, which here is
        // the order they sit on the wire.
        let pdu = match first >> 4 {
            0 => Pdu::ConfirmedRequest {
                limits: Limits(c.u8()?),
                invoke_id: c.u8()?,
                segment: segment(&mut c)?,
                service: c.u8()?,
                body: c.rest(),
            },
            1 => Pdu::UnconfirmedRequest {
                service: c.u8()?,
                body: c.rest(),
            },
            2 => Pdu::SimpleAck {
                invoke_id: c.u8()?,
                service: c.u8()?,
            },
            3 => Pdu::ComplexAck {
                invoke_id: c.u8()?,
                segment: segment(&mut c)?,
                service: c.u8()?,
                body: c.rest(),
            },
            4 => Pdu::SegmentAck {
                invoke_id: c.u8()?,
                sequence: c.u8()?,
                window: c.u8()?,
            },
            5 => Pdu::Error {
                invoke_id: c.u8()?,
                service: c.u8()?,
                body: c.rest(),
            },
            6 => Pdu::Reject {
                invoke_id: c.u8()?,
                reason: c.u8()?,
            },
            7 => Pdu::Abort {
                invoke_id: c.u8()?,
                reason: c.u8()?,
            },
            t => return Err(Error::new(ErrorKind::PduType(t), buf, 0)),
        };
        Ok(Self {
            flags,
            pdu,
            trailing: c.rest(),
        })
    }

    /// Appends the APDU's octets to `out`.
    pub fn encode(&self, out: &mut Vec<u8>) {
        let segment = match self.pdu {
            Pdu::ConfirmedRequest { segment, .. } | Pdu::ComplexAck { segment, .. } => {
                Some(segment)
            }
            _ => None,
        };
        let mut flags = self.flags & 0x0F;
        if let Some(s) = segment {
            flags = (flags & !SEG) | if s.is_some() { SEG } else { 0 };
        }
        out.push((self.pdu_type() << 4) | flags);
        let seg = |out: &mut Vec<u8>, s: Option<Segment>| {
            if let Some(s) = s {
                out.extend_from_slice(&[s.sequence, s.window]);
            }
        };
        match self.pdu {
            Pdu::ConfirmedRequest {
                limits,
                invoke_id,
                segment,
                service,
                body,
            } => {
                out.extend_from_slice(&[limits.0, invoke_id]);
                seg(out, segment);
                out.push(service);
                out.extend_from_slice(body);
            }
            Pdu::ComplexAck {
                invoke_id,
                segment,
                service,
                body,
            } => {
                out.push(invoke_id);
                seg(out, segment);
                out.push(service);
                out.extend_from_slice(body);
            }
            Pdu::UnconfirmedRequest { service, body } => {
                out.push(service);
                out.extend_from_slice(body);
            }
            Pdu::Error {
                invoke_id,
                service,
                body,
            } => {
                out.extend_from_slice(&[invoke_id, service]);
                out.extend_from_slice(body);
            }
            Pdu::SimpleAck {
                invoke_id,
                service: b,
            }
            | Pdu::Reject {
                invoke_id,
                reason: b,
            }
            | Pdu::Abort {
                invoke_id,
                reason: b,
            } => out.extend_from_slice(&[invoke_id, b]),
            Pdu::SegmentAck {
                invoke_id,
                sequence,
                window,
            } => {
                out.extend_from_slice(&[invoke_id, sequence, window]);
            }
        }
        out.extend_from_slice(self.trailing);
    }
}
