// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! What the state machine tells its runtime to do, and what it refuses.

use core::fmt;
use core::time::Duration;

/// The next thing to do, from [`Tsm::poll`](super::Tsm::poll). `P` is the
/// caller's name for a peer: an address, a device instance — whatever it
/// sends to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step<P> {
    /// Send this APDU to `peer`, inside an NPDU that expects a reply when
    /// it is a request. `attempt` is 1 for a first send, 2 for the first
    /// retry; a Segment-ACK or Abort the machine sends is attempt 1.
    Transmit {
        /// Where.
        peer: P,
        /// The transaction.
        invoke: u8,
        /// Which send this is.
        attempt: u8,
        /// The APDU's octets.
        apdu: Vec<u8>,
    },
    /// The transaction ended with an answer from the peer.
    Complete {
        /// Who answered.
        peer: P,
        /// The transaction.
        invoke: u8,
        /// The answer.
        reply: Reply,
    },
    /// The transaction ended without an answer. Not an error to return:
    /// a device going quiet is an event the caller reports, and the
    /// invoke id is free again.
    Abandon {
        /// The peer that went quiet or was aborted.
        peer: P,
        /// The transaction.
        invoke: u8,
        /// Why.
        reason: Abandoned,
    },
    /// Nothing to do until `wake_at`, or until something arrives; `None`
    /// when no transaction is open.
    Idle {
        /// The earliest deadline of any open transaction.
        wake_at: Option<Duration>,
    },
}

/// How the peer answered (§20.1.4–§20.1.9). Bodies are owned: a segmented
/// answer is reassembled from several datagrams, so it cannot borrow one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reply {
    /// Simple-ACK: done, nothing to return.
    Simple,
    /// Complex-ACK: the service result, reassembled if it was segmented.
    Complex(Vec<u8>),
    /// Error: the service ran and failed; the error class and code (§18).
    Error(Vec<u8>),
    /// Reject: the request was not understood (§20.1.8.3).
    Reject(u8),
    /// Abort sent by the server (§20.1.9.3).
    Abort(u8),
}

/// Why a transaction ended without an answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Abandoned {
    /// No answer after every retry. `attempts` counts every send.
    Timeout {
        /// Sends made, the first included.
        attempts: u8,
    },
    /// A segmented answer stopped arriving part way (§5.4.4.4).
    SegmentTimeout,
    /// This device aborted, with this reason (§20.1.9.3), and sent the
    /// Abort — for an answer it could not take, or a PDU that made no sense
    /// in the transaction's state.
    Aborted(u8),
}

impl fmt::Display for Abandoned {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timeout { attempts } => write!(f, "no answer after {attempts} attempts"),
            Self::SegmentTimeout => f.write_str("segmented answer stopped arriving"),
            Self::Aborted(r) => write!(f, "aborted, reason {r}"),
        }
    }
}

/// Why [`Tsm::request`](super::Tsm::request) would not start a transaction.
/// Each is something the caller must change, so it is an error, not a step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Refused {
    /// All 256 invoke ids are in use: wait for an answer before asking more.
    NoInvokeId,
    /// The request is larger than the peer takes in one APDU, and it does
    /// not reassemble segmented requests.
    NeedsSegmentation,
    /// Segmented, the request needs more segments than the peer takes.
    TooManySegments {
        /// Segments the request needs.
        needed: usize,
    },
    /// The peer's max APDU leaves no room for any service data.
    PeerTooSmall,
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoInvokeId => f.write_str("every invoke id is in use"),
            Self::NeedsSegmentation => {
                f.write_str("request too large for the peer, which does not segment")
            }
            Self::TooManySegments { needed } => {
                write!(
                    f,
                    "request needs {needed} segments, more than the peer takes"
                )
            }
            Self::PeerTooSmall => f.write_str("peer max APDU leaves no room for data"),
        }
    }
}

impl std::error::Error for Refused {}

/// What [`Tsm::receive`](super::Tsm::receive) made of an APDU.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Received {
    /// It belonged to an open transaction; [`Tsm::poll`](super::Tsm::poll)
    /// has whatever follows.
    Matched,
    /// No open transaction has that peer and invoke id, or the service
    /// does not match: a late answer to one already abandoned, most often.
    Unknown,
    /// A PDU a client never receives — a request, or a Segment-ACK or Abort
    /// from a client. It is for a server, which this machine is not.
    NotForClient,
}
