// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The network layer (Clause 6): the header that lets a message cross
//! routers between `BACnet` networks of different datalinks.
//!
//! An [`Npdu`] says where a message is going beyond the local network, where
//! it came from, how many more routers may forward it, and whether it carries
//! an APDU for a device or a [`Message`] for routers. It does not route; a
//! router is a runtime with ports, built on this.
//!
//! # Why a network layer at all
//!
//! A building runs MS/TP on twisted pair to the field and BACnet/IP between
//! floors, and a lift controller can sit on either. The NPDU is what lets a
//! request cross from one to the other unchanged: datalinks differ in how
//! long a MAC address is, so the network layer carries the MAC as opaque
//! octets with its length, and only the destination datalink reads it.
//!
//! # Lossless
//!
//! Decoding borrows the buffer and keeps the control octet as sent, reserved
//! bits included; [`Npdu::encode`] reproduces the input exactly. A message
//! type this crate cannot read still round-trips as its raw data.

mod address;
mod frame;
mod message;
mod traced;

pub use address::{Address, Destination, GLOBAL};
pub use frame::{Body, Npdu, Priority, RESERVED};
pub use message::{Message, NetworkMessage, Networks};
pub use traced::{Summary, decode_traced};
