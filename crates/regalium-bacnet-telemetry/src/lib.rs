// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Telemetry seam for the `BACnet` driver: the traits every layer records
//! through, and nothing else.
//!
//! Observability for this project goes through the the-protobuf-project
//! telemetry SDK, never through an OpenTelemetry import. This crate is the
//! whole seam — it says what the layers need, and
//! `regalium-bacnet-telemetry-otel` attaches the SDK to it.
//!
//! That is [`Meter`]: declared [`Instrument`]s recorded with [`Attribute`]s
//! built from the declared [`keys`]; and [`Tracer`], whose spans record only
//! what a type declares with [`trace_attrs!`], nested parent to child; and
//! [`Logger`], whose lines carry the span they happened in.
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
//! Owns: the meter, tracer and logger traits and their no-op implementations, the
//! span guard, instrument and attribute declarations, the attribute key
//! constants, and the `trace_attrs!` declaration macro.
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
//!
//! # Metrics
//!
//! Instrument names are `bacnet.<thing>.<measure>` and labels are only ever
//! [`keys`]: which device, which object, which service — never a value read
//! from equipment. The probe's instruments are declared in its `metrics`
//! module; the driver's arrive with the runtime.

mod attribute;
pub mod keys;
mod logs;
mod meter;
mod trace;
mod traced;

pub use attribute::{Attribute, AttributeValue, Key};
pub use logs::{Level, Logger, NoopLogger};
pub use meter::{Instrument, Kind, Meter, NoopMeter};
pub use trace::{NoopTracer, Span, SpanId, Tracer};
pub use traced::{Field, Traced};
