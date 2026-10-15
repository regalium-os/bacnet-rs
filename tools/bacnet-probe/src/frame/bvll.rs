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

use super::BVLC_TYPE;

const BVLC_RESULT: u8 = 0x00;
const REGISTER_FOREIGN_DEVICE: u8 = 0x05;

/// Register-Foreign-Device asking for `ttl` seconds.
#[must_use]
pub fn register_foreign_device(ttl: u16) -> Vec<u8> {
    let [hi, lo] = ttl.to_be_bytes();
    vec![BVLC_TYPE, REGISTER_FOREIGN_DEVICE, 0x00, 0x06, hi, lo]
}

/// The TTL a Register-Foreign-Device asks for, borrowing the datagram, or
/// `None` if it is something else.
#[must_use]
pub fn registration(datagram: &[u8]) -> Option<u16> {
    match datagram {
        [BVLC_TYPE, REGISTER_FOREIGN_DEVICE, 0x00, 0x06, hi, lo] => {
            Some(u16::from_be_bytes([*hi, *lo]))
        }
        _ => None,
    }
}

/// A BVLC-Result carrying `code`.
#[must_use]
pub fn result(code: u16) -> Vec<u8> {
    let [hi, lo] = code.to_be_bytes();
    vec![BVLC_TYPE, BVLC_RESULT, 0x00, 0x06, hi, lo]
}

/// The code of a BVLC-Result, borrowing the datagram: `0x0000` is success,
/// `0x0030` is Register-Foreign-Device NAK (J.2.1.1).
#[must_use]
pub fn result_code(datagram: &[u8]) -> Option<u16> {
    match datagram {
        [BVLC_TYPE, BVLC_RESULT, 0x00, 0x06, hi, lo] => Some(u16::from_be_bytes([*hi, *lo])),
        _ => None,
    }
}

/// The NAK a BBMD sends when it refuses a registration.
pub const REGISTER_NAK: u16 = 0x0030;
