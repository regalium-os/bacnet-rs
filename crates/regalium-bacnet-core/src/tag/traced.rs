// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The tag layer's spans: `bacnet.tag.decode` and `bacnet.tag.encode`, one
//! per buffer, never one per element — a datagram holds dozens of elements,
//! and a span each would cost more than the decode.
//!
//! What a span may say about a buffer is declared on [`Decoded`] and
//! nowhere else: its size, how many elements, and whether every header was
//! canonical. Never the contents, which are values read from equipment.

use super::element::{Element, decode_all, encode_all};
use super::error::Error;
use regalium_bacnet_telemetry::{Span, Tracer, keys, trace_attrs};

/// What a span records about one buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decoded {
    /// Octets in the buffer.
    pub octets: usize,
    /// Elements in it.
    pub elements: usize,
    /// Whether every header was spelled the shortest way.
    pub canonical: bool,
}

trace_attrs! { Decoded => {
    octets: keys::TAG_OCTETS,
    elements: keys::TAG_ELEMENTS,
    canonical: keys::TAG_CANONICAL,
} }

impl Decoded {
    fn of(octets: usize, elements: &[Element<'_>]) -> Self {
        Self {
            octets,
            elements: elements.len(),
            canonical: elements.iter().all(|e| e.header.is_canonical()),
        }
    }
}

/// [`decode_all`] inside a `bacnet.tag.decode` span, which records
/// [`Decoded`] and, on failure, the error's [`reason`](Error::reason) —
/// never its octets.
///
/// # Errors
///
/// As [`decode_all`].
pub fn decode_all_traced<'a>(
    buf: &'a [u8],
    tracer: &dyn Tracer,
) -> Result<Vec<Element<'a>>, Error<'a>> {
    let span = Span::start(tracer, "bacnet.tag.decode");
    match decode_all(buf) {
        Ok(elements) => {
            span.record(&Decoded::of(buf.len(), &elements));
            span.end();
            Ok(elements)
        }
        Err(e) => {
            span.record(&Decoded {
                octets: buf.len(),
                elements: 0,
                canonical: true,
            });
            span.fail(&e.reason());
            Err(e)
        }
    }
}

/// [`encode_all`] inside a `bacnet.tag.encode` span.
#[must_use]
pub fn encode_all_traced(elements: &[Element<'_>], tracer: &dyn Tracer) -> Vec<u8> {
    let span = Span::start(tracer, "bacnet.tag.encode");
    let out = encode_all(elements);
    span.record(&Decoded::of(out.len(), elements));
    out
}
