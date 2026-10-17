// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The field probe's library: just enough `BACnet`/IP to discover devices,
//! read their points, follow them live, and export them as metrics.
//!
//! # Layer contract
//!
//! Owns, layering inward:
//!
//! | Module | Owns |
//! | --- | --- |
//! | `tag` | ASHRAE 135 §20.2 tag codec |
//! | `frame` | Annex J BVLL (incl. foreign device registration) and Clause 6 NPDU |
//! | `service` | `ReadProperty`, `ReadPropertyMultiple`, Who-Is/I-Am, Who-Has/I-Have, `SubscribeCOV` and notifications |
//! | `client` | a blocking client over one UDP socket: transactions, discovery sweeps, COV, BBMD registration |
//! | `scope` | IPv4 ranges to sweep and accept answers from |
//! | `scan` | reading devices into plain data, batched through `ReadPropertyMultiple` |
//! | `metrics` | every instrument the probe records, and the labels on them |
//! | `names`, `props`, `explain`, `json`, `dissect` | turning numbers into meaning, JSON, and packet breakdowns |
//! | `sim` | a simulated lift gateway, for tests and demonstrations |
//! | `api` | the schema's device, object and property services, served over `client` |
//!
//! Must not: name an OpenTelemetry type, or decide where metrics go. It
//! records through the seam's `Meter`; the binary chooses the exporter.
//!
//! Allowed imports: `regalium-bacnet-telemetry`, and `regalium-bacnet-schema`
//! for the messages `api` takes and returns.
//!
//! # Interim codec
//!
//! The probe carries its own codec because the driver's core decodes nothing
//! until Phase 3; it moves onto `regalium-bacnet` at Phase 8, the first live
//! read, and this codec is deleted. It is deliberately the smallest codec
//! that reads and changes a real controller: no segmentation (requests are
//! kept small enough not to need it).

pub mod api;
pub mod client;
pub mod dissect;
pub mod explain;
pub mod frame;
pub mod identity;
pub mod json;
pub mod metrics;
pub mod names;
pub mod props;
pub mod scan;
pub mod scope;
pub mod service;
pub mod sim;
pub mod tag;

pub use client::{Client, Device, Direction, Discovery, ForeignError, ReadError};
pub use identity::Identity;
pub use scope::Scope;
pub use tag::{ObjectId, Value};

/// Property identifiers (ASHRAE 135 §21, `BACnetPropertyIdentifier`).
pub mod property {
    /// `model-name`.
    pub const MODEL_NAME: u32 = 70;
    /// `object-list`, an array: index 0 is its length.
    pub const OBJECT_LIST: u32 = 76;
    /// `object-name`.
    pub const OBJECT_NAME: u32 = 77;
    /// `present-value`.
    pub const PRESENT_VALUE: u32 = 85;
    /// `units`.
    pub const UNITS: u32 = 117;
    /// `vendor-name`.
    pub const VENDOR_NAME: u32 = 121;
}
