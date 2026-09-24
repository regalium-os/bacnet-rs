// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `BACnet` Secure Connect datalink — ASHRAE 135 Annex AB.
//!
//! `BACnet` over `WebSocket` and TLS: the hub-and-spoke and direct-connect
//! topologies, BVLC-SC message framing, connection establishment with
//! certificate-based device identity, and the heartbeat that distinguishes a
//! quiet peer from a gone one.
//!
//! Nothing Regalium runs today needs this — both Schindler PICS documents
//! declare non-secure devices — but BACnet/SC is where the standard is going,
//! and it is the direct analogue of `osdp-go`'s Secure Channel: the layer
//! where a mistake is a security bug rather than a decode error.
//!
//! # Layer contract
//!
//! Owns: BVLC-SC framing, the WebSocket connection, TLS configuration, peer
//! certificate validation, and the hub connection state machine.
//!
//! Must not: interpret an APDU. Never record key material, a certificate's
//! private half, or a session secret as a span attribute.
//!
//! Allowed imports: `regalium-bacnet-core`, `regalium-bacnet-telemetry`.
//!
//! # Spans
//!
//! `bacnet.sc.{connect,send,receive}`, `bacnet.sc.hub.{join,heartbeat}`.
