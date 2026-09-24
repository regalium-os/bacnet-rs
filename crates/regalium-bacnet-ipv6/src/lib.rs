// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! BACnet/IPv6 datalink — ASHRAE 135 Annex U.
//!
//! The IPv6 counterpart to Annex J. It is not Annex J with a wider address;
//! IPv6 has no broadcast, so the BBMD's job is done by multicast group
//! membership, and a virtual MAC address layer maps `BACnet` device identity
//! onto it.
//!
//! # Layer contract
//!
//! Owns: BVLC-IPv6 function codes, the virtual MAC table, multicast group
//! membership, and the socket.
//!
//! Must not: interpret an APDU.
//!
//! Allowed imports: `regalium-bacnet-core`, `regalium-bacnet-telemetry`.
//!
//! # Spans
//!
//! `bacnet.ipv6.{send,receive}`, `bacnet.ipv6.vmac.resolve`.
