// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! How a call fails, as a status code a transport can carry unchanged, with
//! the device's own error class and code kept beside it.
//!
//! The code says what the caller should do (fix the request, wait, give
//! up); the `BACnet` pair says what the device actually said, for a person
//! reading the log. Neither is recovered from a message string.

use crate::ReadError;
use std::fmt;

/// The canonical status codes this API returns, numbered as `google.rpc.Code`
/// so a transport maps them without a table (AIP-193).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Code {
    /// The request itself is wrong: a malformed name, a value of the wrong
    /// type, a state out of range.
    InvalidArgument = 3,
    /// The device, object or property does not exist.
    NotFound = 5,
    /// `CreateObject` named an object that already exists.
    AlreadyExists = 6,
    /// The device refused in its present state: a read-only property, an
    /// input that is in service, an object it will not delete.
    FailedPrecondition = 9,
    /// The device does not implement the service or the object type, or
    /// this API does not implement the request field yet.
    Unimplemented = 12,
    /// The device sent something no grammar allows.
    Internal = 13,
    /// The device did not answer. Often transient: it may be rebooting.
    Unavailable = 14,
}

/// A failed call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiError {
    /// What kind of failure.
    pub code: Code,
    /// For a person: what failed and why.
    pub message: String,
    /// The device's (error-class, error-code), when it answered with one.
    pub bacnet: Option<(u32, u32)>,
}

impl ApiError {
    /// A failure the device had no part in.
    #[must_use]
    pub fn new(code: Code, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            bacnet: None,
        }
    }

    /// A malformed request.
    #[must_use]
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(Code::InvalidArgument, message)
    }

    /// What `context` was doing when `e` happened.
    #[must_use]
    pub fn from_read(context: &str, e: &ReadError) -> Self {
        let (code, bacnet) = match e {
            ReadError::Timeout | ReadError::Io(_) | ReadError::Abort(_) | ReadError::Segmented => {
                (Code::Unavailable, None)
            }
            ReadError::Bacnet { class, code } => {
                (bacnet_code(*class, *code), Some((*class, *code)))
            }
            ReadError::Reject(9) => (Code::Unimplemented, None), // unrecognized-service
            ReadError::Reject(_) | ReadError::Malformed => (Code::Internal, None),
        };
        Self {
            code,
            message: format!("{context}: {e}"),
            bacnet,
        }
    }
}

/// The status a `BACnet` error maps to, by what the caller can do about it.
fn bacnet_code(class: u32, code: u32) -> Code {
    match (class, code) {
        (_, 31 | 32) => Code::NotFound, // unknown-object, unknown-property
        (_, 24) => Code::AlreadyExists, // object-identifier-already-exists
        // invalid-data-type, value-out-of-range, invalid-array-index,
        // property-is-not-an-array
        (_, 9 | 37 | 42 | 50) => Code::InvalidArgument,
        // dynamic-creation-not-supported, unsupported-object-type,
        // optional-functionality-not-supported, and the services class
        (_, 4 | 36 | 45) | (5, _) => Code::Unimplemented,
        // write-access-denied, object-deletion-not-permitted, device-busy,
        // configuration-in-progress, and anything else the device refused
        _ => Code::FailedPrecondition,
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for ApiError {}
