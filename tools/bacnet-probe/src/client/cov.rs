// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! COV subscriptions, and collecting the notifications they produce.

use super::{Client, Device, ReadError};
use crate::frame;
use crate::service::confirmed;
use crate::service::cov::{self, Notification, Subscribe};
use crate::tag::ObjectId;
use std::io;
use std::time::{Duration, Instant};

impl Client {
    /// Subscribes to unconfirmed COV notifications from `object` for
    /// `lifetime` seconds; returns the subscriber process id that its
    /// notifications will carry. Renew with [`Client::renew_cov`] before the
    /// lifetime runs out.
    ///
    /// # Errors
    ///
    /// When the device refuses; [`ReadError::unsupported`] or an
    /// object / optional-functionality-not-supported error means poll instead.
    pub fn subscribe_cov(
        &mut self,
        device: &Device,
        object: ObjectId,
        lifetime: u32,
    ) -> Result<u32, ReadError> {
        let process = self.next_process;
        self.next_process = self.next_process.wrapping_add(1).max(1);
        self.renew_cov(device, object, process, lifetime)?;
        Ok(process)
    }

    /// Renews (or re-creates) subscription `process`.
    ///
    /// # Errors
    ///
    /// As for [`Client::subscribe_cov`].
    pub fn renew_cov(
        &mut self,
        device: &Device,
        object: ObjectId,
        process: u32,
        lifetime: u32,
    ) -> Result<(), ReadError> {
        let request = Subscribe {
            process,
            object,
            confirmed: Some(false),
            lifetime: Some(lifetime),
        };
        self.confirm(device, confirmed::SUBSCRIBE_COV, &cov::subscribe(&request))
            .map(drop)
    }

    /// Cancels subscription `process`, so the device stops sending.
    ///
    /// # Errors
    ///
    /// When the device does not acknowledge; the subscription then lapses at
    /// its lifetime regardless.
    pub fn cancel_cov(
        &mut self,
        device: &Device,
        object: ObjectId,
        process: u32,
    ) -> Result<(), ReadError> {
        let request = Subscribe {
            process,
            object,
            confirmed: None,
            lifetime: None,
        };
        self.confirm(device, confirmed::SUBSCRIBE_COV, &cov::subscribe(&request))
            .map(drop)
    }

    /// Every COV notification received so far, waiting up to `wait` for the
    /// first if none is queued. Returns early once one arrives.
    ///
    /// # Errors
    ///
    /// When the socket fails.
    pub fn notifications(&mut self, wait: Duration) -> io::Result<Vec<Notification>> {
        let _ = self.maintain();
        let deadline = Instant::now() + wait;
        while self.pending.is_empty() {
            let Some((datagram, peer)) = self.recv_until(deadline)? else {
                break;
            };
            if let Some(msg) = frame::decode(&datagram, peer) {
                self.unsolicited(&msg)?;
            }
        }
        Ok(self.pending.drain(..).collect())
    }
}
