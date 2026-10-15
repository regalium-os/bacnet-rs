// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The socket itself: every datagram in or out passes here, so this is
//! where the trace sees it and where packet and byte counts are recorded.

use super::{Client, Direction};
use crate::metrics;
use regalium_bacnet_telemetry::{Attribute, keys};
use std::io;
use std::net::SocketAddr;
use std::time::Instant;

impl Client {
    pub(super) fn send(&self, packet: &[u8], to: SocketAddr) -> io::Result<()> {
        if let Some(trace) = &self.trace {
            trace(Direction::Sent, to, packet);
        }
        self.count(Direction::Sent, packet.len());
        self.socket.send_to(packet, to).map(drop)
    }

    /// One datagram, or `None` once `deadline` passes.
    pub(super) fn recv_until(
        &self,
        deadline: Instant,
    ) -> io::Result<Option<(Vec<u8>, SocketAddr)>> {
        let Some(left) = deadline
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
        else {
            return Ok(None);
        };
        self.socket.set_read_timeout(Some(left))?;
        let mut buf = vec![0u8; 1500];
        match self.socket.recv_from(&mut buf) {
            Ok((n, peer)) => {
                buf.truncate(n);
                if let Some(trace) = &self.trace {
                    trace(Direction::Received, peer, &buf);
                }
                self.count(Direction::Received, n);
                Ok(Some((buf, peer)))
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    fn count(&self, direction: Direction, bytes: usize) {
        let labels = [Attribute::str(keys::DIRECTION, direction.label())];
        self.meter.record(&metrics::PACKETS, 1.0, &labels);
        #[allow(clippy::cast_precision_loss)] // A datagram is under 64 KiB.
        self.meter.record(&metrics::BYTES, bytes as f64, &labels);
    }
}
