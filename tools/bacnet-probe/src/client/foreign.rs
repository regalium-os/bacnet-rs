// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Foreign device registration (Annex J.5): reaching broadcasts on a subnet
//! this host is not on, through that subnet's BBMD.

use super::Client;
use crate::frame::{Delivery, bvll};
use regalium_bacnet::ip::foreign::{Outcome, Registration};
use regalium_bacnet_telemetry::Level;
use std::net::SocketAddr;
use std::{fmt, io};

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

    /// Drives the driver's registration state machine until the BBMD
    /// answers, or `retries` resends go unanswered.
    fn register_untraced(&mut self, bbmd: SocketAddr, ttl: u16) -> Result<(), ForeignError> {
        let SocketAddr::V4(v4) = bbmd else {
            return Err(ForeignError::Io(io::ErrorKind::InvalidInput.into()));
        };
        let mut registration = Registration::new(v4, ttl).retry_after(self.timeout);
        let mut sends = 0;
        loop {
            if let Some(request) = registration.poll(self.clock()) {
                if sends > self.retries {
                    return Err(ForeignError::Timeout);
                }
                sends += 1;
                self.send(&request, bbmd).map_err(ForeignError::Io)?;
            }
            let deadline = self.origin + registration.deadline();
            while let Some((datagram, peer)) =
                self.recv_until(deadline).map_err(ForeignError::Io)?
            {
                let (Some(code), SocketAddr::V4(from)) = (bvll::result_code(&datagram), peer)
                else {
                    continue;
                };
                match registration.result(from, code, self.clock()) {
                    Some(Outcome::Registered { .. }) => {
                        self.foreign = Some(registration);
                        return Ok(());
                    }
                    Some(Outcome::Refused(code)) => return Err(ForeignError::Refused(code)),
                    None => {}
                }
            }
        }
    }

    /// The BBMD this client is registered with, if any.
    #[must_use]
    pub fn bbmd(&self) -> Option<SocketAddr> {
        self.foreign.as_ref().map(|r| SocketAddr::V4(r.bbmd()))
    }

    /// Renews the registration if it is due. Called by every other method,
    /// so a long-running caller needs no timer of its own.
    ///
    /// # Errors
    ///
    /// When the renewal fails; the old registration then lapses on the BBMD.
    pub fn maintain(&mut self) -> Result<(), ForeignError> {
        match &self.foreign {
            Some(r) if self.clock() >= r.deadline() => {
                let (bbmd, ttl) = (r.bbmd(), r.ttl());
                self.register_foreign_device(SocketAddr::V4(bbmd), ttl)
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
