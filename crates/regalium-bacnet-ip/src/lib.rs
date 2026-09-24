// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! BACnet/IP datalink — ASHRAE 135 Annex J.
//!
//! Wraps NPDUs in BVLL, and owns the parts of Annex J that are not simply a
//! UDP send: directed and broadcast distribution, the BBMD's broadcast
//! distribution table, forwarded-NPDU handling, and foreign device
//! registration with its re-registration deadline.
//!
//! This is the datalink the Schindler Cube exposes to the operator network,
//! and because the Cube routes BACnet/IP to MS/TP, it is the one that reaches
//! every lift controller behind it.
//!
//! # Layer contract
//!
//! Owns: BVLC function codes, the broadcast distribution table, the foreign
//! device table, and the socket. One of the layers permitted to perform I/O.
//!
//! Must not: interpret an APDU. Everything above the NPDU belongs to the core;
//! a datalink that reads a service is a datalink that has to be rewritten when
//! a service is added.
//!
//! Allowed imports: `regalium-bacnet-core`, `regalium-bacnet-telemetry`.
//!
//! # Spans
//!
//! `bacnet.ip.{send,receive,forward}`, `bacnet.ip.bbmd.distribute`,
//! `bacnet.ip.foreign.register`.
