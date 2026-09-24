// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `BACnet` MS/TP master datalink — ASHRAE 135 Clause 9.
//!
//! The master node state machine over RS-485: token passing, poll-for-master,
//! sole-master operation, and the frame codec with its two CRCs — an 8-bit
//! header CRC and a 16-bit data CRC.
//!
//! Timing is the whole difficulty of MS/TP, so none of it is hard-coded here.
//! The state machine returns the next state and the deadline that state
//! expires at; the runtime owns the waiting. That is what makes token loss,
//! silence and a duplicate master testable without a bus.
//!
//! # Layer contract
//!
//! Owns: the master node state machine, MS/TP framing, both CRCs, and the
//! serial port.
//!
//! Must not: interpret an APDU, or sleep.
//!
//! Allowed imports: `regalium-bacnet-core`, `regalium-bacnet-telemetry`.
//!
//! # Spans
//!
//! `bacnet.mstp.{send,receive}`, `bacnet.mstp.token.{pass,recover}`,
//! `bacnet.mstp.frame.decode`.
