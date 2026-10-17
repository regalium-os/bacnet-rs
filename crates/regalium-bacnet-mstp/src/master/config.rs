// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! A master's configuration: its station address and the timers of
//! §9.5.3, defaulted to the standard's values.

use core::time::Duration;

/// A master's configuration.
///
/// `Master::new(station)` uses the standard's defaults: stations up to 127,
/// one frame per token, `Tno_token` 500 ms, `Treply_timeout` 255 ms,
/// `Treply_delay` 250 ms, `Tslot` 10 ms, and a usage timeout of 50 ms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    pub(crate) station: u8,
    pub(crate) max_master: u8,
    pub(crate) max_info_frames: u8,
    pub(crate) no_token: Duration,
    pub(crate) reply_timeout: Duration,
    pub(crate) reply_delay: Duration,
    pub(crate) slot: Duration,
    pub(crate) usage: Duration,
}

impl Config {
    /// Station `station`'s configuration, the standard's defaults otherwise.
    #[must_use]
    pub const fn new(station: u8) -> Self {
        Self {
            station,
            max_master: 127,
            max_info_frames: 1,
            no_token: Duration::from_millis(500),
            reply_timeout: Duration::from_millis(255),
            reply_delay: Duration::from_millis(250),
            slot: Duration::from_millis(10),
            usage: Duration::from_millis(50),
        }
    }

    /// The highest station address polled for (`Nmax_master`).
    #[must_use]
    pub const fn max_master(mut self, n: u8) -> Self {
        self.max_master = if n > 127 { 127 } else { n };
        self
    }

    /// Frames sent per token (`Nmax_info_frames`), at least one.
    #[must_use]
    pub const fn max_info_frames(mut self, n: u8) -> Self {
        self.max_info_frames = if n == 0 { 1 } else { n };
        self
    }
}
