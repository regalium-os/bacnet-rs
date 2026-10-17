// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! A header on the wire: decoded from octets exactly as spelled, and encoded
//! back to the same octets (§20.2.1).

use super::error::{Error, ErrorKind};
use super::header::{Class, Header, Length, Lvt};

impl Header {
    /// Decodes the header at the front of `buf`, returning it and its size.
    /// Borrows `buf`; copies nothing. `offset` is where `buf` starts in the
    /// caller's buffer, for the error.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::Truncated`] when the header runs past the buffer;
    /// [`ErrorKind::ReservedLvt`] for an application tag using 6 or 7.
    pub fn decode(buf: &[u8], offset: usize) -> Result<(Self, usize), Error<'_>> {
        let short = |needed| {
            Error::new(
                ErrorKind::Truncated {
                    needed,
                    available: buf.len(),
                },
                buf,
                offset,
            )
        };
        let first = *buf.first().ok_or_else(|| short(1))?;
        let class = if first & 0x08 == 0 {
            Class::Application
        } else {
            Class::Context
        };
        let (number, extended_number, mut at) = match first >> 4 {
            0x0F => (*buf.get(1).ok_or_else(|| short(2))?, true, 2),
            n => (n, false, 1),
        };
        let code = first & 0x07;
        let lvt = match (class, number, code) {
            (Class::Application, 1, v) => Lvt::Boolean(v),
            (Class::Context, _, 6) => Lvt::Opening,
            (Class::Context, _, 7) => Lvt::Closing,
            (Class::Application, _, 6 | 7) => {
                return Err(Error::new(ErrorKind::ReservedLvt(code), buf, offset));
            }
            (_, _, 5) => {
                let escape = *buf.get(at).ok_or_else(|| short(at + 1))?;
                at += 1;
                let mut wide = |n: usize| -> Result<u32, Error<'_>> {
                    let octets = buf.get(at..at + n).ok_or_else(|| short(at + n))?;
                    at += n;
                    Ok(octets.iter().fold(0, |acc, b| (acc << 8) | u32::from(*b)))
                };
                Lvt::Length(match escape {
                    254 => Length::Extended16(u16::try_from(wide(2)?).unwrap_or(0)),
                    255 => Length::Extended32(wide(4)?),
                    n => Length::Extended8(n),
                })
            }
            (_, _, n) => Lvt::Length(Length::Inline(n)),
        };
        Ok((
            Self {
                class,
                number,
                extended_number,
                lvt,
            },
            at,
        ))
    }

    /// Appends the header exactly as it was spelled.
    pub fn encode(&self, out: &mut Vec<u8>) {
        let class = if self.class == Class::Context {
            0x08
        } else {
            0
        };
        let nibble = if self.extended_number {
            0xF0
        } else {
            self.number << 4
        };
        let code = match self.lvt {
            Lvt::Length(Length::Inline(n)) => n & 0x07,
            Lvt::Length(_) => 5,
            Lvt::Boolean(v) => v & 0x07,
            Lvt::Opening => 6,
            Lvt::Closing => 7,
        };
        out.push(nibble | class | code);
        if self.extended_number {
            out.push(self.number);
        }
        match self.lvt {
            Lvt::Length(Length::Extended8(n)) => out.push(n),
            Lvt::Length(Length::Extended16(n)) => {
                out.push(254);
                out.extend(n.to_be_bytes());
            }
            Lvt::Length(Length::Extended32(n)) => {
                out.push(255);
                out.extend(n.to_be_bytes());
            }
            _ => {}
        }
    }
}
