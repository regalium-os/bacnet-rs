// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The network layer's span, `bacnet.npdu.decode`: one per datagram, saying
//! what kind of NPDU it was and never what it carried — the addresses are
//! a site's topology, and the body is equipment's data.

use super::frame::{Body, Npdu};
use crate::error::Error;
use regalium_bacnet_telemetry::{Span, Tracer, keys, trace_attrs};

/// What a span records about one NPDU.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    /// Octets in the NPDU.
    pub octets: usize,
    /// Whether it carries a network layer message.
    pub message: bool,
    /// Whether it names a destination or source network.
    pub routed: bool,
}

trace_attrs! { Summary => {
    octets: keys::PDU_OCTETS,
    message: keys::NPDU_MESSAGE,
    routed: keys::NPDU_ROUTED,
} }

impl Summary {
    /// The summary of a decoded NPDU of `octets` octets.
    #[must_use]
    pub const fn of(octets: usize, npdu: &Npdu<'_>) -> Self {
        Self {
            octets,
            message: matches!(npdu.body, Body::Message(_)),
            routed: npdu.destination.is_some() || npdu.source.is_some(),
        }
    }
}

/// [`Npdu::decode`] inside a `bacnet.npdu.decode` span, which records the
/// [`Summary`] and, on failure, the error's [`reason`](Error::reason).
///
/// # Errors
///
/// As [`Npdu::decode`].
pub fn decode_traced<'a>(buf: &'a [u8], tracer: &dyn Tracer) -> Result<Npdu<'a>, Error<'a>> {
    let span = Span::start(tracer, "bacnet.npdu.decode");
    match Npdu::decode(buf) {
        Ok(npdu) => {
            span.record(&Summary::of(buf.len(), &npdu));
            span.end();
            Ok(npdu)
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
