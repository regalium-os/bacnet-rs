// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! BVLL messages with no NPDU: foreign device registration (Annex J.5.2).
//!
//! A device on a subnet without a BBMD of its own cannot hear, or be heard
//! by, broadcasts elsewhere. It registers with a BBMD as a *foreign device*
//! for a time-to-live; the BBMD then forwards it every broadcast it
//! distributes, and broadcasts on its behalf when asked
//! (`Delivery::Distribute`). The registration lapses unless renewed, which is
//! why the standard adds a 30 s grace period (J.5.2.3) and why a client
//! renews well before the TTL runs out.

use regalium_bacnet::ip::bvll::{Bvll, Function, result};

/// Register-Foreign-Device asking for `ttl` seconds.
#[must_use]
pub fn register_foreign_device(ttl: u16) -> Vec<u8> {
    Bvll::to_vec(Function::RegisterForeignDevice { ttl })
}

/// The TTL a Register-Foreign-Device asks for, borrowing the datagram, or
/// `None` if it is something else.
#[must_use]
pub fn registration(datagram: &[u8]) -> Option<u16> {
    match Bvll::decode(datagram).ok()? {
        Bvll {
            length: 6,
            function: Function::RegisterForeignDevice { ttl },
        } if datagram.len() == 6 => Some(ttl),
        _ => None,
    }
}

/// A BVLC-Result carrying `code`.
#[must_use]
pub fn result(code: u16) -> Vec<u8> {
    Bvll::to_vec(Function::Result(code))
}

/// The code of a BVLC-Result, borrowing the datagram: `0x0000` is success,
/// `0x0030` is Register-Foreign-Device NAK (J.2.1.1).
#[must_use]
pub fn result_code(datagram: &[u8]) -> Option<u16> {
    match Bvll::decode(datagram).ok()? {
        Bvll {
            length: 6,
            function: Function::Result(code),
        } if datagram.len() == 6 => Some(code),
        _ => None,
    }
}

/// The NAK a BBMD sends when it refuses a registration.
pub const REGISTER_NAK: u16 = result::REGISTER_FOREIGN_DEVICE_NAK;
