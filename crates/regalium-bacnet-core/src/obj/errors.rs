// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The errors a database answers with (§18), by class and code.

use crate::service::ServiceError;

/// object, unknown-object (31): no such object.
pub const UNKNOWN_OBJECT: ServiceError = ServiceError { class: 1, code: 31 };
/// property, unknown-property (32): the object has no such property.
pub const UNKNOWN_PROPERTY: ServiceError = ServiceError { class: 2, code: 32 };
/// property, write-access-denied (40): it exists and is not writable.
pub const WRITE_ACCESS_DENIED: ServiceError = ServiceError { class: 2, code: 40 };
/// property, invalid-array-index (42): an index past the array, or into a
/// property that is not one.
pub const INVALID_ARRAY_INDEX: ServiceError = ServiceError { class: 2, code: 42 };
/// property, value-out-of-range (37): the right type, an impossible value —
/// a priority of 17.
pub const VALUE_OUT_OF_RANGE: ServiceError = ServiceError { class: 2, code: 37 };
/// property, invalid-data-type (9): the wrong type for the property.
pub const INVALID_DATA_TYPE: ServiceError = ServiceError { class: 2, code: 9 };
