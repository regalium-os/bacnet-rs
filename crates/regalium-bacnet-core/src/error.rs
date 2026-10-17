// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Why octets did not decode, and which octets.
//!
//! An error borrows the input: [`Error::bytes`] is the slice that could not
//! be read, starting at the element that failed, so a caller logging it
//! logs what the device sent rather than a description of it. Matching is
//! on [`ErrorKind`], never on a message.
//!
//! One error serves every layer of the core — tags, the APDU and the NPDU —
//! so a caller decoding a datagram matches one type however deep the fault.

use core::fmt;

/// What was wrong. Non-exhaustive: a later layer adds its own kinds, and a
/// caller's `match` must not break when it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorKind {
    /// The buffer ended inside a header or its contents: `needed` octets
    /// were required from the start of the element, `available` were there.
    Truncated {
        /// Octets the element needs.
        needed: usize,
        /// Octets the buffer had.
        available: usize,
    },
    /// An application tag used a length/value/type of 6 or 7, which only a
    /// context tag may (§20.2.1.3.2, opening and closing tags).
    ReservedLvt(u8),
    /// The contents are the wrong length for the type: a REAL of 3 octets,
    /// an Unsigned of 0 (§20.2.4–§20.2.14).
    WrongLength {
        /// Application tag number of the type being decoded.
        tag: u8,
        /// Octets the contents had.
        len: usize,
    },
    /// The element is not the type asked for: a context tag where an
    /// application one was expected, or an opening tag where a value was.
    WrongTag,
    /// The contents are the right length but not a valid value of the
    /// type: a BOOLEAN of 2, a bit string claiming 9 unused bits.
    InvalidValue {
        /// Application tag number of the type being decoded.
        tag: u8,
    },
    /// An opening tag without its closing tag, or a closing tag with no
    /// opening one, or one closing a different number (§20.2.1.3.2).
    Unbalanced,
    /// An NPDU whose protocol version is not 1, the only one Clause 6
    /// defines: what follows cannot be assumed to have its layout.
    Version(u8),
    /// A network layer message whose data is the wrong length for its type
    /// (§6.4): an odd-length network list, a reject of two octets.
    MessageLength {
        /// The message type.
        message: u8,
        /// Octets of data it had.
        len: usize,
    },
    /// An APDU type of 8–15, which §20.1.1 reserves.
    PduType(u8),
    /// A service body without the field its grammar requires next: the
    /// context tag number expected, or 255 for an application-tagged one.
    MissingField(u8),
    /// Octets after the last field of a service's grammar.
    Trailing,
}

/// A decode failure and the octets it happened at. Borrows the input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error<'a> {
    /// What was wrong.
    pub kind: ErrorKind,
    /// The input from the element that failed to the end of the buffer.
    pub bytes: &'a [u8],
    /// Where `bytes` starts within the buffer decoding was asked of.
    pub offset: usize,
}

impl<'a> Error<'a> {
    pub(crate) const fn new(kind: ErrorKind, bytes: &'a [u8], offset: usize) -> Self {
        Self {
            kind,
            bytes,
            offset,
        }
    }
}

impl fmt::Display for Error<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            ErrorKind::Truncated { needed, available } => {
                write!(f, "truncated: {needed} octets needed, {available} present")?;
            }
            ErrorKind::ReservedLvt(lvt) => {
                write!(f, "application tag with reserved length/value/type {lvt}")?;
            }
            ErrorKind::WrongLength { tag, len } => {
                write!(f, "application type {tag} cannot be {len} octets")?;
            }
            ErrorKind::WrongTag => f.write_str("not the type expected here")?,
            ErrorKind::InvalidValue { tag } => {
                write!(f, "not a valid value of application type {tag}")?;
            }
            ErrorKind::Unbalanced => f.write_str("opening and closing tags do not pair")?,
            ErrorKind::Version(v) => write!(f, "NPDU version {v}, not 1")?,
            ErrorKind::MessageLength { message, len } => {
                write!(f, "network message {message:#04x} cannot be {len} octets")?;
            }
            ErrorKind::PduType(t) => write!(f, "reserved APDU type {t}")?,
            ErrorKind::MissingField(255) => f.write_str("missing application-tagged field")?,
            ErrorKind::MissingField(n) => write!(f, "missing context field [{n}]")?,
            ErrorKind::Trailing => f.write_str("octets after the service's last field")?,
        }
        write!(f, " at offset {}", self.offset)?;
        for b in self.bytes.iter().take(8) {
            write!(f, " {b:02x}")?;
        }
        if self.bytes.len() > 8 {
            f.write_str(" …")?;
        }
        Ok(())
    }
}

impl std::error::Error for Error<'_> {}

impl Error<'_> {
    /// What went wrong and where, without the octets: what a span may
    /// carry. The octets are equipment's data, and stay with the caller.
    #[must_use]
    pub fn reason(&self) -> Reason {
        Reason {
            kind: self.kind,
            offset: self.offset,
        }
    }
}

/// An [`Error`] without its octets. See [`Error::reason`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reason {
    /// What was wrong.
    pub kind: ErrorKind,
    /// Where.
    pub offset: usize,
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let full = Error::new(self.kind, &[], self.offset).to_string();
        f.write_str(full.trim_end())
    }
}
