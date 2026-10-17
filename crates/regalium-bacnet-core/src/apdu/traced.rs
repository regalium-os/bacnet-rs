// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The application layer's spans, `bacnet.apdu.decode` and
//! `bacnet.apdu.encode`: one per APDU, saying which type, service and
//! transaction it was — enough to follow a request to its answer across
//! traces — and never what the body held.

use super::pdu::{Apdu, Pdu};
use crate::error::Error;
use regalium_bacnet_telemetry::{Attribute, Field, Span, Traced, Tracer, keys};

/// What a span records about one APDU.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    /// Octets in the APDU.
    pub octets: usize,
    /// [`Apdu::type_name`], empty when it did not decode.
    pub kind: &'static str,
    /// The service choice, for every type but Segment-ACK, Reject, Abort.
    pub service: Option<u8>,
    /// The invoke id, for every type but Unconfirmed-Request.
    pub invoke_id: Option<u8>,
    /// Whether it is one segment of several.
    pub segmented: bool,
}

impl Traced for Summary {
    fn attributes<'s>(&'s self, out: &mut dyn FnMut(Attribute<'s>)) {
        out(self.octets.attribute(keys::PDU_OCTETS));
        if !self.kind.is_empty() {
            out(self.kind.attribute(keys::APDU_TYPE));
        }
        if let Some(s) = &self.service {
            out(s.attribute(keys::SERVICE_CHOICE));
        }
        if let Some(i) = &self.invoke_id {
            out(i.attribute(keys::INVOKE_ID));
        }
        out(self.segmented.attribute(keys::SEGMENTED));
    }
}

impl Summary {
    /// The summary of an APDU of `octets` octets.
    #[must_use]
    pub const fn of(octets: usize, apdu: &Apdu<'_>) -> Self {
        let (service, invoke_id, segmented) = match apdu.pdu {
            Pdu::ConfirmedRequest {
                invoke_id,
                segment,
                service,
                ..
            }
            | Pdu::ComplexAck {
                invoke_id,
                segment,
                service,
                ..
            } => (Some(service), Some(invoke_id), segment.is_some()),
            Pdu::UnconfirmedRequest { service, .. } => (Some(service), None, false),
            Pdu::SimpleAck { invoke_id, service }
            | Pdu::Error {
                invoke_id, service, ..
            } => (Some(service), Some(invoke_id), false),
            Pdu::SegmentAck { invoke_id, .. }
            | Pdu::Reject { invoke_id, .. }
            | Pdu::Abort { invoke_id, .. } => (None, Some(invoke_id), false),
        };
        Self {
            octets,
            kind: apdu.type_name(),
            service,
            invoke_id,
            segmented,
        }
    }
}

/// [`Apdu::decode`] inside a `bacnet.apdu.decode` span, which records the
/// [`Summary`] and, on failure, the error's [`reason`](Error::reason).
///
/// # Errors
///
/// As [`Apdu::decode`].
pub fn decode_traced<'a>(buf: &'a [u8], tracer: &dyn Tracer) -> Result<Apdu<'a>, Error<'a>> {
    let span = Span::start(tracer, "bacnet.apdu.decode");
    match Apdu::decode(buf) {
        Ok(apdu) => {
            span.record(&Summary::of(buf.len(), &apdu));
            span.end();
            Ok(apdu)
        }
        Err(e) => {
            span.record(&Summary {
                octets: buf.len(),
                ..Summary::default()
            });
            span.fail(&e.reason());
            Err(e)
        }
    }
}

/// [`Apdu::encode`] inside a `bacnet.apdu.encode` span.
pub fn encode_traced(apdu: &Apdu<'_>, out: &mut Vec<u8>, tracer: &dyn Tracer) {
    let span = Span::start(tracer, "bacnet.apdu.encode");
    let start = out.len();
    apdu.encode(out);
    span.record(&Summary::of(out.len() - start, apdu));
}
