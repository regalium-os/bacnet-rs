// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Binds the telemetry seam to the the-protobuf-project telemetry SDK.
//!
//! This is the only crate in the workspace allowed to name an OpenTelemetry
//! type, and the reason no other crate has to. `cargo xtask deps` fails the
//! build if `opentelemetry` appears anywhere else.
//!
//! The SDK's `Tracer` and `Span` are concrete structs rather than traits, so
//! the binding is a hand-written implementation of the seam's traits over
//! them — there is no structural adapter to be had, unlike `osdp-go`, whose
//! Go SDK exposes a method value that can be matched by shape.
//!
//! # Layer contract
//!
//! Owns: construction and shutdown of the SDK pipeline, translation of seam
//! attributes into SDK attributes, and the mapping from seam metric
//! instruments onto the SDK's.
//!
//! Must not: be imported by the core, a datalink, or the schema crate. An
//! application chooses a telemetry backend; a protocol codec does not.
//!
//! Allowed imports: `regalium-bacnet-telemetry`, and the SDK.
