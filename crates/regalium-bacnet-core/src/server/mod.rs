// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! A local device's answers (§5.4.5, Clauses 15–16): a request in, from any
//! [`Database`], the APDU that answers it out. Pure, like the rest of the
//! core; the runtime is what receives and sends.
//!
//! # Served
//!
//! Who-Is and Who-Has (answered with I-Am and I-Have); `ReadProperty`,
//! `ReadPropertyMultiple` — `all`, `required` and `optional` expanded to
//! every property the object has — and `WriteProperty`. Any other confirmed
//! service is rejected as unrecognized (§20.1.8.3), so a client falls back
//! rather than waits.
//!
//! # Segmentation
//!
//! [`respond`] keeps no state, so an answer larger than the requester takes
//! is aborted as segmentation-not-supported (§5.4.5.2), as a device that
//! does not segment must say. [`Responder`] keeps the state to send it in
//! segments instead, when the requester accepts them. A segmented request
//! is aborted either way: none of the services served needs one.

mod discovery;
mod outgoing;
mod read;
mod segments;

pub use segments::Responder;

use crate::apdu::{Apdu, Pdu};
use crate::error::{Error, ErrorKind};
use crate::obj::Database;
use crate::service::{ServiceError, WriteProperty, confirmed, unconfirmed};

/// Abort reason segmentation-not-supported (§20.1.9.3).
const SEGMENTATION_NOT_SUPPORTED: u8 = 4;
/// Reject reason unrecognized-service (§20.1.8.3).
const UNRECOGNIZED_SERVICE: u8 = 9;

/// An answer, and how it goes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Response {
    /// To the requester: an ACK, an Error, a Reject or an Abort.
    Reply(Vec<u8>),
    /// To everyone: an I-Am or I-Have, which §16 has broadcast so every
    /// client learns the device, not only the one that asked.
    Broadcast(Vec<u8>),
}

/// The answer to `apdu` from `db`, or `None` when it needs none: an
/// unconfirmed request not for this device, an ACK, anything a server
/// does not answer.
pub fn respond(apdu: &Apdu<'_>, db: &mut dyn Database) -> Option<Response> {
    match apdu.pdu {
        Pdu::UnconfirmedRequest {
            service: unconfirmed::WHO_IS,
            body,
        } => discovery::who_is(body, db),
        Pdu::UnconfirmedRequest {
            service: unconfirmed::WHO_HAS,
            body,
        } => discovery::who_has(body, db),
        Pdu::ConfirmedRequest {
            invoke_id,
            segment: Some(_),
            ..
        } => Some(abort(invoke_id)),
        Pdu::ConfirmedRequest {
            invoke_id,
            service,
            limits,
            body,
            ..
        } => {
            let reply = answer(invoke_id, service, body, db);
            let max = usize::from(limits.max_apdu().unwrap_or(50));
            Some(if reply.len() > max {
                abort(invoke_id)
            } else {
                Response::Reply(reply)
            })
        }
        _ => None,
    }
}

/// The whole answer to confirmed `service`, before any question of size.
pub(crate) fn answer(invoke: u8, service: u8, body: &[u8], db: &mut dyn Database) -> Vec<u8> {
    match service {
        confirmed::READ_PROPERTY => read::read_property(invoke, body, db),
        confirmed::READ_PROPERTY_MULTIPLE => read::read_multiple(invoke, body, db),
        confirmed::WRITE_PROPERTY => write_property(invoke, body, db),
        _ => vec![0x60, invoke, UNRECOGNIZED_SERVICE],
    }
}

fn write_property(invoke: u8, body: &[u8], db: &mut dyn Database) -> Vec<u8> {
    let w = match WriteProperty::decode(body) {
        Ok(w) => w,
        Err(e) => return reject(invoke, &e),
    };
    let v = w.value;
    match db.write(
        w.object,
        v.property.property,
        v.property.index,
        v.value,
        v.priority,
    ) {
        Ok(()) => vec![0x20, invoke, confirmed::WRITE_PROPERTY],
        Err(e) => error(invoke, confirmed::WRITE_PROPERTY, e),
    }
}

/// A server's Abort: SRV set, segmentation-not-supported.
pub(crate) fn abort(invoke: u8) -> Response {
    Response::Reply(vec![0x71, invoke, SEGMENTATION_NOT_SUPPORTED])
}

/// An Error PDU with the plain error.
pub(crate) fn error(invoke: u8, service: u8, e: ServiceError) -> Vec<u8> {
    let mut out = vec![0x50, invoke, service];
    e.encode(&mut out);
    out
}

/// A Reject naming why the request did not parse (§20.1.8.3): a missing
/// parameter (5), too many (7), the wrong type (3), or an invalid tag (4).
pub(crate) fn reject(invoke: u8, e: &Error<'_>) -> Vec<u8> {
    let reason = match e.kind {
        ErrorKind::MissingField(_) => 5,
        ErrorKind::Trailing => 7,
        ErrorKind::InvalidValue { .. } | ErrorKind::WrongLength { .. } => 3,
        _ => 4,
    };
    vec![0x60, invoke, reason]
}
