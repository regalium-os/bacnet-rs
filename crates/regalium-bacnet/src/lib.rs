// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! A pure-Rust `BACnet` driver for `RegaliumOS`.
//!
//! Implements ASHRAE 135 / ISO 16484-5. The protocol layers live in sibling
//! crates; what a consumer can reach is exactly what this crate re-exports,
//! which leaves the internals free to change without breaking anyone.
//!
//! # Re-export, never wrap
//!
//! Everything here is a `pub use`. A re-export is the same type, so a value
//! obtained through this crate satisfies the traits declared inside the layer
//! that defined it, and vice versa — which is what keeps the extension points
//! open. A third party can implement a datalink for a proprietary bridge, or
//! add an object type, without this repository being involved. A newtype
//! wrapper would compile and then silently close every one of those doors.
//!
//! # Layer contract
//!
//! Owns: the public surface, and nothing else. No logic lives here.
//!
//! Allowed imports: every crate in the workspace.

/// Tags (§20.2): lossless decoding and encoding of every element.
pub use regalium_bacnet_core::tag;

/// The thirteen primitive types (§20.2.2–§20.2.14).
pub use regalium_bacnet_core::primitive;

/// The application layer's PDU (§20.1): the eight types, invoke ids and
/// segmentation fields.
pub use regalium_bacnet_core::apdu;

/// The network layer (Clause 6): addresses, hop count, network messages.
pub use regalium_bacnet_core::npdu;

/// Services (Clauses 13–17): Who-Is, I-Am, `ReadProperty` so far.
pub use regalium_bacnet_core::service;

/// Names for object types, properties and errors (§21).
pub use regalium_bacnet_core::enumerated;

/// The protobuf model, per Google AIP, and the conversions into it.
pub use regalium_bacnet_schema as schema;

/// The client transaction state machine (§5.4): invoke ids, retries,
/// segmentation, on a clock the caller supplies.
pub use regalium_bacnet_core::tsm;

/// The one decode error every layer of the core returns.
pub use regalium_bacnet_core::{Error, ErrorKind};

/// The telemetry seam every layer records through.
pub use regalium_bacnet_telemetry as telemetry;

/// The telemetry SDK binding: metrics and spans over OTLP. Opt-in, as it
/// brings the SDK's native compression libraries.
#[cfg(feature = "otel")]
pub use regalium_bacnet_telemetry_otel::Otel;
