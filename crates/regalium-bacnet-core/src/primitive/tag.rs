// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Application tag numbers (§20.2.1.4): the tag number of an application
//! element is its type.

/// NULL.
pub const NULL: u8 = 0;
/// BOOLEAN.
pub const BOOLEAN: u8 = 1;
/// Unsigned Integer.
pub const UNSIGNED: u8 = 2;
/// Signed Integer.
pub const SIGNED: u8 = 3;
/// REAL.
pub const REAL: u8 = 4;
/// Double.
pub const DOUBLE: u8 = 5;
/// Octet String.
pub const OCTET_STRING: u8 = 6;
/// Character String.
pub const CHARACTER_STRING: u8 = 7;
/// Bit String.
pub const BIT_STRING: u8 = 8;
/// Enumerated.
pub const ENUMERATED: u8 = 9;
/// Date.
pub const DATE: u8 = 10;
/// Time.
pub const TIME: u8 = 11;
/// `BACnetObjectIdentifier`.
pub const OBJECT_IDENTIFIER: u8 = 12;
