// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The `BACnet` domain model, generated from the protobuf source of truth.
//!
//! `protobuf/` is the authority for what a device, object, point and event
//! are, shaped to Google AIP. Three artefacts come out of it and this crate
//! carries all three:
//!
//! - **prost types** for the configuration surface, where a few hundred
//!   nanoseconds of parsing never mattered;
//! - **`FlatBuffers` and Cap’n Proto mirrors** of the hot-path payloads — COV
//!   notifications, readings, event notifications — derived by `buffers` from
//!   the same descriptor set, never hand-written;
//! - the **module wiring** that makes them reachable, which is hand-written
//!   because `buffers` emits no `mod.rs` for Rust.
//!
//! # Why the core does not import this
//!
//! The protocol layers do not depend on the generated types. Doing so would
//! put prost in the dependency graph of every consumer of the codec, and the
//! codec's whole value is that it has no graph. Conversion lives here and
//! imports the core, not the other way round.
//!
//! # Ordinals are a wire format
//!
//! `buffers.lock` records the target slot every field was assigned. A
//! consumer compiled when a field sat at ordinal 5 reads slot 5 forever, and
//! no compiler in the chain reports it when a rebuild puts something else
//! there. `just gen all` fails on a moved ordinal rather than shipping it.
//!
//! # Layer contract
//!
//! Owns: generated types and the conversions between them and the core's
//! protocol types.
//!
//! Must not: appear in the dependency graph of the core or any datalink.
//!
//! Allowed imports: `regalium-bacnet-telemetry`.
