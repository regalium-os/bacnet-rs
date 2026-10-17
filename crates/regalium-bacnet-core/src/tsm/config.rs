// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! What a transaction is configured by: this device's timers and limits,
//! what the peer said it can take, and the request itself.

use crate::apdu::{Limits, Segments};
use core::time::Duration;
use regalium_bacnet_telemetry::SpanId;

/// This device's side of every transaction: the Device object's
/// `APDU_Timeout`, `APDU_Segment_Timeout` and `Number_Of_APDU_Retries`
/// (§12.11), and what it will accept back.
///
/// `Default` is the values ASHRAE 135-2016 recommends — 3 s, 2 s, 3
/// retries — accepting answers of any number of 1476-octet segments, in
/// windows of up to 16. Every field has a builder method; adding one never
/// breaks a caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    pub(crate) apdu_timeout: Duration,
    pub(crate) segment_timeout: Duration,
    pub(crate) retries: u8,
    pub(crate) window: u8,
    pub(crate) accept: Limits,
    pub(crate) segmented_response: bool,
    pub(crate) max_reassembly: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            apdu_timeout: Duration::from_secs(3),
            segment_timeout: Duration::from_secs(2),
            retries: 3,
            window: 16,
            accept: Limits::new(Segments::MoreThan64, 5),
            segmented_response: true,
            max_reassembly: 64 * 1024,
        }
    }
}

impl Config {
    /// How long to wait for an answer before sending again (§5.4.1, `T_out`).
    #[must_use]
    pub const fn apdu_timeout(mut self, t: Duration) -> Self {
        self.apdu_timeout = t;
        self
    }

    /// How long to wait for a Segment-ACK or the next segment (`T_seg`). A
    /// segmented answer is abandoned after four of these in silence, as
    /// §5.4.4.4 says: the sender has its own retries to spend first.
    #[must_use]
    pub const fn segment_timeout(mut self, t: Duration) -> Self {
        self.segment_timeout = t;
        self
    }

    /// How many times to send again before abandoning (`N_retry`). The
    /// first send is not a retry: 3 retries is 4 sends.
    #[must_use]
    pub const fn retries(mut self, n: u8) -> Self {
        self.retries = n;
        self
    }

    /// The window proposed when sending segments, and the most accepted
    /// when receiving them; 1–127 (§20.1.2.8). Out-of-range values clamp.
    #[must_use]
    pub const fn window(mut self, n: u8) -> Self {
        self.window = if n == 0 {
            1
        } else if n > 127 {
            127
        } else {
            n
        };
        self
    }

    /// What every request advertises it can take back.
    #[must_use]
    pub const fn accept(mut self, limits: Limits) -> Self {
        self.accept = limits;
        self
    }

    /// Whether to accept a segmented answer at all (the `SA` bit). Off,
    /// a segmented answer is aborted as segmentation-not-supported.
    #[must_use]
    pub const fn segmented_response(mut self, on: bool) -> Self {
        self.segmented_response = on;
        self
    }

    /// The most octets a segmented answer may reassemble to before it is
    /// aborted as a buffer overflow. A bound on memory a peer can claim.
    #[must_use]
    pub const fn max_reassembly(mut self, octets: usize) -> Self {
        self.max_reassembly = octets;
        self
    }
}

/// What the peer said it can take, from its I-Am or Device object.
///
/// `Default` is a device nothing is known about: 50-octet APDUs, the
/// smallest any device accepts, and no segmentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Remote {
    /// Max-APDU-length-accepted, in octets. The caller should already have
    /// lowered it to what the datalink between the two can carry.
    pub max_apdu: u16,
    /// Whether it reassembles segmented requests (segmentation `both` or
    /// `receive`).
    pub segmented_requests: bool,
    /// How many segments it reassembles, when it said.
    pub max_segments: Segments,
}

impl Default for Remote {
    fn default() -> Self {
        Self {
            max_apdu: 50,
            segmented_requests: false,
            max_segments: Segments::Unspecified,
        }
    }
}

/// One confirmed request, before it has an invoke id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub(crate) service: u8,
    pub(crate) body: Vec<u8>,
    pub(crate) remote: Remote,
    pub(crate) parent: SpanId,
}

impl Request {
    /// A request for confirmed `service` (§21) with its encoded `body`,
    /// owned so it can be sent again without the caller keeping it.
    #[must_use]
    pub const fn new(service: u8, body: Vec<u8>) -> Self {
        Self {
            service,
            body,
            remote: Remote {
                max_apdu: 50,
                segmented_requests: false,
                max_segments: Segments::Unspecified,
            },
            parent: SpanId::NONE,
        }
    }

    /// What the peer can take; [`Remote::default`] when not given.
    #[must_use]
    pub const fn remote(mut self, remote: Remote) -> Self {
        self.remote = remote;
        self
    }

    /// The span the transaction's span is a child of.
    #[must_use]
    pub const fn within(mut self, parent: SpanId) -> Self {
        self.parent = parent;
        self
    }
}
