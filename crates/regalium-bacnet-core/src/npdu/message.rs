// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Network layer messages (§6.4): what routers say to each other and to the
//! devices that ask them where a network is.

use crate::error::{Error, ErrorKind};

/// A network layer message as it arrived: its type, the vendor of a
/// proprietary one, and its data undecoded. Borrows the NPDU's buffer.
///
/// Kept raw so that a message this crate cannot read still round-trips;
/// [`Message::decode`] reads the ones it can.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Message<'a> {
    /// The message type (§6.2.4, Table 6-1). X'80' and above are
    /// proprietary.
    pub kind: u8,
    /// The vendor identifier, which follows a proprietary message type and
    /// only a proprietary one. Written when present.
    pub vendor: Option<u16>,
    /// The message's data.
    pub data: &'a [u8],
}

impl<'a> Message<'a> {
    pub(crate) fn encode(&self, out: &mut Vec<u8>) {
        out.push(self.kind);
        if let Some(v) = self.vendor {
            out.extend_from_slice(&v.to_be_bytes());
        }
        out.extend_from_slice(self.data);
    }

    /// The message type's name in Table 6-1, or `proprietary`/`reserved`.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self.kind {
            0x00 => "who-is-router-to-network",
            0x01 => "i-am-router-to-network",
            0x02 => "i-could-be-router-to-network",
            0x03 => "reject-message-to-network",
            0x04 => "router-busy-to-network",
            0x05 => "router-available-to-network",
            0x06 => "initialize-routing-table",
            0x07 => "initialize-routing-table-ack",
            0x08 => "establish-connection-to-network",
            0x09 => "disconnect-connection-to-network",
            0x12 => "what-is-network-number",
            0x13 => "network-number-is",
            0x80.. => "proprietary",
            _ => "reserved",
        }
    }

    /// Reads the data of the messages a client meets. Copies nothing: a
    /// network list borrows the data.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::MessageLength`] when the data is the wrong length for
    /// the type. The message itself is still whole and still re-encodes.
    pub fn decode(&self) -> Result<NetworkMessage<'a>, Error<'a>> {
        let d = self.data;
        let wrong = || {
            Error::new(
                ErrorKind::MessageLength {
                    message: self.kind,
                    len: d.len(),
                },
                d,
                0,
            )
        };
        let word = |at: usize| match d.get(at..at + 2) {
            Some(&[hi, lo]) => Ok(u16::from_be_bytes([hi, lo])),
            _ => Err(wrong()),
        };
        let byte = |at: usize| d.get(at).copied().ok_or_else(wrong);
        let list = || {
            if d.len().is_multiple_of(2) {
                Ok(Networks(d))
            } else {
                Err(wrong())
            }
        };
        let exactly = |n: usize| if d.len() == n { Ok(()) } else { Err(wrong()) };
        Ok(match self.kind {
            0x00 if d.is_empty() => NetworkMessage::WhoIsRouter(None),
            0x00 => exactly(2)
                .and_then(|()| word(0))
                .map(|n| NetworkMessage::WhoIsRouter(Some(n)))?,
            0x01 => NetworkMessage::IAmRouter(list()?),
            0x02 => exactly(3).and_then(|()| {
                Ok(NetworkMessage::ICouldBeRouter {
                    network: word(0)?,
                    performance: byte(2)?,
                })
            })?,
            0x03 => exactly(3).and_then(|()| {
                Ok(NetworkMessage::Reject {
                    reason: byte(0)?,
                    network: word(1)?,
                })
            })?,
            0x04 => NetworkMessage::RouterBusy(list()?),
            0x05 => NetworkMessage::RouterAvailable(list()?),
            0x12 => exactly(0).map(|()| NetworkMessage::WhatIsNetworkNumber)?,
            0x13 => exactly(3).and_then(|()| {
                Ok(NetworkMessage::NetworkNumberIs {
                    network: word(0)?,
                    flag: byte(2)?,
                })
            })?,
            _ => NetworkMessage::Undecoded,
        })
    }
}

/// The meaning of a [`Message`] whose type this crate reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkMessage<'a> {
    /// Who routes to this network — or, with none, to which networks?
    WhoIsRouter(Option<u16>),
    /// I route to these networks. Sent in answer, and broadcast by a router
    /// when it starts, so devices learn a path without asking.
    IAmRouter(Networks<'a>),
    /// I could reach this network over a dial-up connection.
    ICouldBeRouter {
        /// The network.
        network: u16,
        /// How well, 0 best.
        performance: u8,
    },
    /// I could not deliver to this network. Reasons (§6.4.4): 0 other,
    /// 1 not directly connected and no path known, 2 router busy, 3 unknown
    /// message type, 4 message too long, 5 security error, 6 bad addressing.
    Reject {
        /// Why.
        reason: u8,
        /// The network it was for.
        network: u16,
    },
    /// Stop sending me traffic for these networks; empty means all of them.
    RouterBusy(Networks<'a>),
    /// I can take traffic for these networks again; empty means all.
    RouterAvailable(Networks<'a>),
    /// What network am I on? Asked by a device that was not configured.
    WhatIsNetworkNumber,
    /// This network's number.
    NetworkNumberIs {
        /// The number.
        network: u16,
        /// 1 when the number was configured, 0 when learned (§6.4.20).
        flag: u8,
    },
    /// A type this crate does not read: routing table exchange, dial-up
    /// connections, proprietary messages. Its [`Message`] still has it all.
    Undecoded,
}

/// A list of network numbers, two octets each. Borrows the message data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Networks<'a>(&'a [u8]);

impl Networks<'_> {
    /// The networks, in the order sent.
    pub fn iter(&self) -> impl Iterator<Item = u16> + '_ {
        self.0
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| u16::from_be_bytes(*p))
    }

    /// Whether the list is empty, which for busy and available means all.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
