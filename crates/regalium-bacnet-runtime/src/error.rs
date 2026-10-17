// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Why a request did not produce a value — matched by variant, never by
//! message.

use core::fmt;
use regalium_bacnet_core::service::ServiceError;
use regalium_bacnet_core::tsm::{Abandoned, Refused};

/// Why one request — a read, a write, a subscription — did not succeed.
/// Each says who to blame: the device (`Error`, `Reject`, `Abort`,
/// `Malformed`), the network (`Offline`), or the caller (`NotBound`,
/// `Refused`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RequestError {
    /// The device is not bound: discover it, or add it.
    NotBound,
    /// No answer after every retry. The device was also reported offline
    /// through [`Client::events`](crate::Client::events).
    Offline(Abandoned),
    /// The device ran the request and refused it: no such object, no such
    /// property, not writable.
    Error(ServiceError),
    /// The device could not parse the request (§20.1.8.3).
    Reject(u8),
    /// The device, or this client, ended the transaction (§20.1.9.3).
    Abort(u8),
    /// The answer did not decode; its octets, for inspection.
    Malformed(Vec<u8>),
    /// The request could not be sent to this device as it stands.
    Refused(Refused),
    /// A batch was stopped before this request finished.
    Unanswered,
    /// What was asked needs a service the device does not implement —
    /// `all` from a device without `ReadPropertyMultiple`.
    Unsupported,
}

impl fmt::Display for RequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotBound => f.write_str("device not bound"),
            Self::Offline(why) => write!(f, "device offline: {why}"),
            Self::Error(e) => write!(f, "device refused: {}", e.describe()),
            Self::Reject(r) => write!(f, "device rejected the request, reason {r}"),
            Self::Abort(r) => write!(f, "transaction aborted, reason {r}"),
            Self::Malformed(o) => write!(f, "answer of {} octets did not decode", o.len()),
            Self::Refused(r) => write!(f, "not sent: {r}"),
            Self::Unanswered => f.write_str("batch stopped before an answer"),
            Self::Unsupported => f.write_str("needs a service the device does not implement"),
        }
    }
}

impl std::error::Error for RequestError {}

/// A call's failure: what happened to the request, or a datalink that
/// failed under it — the one the caller must fix.
#[derive(Debug)]
pub enum Error<E> {
    /// The request failed.
    Request(RequestError),
    /// The datalink failed.
    Link(E),
}

impl<E: fmt::Display> fmt::Display for Error<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Request(e) => e.fmt(f),
            Self::Link(e) => write!(f, "datalink: {e}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Error<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Request(e) => Some(e),
            Self::Link(e) => Some(e),
        }
    }
}
