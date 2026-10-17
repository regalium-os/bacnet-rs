// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Foreign device registration (Annex J.5): reaching broadcasts on a subnet
//! this host is not on, through that subnet's BBMD.

use super::Client;
use crate::frame::{Delivery, bvll};
use regalium_bacnet_telemetry::Level;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use std::{fmt, io};

pub(crate) struct Registration {
    bbmd: SocketAddr,
    ttl: u16,
    renew_at: Instant,
}

/// Why a registration failed.
#[derive(Debug)]
pub enum ForeignError {
    /// The socket failed.
    Io(io::Error),
    /// The BBMD did not answer.
    Timeout,
    /// The BBMD refused, with this BVLC-Result code (0x0030 is NAK: its
    /// foreign device table is full or it accepts none).
    Refused(u16),
}

impl fmt::Display for ForeignError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "i/o: {e}"),
            Self::Timeout => f.write_str("the BBMD did not answer"),
            Self::Refused(code) => write!(
                f,
                "the BBMD refused the registration (BVLC-Result 0x{code:04x})"
            ),
        }
    }
}

impl std::error::Error for ForeignError {}

impl Client {
    /// Registers with `bbmd` as a foreign device for `ttl` seconds. From then
    /// on broadcasts go to the BBMD as Distribute-Broadcast-To-Network, and
    /// the registration renews itself before it lapses, on any later call.
    ///
    /// # Errors
    ///
    /// When the BBMD does not answer or refuses.
    pub fn register_foreign_device(
        &mut self,
        bbmd: SocketAddr,
        ttl: u16,
    ) -> Result<(), ForeignError> {
        let span = self.enter("bacnet.client.registerForeignDevice");
        let result = self.register_untraced(bbmd, ttl);
        match &result {
            Ok(()) => self.log(
                Level::Info,
                &format_args!("registered with BBMD {bbmd} for {ttl} s"),
                &[],
            ),
            Err(e) => self.log(Level::Warn, &format_args!("BBMD {bbmd}: {e}"), &[]),
        }
        self.exit(
            span,
            result.as_ref().err().map(|e| e as &dyn std::fmt::Display),
        );
        result
    }

    fn register_untraced(&mut self, bbmd: SocketAddr, ttl: u16) -> Result<(), ForeignError> {
        let request = bvll::register_foreign_device(ttl);
        for _ in 0..=self.retries {
            self.send(&request, bbmd).map_err(ForeignError::Io)?;
            let deadline = Instant::now() + self.timeout;
            while let Some((datagram, peer)) =
                self.recv_until(deadline).map_err(ForeignError::Io)?
            {
                match bvll::result_code(&datagram) {
                    Some(0) if peer == bbmd => {
                        // Renew inside the 30 s grace the BBMD adds (J.5.2.3).
                        let renew = if ttl > 60 { ttl - 30 } else { ttl / 2 }.max(1);
                        let renew_at = Instant::now() + Duration::from_secs(renew.into());
                        self.foreign = Some(Registration {
                            bbmd,
                            ttl,
                            renew_at,
                        });
                        return Ok(());
                    }
                    Some(code) if peer == bbmd => return Err(ForeignError::Refused(code)),
                    _ => {}
                }
            }
        }
        Err(ForeignError::Timeout)
    }

    /// The BBMD this client is registered with, if any.
    #[must_use]
    pub fn bbmd(&self) -> Option<SocketAddr> {
        self.foreign.as_ref().map(|r| r.bbmd)
    }

    /// Renews the registration if it is due. Called by every other method,
    /// so a long-running caller needs no timer of its own.
    ///
    /// # Errors
    ///
    /// When the renewal fails; the old registration then lapses on the BBMD.
    pub fn maintain(&mut self) -> Result<(), ForeignError> {
        match &self.foreign {
            Some(r) if Instant::now() >= r.renew_at => {
                let (bbmd, ttl) = (r.bbmd, r.ttl);
                self.register_foreign_device(bbmd, ttl)
            }
            _ => Ok(()),
        }
    }

    /// Where a broadcast to `target` really goes: to the BBMD, when
    /// registered, since this host's own broadcasts reach no one there.
    pub(crate) fn broadcast_via(&self, target: SocketAddr) -> (SocketAddr, Delivery) {
        self.bbmd().map_or((target, Delivery::Broadcast), |bbmd| {
            (bbmd, Delivery::Distribute)
        })
    }
}
