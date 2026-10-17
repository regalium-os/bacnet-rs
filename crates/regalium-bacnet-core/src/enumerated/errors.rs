// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `BACnetErrorClass` and `BACnetErrorCode` (§18, §21): the class by number,
//! and the codes a client meets most, sorted by number.

/// Error classes, entry N is class N.
pub(crate) const CLASSES: &[&str] = &[
    "device",
    "object",
    "property",
    "resources",
    "security",
    "services",
    "vt",
    "communication",
];

/// `(number, name)` of the common error codes, sorted by number.
pub(crate) const CODES: &[(u32, &str)] = &[
    (0, "other"),
    (2, "configuration-in-progress"),
    (3, "device-busy"),
    (4, "dynamic-creation-not-supported"),
    (9, "invalid-data-type"),
    (23, "object-deletion-not-permitted"),
    (24, "object-identifier-already-exists"),
    (25, "operational-problem"),
    (27, "read-access-denied"),
    (31, "unknown-object"),
    (32, "unknown-property"),
    (36, "unsupported-object-type"),
    (37, "value-out-of-range"),
    (40, "write-access-denied"),
    (42, "invalid-array-index"),
    (45, "optional-functionality-not-supported"),
    (50, "property-is-not-an-array"),
];
