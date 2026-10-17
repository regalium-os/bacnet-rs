// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Why a confirmed request produced nothing. Matched, never string-compared.

use std::{fmt, io};

/// The ways a confirmed request fails.
#[derive(Debug)]
pub enum ReadError {
    /// The socket failed.
    Io(io::Error),
    /// No reply within the timeout, after every retry. For a device that has
    /// gone quiet this is the expected answer, not a fault in the probe.
    Timeout,
    /// An Error-PDU (§18): error class and code, e.g. property /
    /// unknown-property.
    Bacnet {
        /// `BACnetErrorClass`.
        class: u32,
        /// `BACnetErrorCode`.
        code: u32,
    },
    /// A Reject-PDU (§18.8): the request was malformed or unsupported,
    /// e.g. reason 9, unrecognized-service.
    Reject(u8),
    /// An Abort-PDU (§18.9), e.g. reason 4, segmentation-not-supported.
    Abort(u8),
    /// The request needs segmentation the device cannot take, so it was
    /// never sent. A segmented answer is reassembled, not refused.
    Segmented,
    /// The reply decoded to nothing the grammar allows.
    Malformed,
}

impl ReadError {
    /// The `bacnet.outcome` label for this failure.
    #[must_use]
    pub fn outcome(&self) -> &'static str {
        match self {
            Self::Io(_) => "io",
            Self::Timeout => "timeout",
            Self::Bacnet { .. } => "error",
            Self::Reject(_) => "reject",
            Self::Abort(_) => "abort",
            Self::Segmented => "segmented",
            Self::Malformed => "malformed",
        }
    }

    /// Whether this says the device does not implement the service at all,
    /// as opposed to failing this one request. A caller falls back to a
    /// simpler service on these.
    #[must_use]
    pub fn unsupported(&self) -> bool {
        matches!(
            self,
            // unrecognized-service; segmentation-not-supported, buffer-overflow
            Self::Reject(9) | Self::Abort(1 | 4) | Self::Segmented
                // services: service-request-denied, optional-functionality-not-supported
                | Self::Bacnet { class: 5, code: 29 | 45 }
        )
    }
}

impl From<io::Error> for ReadError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "i/o: {e}"),
            Self::Timeout => f.write_str("no reply"),
            Self::Bacnet { class, code } => f.write_str(&crate::names::error(*class, *code)),
            Self::Reject(r) => write!(f, "rejected (reason {r})"),
            Self::Abort(r) => write!(f, "aborted (reason {r})"),
            Self::Segmented => f.write_str("reply needs segmentation (unsupported)"),
            Self::Malformed => f.write_str("malformed reply"),
        }
    }
}

impl std::error::Error for ReadError {}
