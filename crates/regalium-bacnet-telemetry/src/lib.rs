// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Telemetry seam for the `BACnet` driver: the traits every layer records
//! through, and nothing else.
//!
//! Observability for this project goes through the the-protobuf-project
//! telemetry SDK, never through an OpenTelemetry import. This crate is the
//! whole seam — [`Tracer`], [`Meter`] and [`Logger`] say what the layers need,
//! and `regalium-bacnet-telemetry-otel` attaches the SDK to them.
//!
//! # Why this exists at commit one
//!
//! Spans at layer boundaries are a design constraint, not an observability
//! feature bolted on later. A span wrapping `tag::decode` forces `decode` to
//! take a tracer, which forces its caller to have one, which is what keeps
//! cancellation and attribution flowing through a stack talking to hardware.
//! Retrofitting that means changing every signature in the core.
//!
//! # Layer contract
//!
//! Owns: the tracer, meter and logger traits, their no-op implementations, the
//! attribute-declaration macro, and the attribute key constants.
//!
//! Must not: perform I/O, read a clock, allocate on a no-op path, or name a
//! type from any telemetry implementation. This crate has no dependencies and
//! `cargo xtask purity` fails if one appears.
//!
//! Allowed imports: none.
//!
//! # Attributes are declared, never set ad hoc
//!
//! Rust has no struct tags, so an attribute is declared next to the field it
//! describes with the `trace_attrs!` macro. An undeclared field is never
//! recorded, and that is the safety mechanism rather than an oversight: this
//! driver reads building equipment, and a property value that no macro names
//! cannot leak into a trace through a well-meaning `set_attribute` added in a
//! hot path, because there is no such call to add.
//!
//! # Spans
//!
//! Span names are `bacnet.<layer>.<operation>` — `bacnet.tag.decode`,
//! `bacnet.apdu.decode`, `bacnet.npdu.route`, `bacnet.tsm.transaction`.
//! Each crate's own documentation lists the spans it opens.
