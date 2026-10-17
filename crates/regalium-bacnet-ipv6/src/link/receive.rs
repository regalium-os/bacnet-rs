// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Receiving: learn from every frame, answer resolutions, send what was held
//! for a station once its address is known, and hand up only NPDUs.

use super::BacnetIpv6;
use crate::bvll::{Bvll6, Function6, Vmac};
use regalium_bacnet_core::datalink::Received;
use regalium_bacnet_telemetry::{Attribute, Span, keys};
use std::io::{self, ErrorKind};
use std::net::{SocketAddr, SocketAddrV6};
use std::time::{Duration, Instant};

/// Larger than any BACnet/IPv6 datagram.
const DATAGRAM: usize = 1600;

impl BacnetIpv6 {
    pub(super) fn receive_until(
        &self,
        buf: &mut [u8],
        deadline: Instant,
    ) -> io::Result<Option<Received<Vmac>>> {
        let mut datagram = [0u8; DATAGRAM];
        loop {
            self.resolve_due()?;
            let now = Instant::now();
            if now >= deadline {
                return Ok(None);
            }
            let mut wait = deadline - now;
            if let Some(due) = self.lock().table.deadline() {
                wait = wait.min(due.saturating_sub(self.now()));
            }
            self.socket
                .set_read_timeout(Some(wait.max(Duration::from_millis(1))))?;
            let (len, from) = match self.socket.recv_from(&mut datagram) {
                Ok(got) => got,
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                    continue;
                }
                Err(e) => return Err(e),
            };
            let SocketAddr::V6(from) = from else { continue };
            if let Some(got) = self.dispatch(from, datagram.get(..len).unwrap_or(&[]), buf)? {
                return Ok(Some(got));
            }
        }
    }

    /// Resolutions to ask again, and frames to drop for those given up.
    fn resolve_due(&self) -> io::Result<()> {
        let now = self.now();
        let again = {
            let mut state = self.lock();
            let (again, gone) = state.table.poll(now);
            for v in gone {
                state.held.remove(&v);
            }
            again
        };
        for ask in again {
            self.transmit(&ask, self.group)?;
        }
        Ok(())
    }

    fn dispatch(
        &self,
        from: SocketAddrV6,
        octets: &[u8],
        buf: &mut [u8],
    ) -> io::Result<Option<Received<Vmac>>> {
        let Ok(b) = Bvll6::decode(octets) else {
            return Ok(None);
        };
        if usize::from(b.length) != octets.len() {
            return Ok(None);
        }
        let own = self.own();
        let Some(source) = b.source else {
            return Ok(None);
        };
        if source == own {
            return Ok(None); // Its own multicast, heard back.
        }
        let span = Span::start(&*self.tracer, "bacnet.ipv6.receive");
        span.set(Attribute::int(
            keys::PDU_OCTETS,
            i64::try_from(octets.len()).unwrap_or(i64::MAX),
        ));
        // A forwarded frame's sender is the station it names, not the BBMD.
        let address = match b.function {
            Function6::Forwarded { origin, .. } => origin,
            _ => from,
        };
        self.learn(source, address)?;
        let delivered = match b.function {
            Function6::AddressResolution(target) if target == own => {
                self.transmit(
                    &Bvll6::to_vec(own, Function6::AddressResolutionAck(source)),
                    from,
                )?;
                None
            }
            Function6::VirtualAddressResolution => {
                self.transmit(
                    &Bvll6::to_vec(own, Function6::VirtualAddressResolutionAck(source)),
                    from,
                )?;
                None
            }
            Function6::OriginalUnicast { destination, npdu } if destination == own => {
                Some((npdu, false))
            }
            Function6::OriginalBroadcast(npdu) | Function6::Forwarded { npdu, .. } => {
                Some((npdu, true))
            }
            _ => None,
        };
        let Some((npdu, broadcast)) = delivered else {
            return Ok(None);
        };
        let Some(into) = buf.get_mut(..npdu.len()) else {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "receive buffer smaller than the NPDU",
            ));
        };
        into.copy_from_slice(npdu);
        Ok(Some(Received {
            from: source,
            len: npdu.len(),
            broadcast,
        }))
    }

    /// Learns `source` at `address`, keeping any duplicate, and sends what
    /// was held for it now that it is known.
    fn learn(&self, source: Vmac, address: SocketAddrV6) -> io::Result<()> {
        let now = self.now();
        let (own, held) = {
            let mut state = self.lock();
            if let Some(d) = state.table.learn(source, address, now) {
                state.duplicates.push(d);
            }
            // Where the table says it is: after a duplicate, the address it
            // kept, not the one that also claimed it.
            let known = state.table.lookup(source, now);
            (state.table.own(), known.zip(state.held.remove(&source)))
        };
        if let Some((to, frames)) = held {
            for npdu in frames {
                let unicast = Function6::OriginalUnicast {
                    destination: source,
                    npdu: &npdu,
                };
                self.transmit(&Bvll6::to_vec(own, unicast), to)?;
            }
        }
        Ok(())
    }
}
