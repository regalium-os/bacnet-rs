// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Sending: a broadcast to the group; a unicast to the address the table
//! knows, or held while it is resolved.

use super::{BacnetIpv6, HELD};
use crate::bvll::{Bvll6, Function6, Vmac};
use crate::vmac_table::Resolve;
use regalium_bacnet_core::datalink::{Datalink, Recipient};
use regalium_bacnet_telemetry::Span;
use std::io;
use std::time::{Duration, Instant};

impl Datalink for BacnetIpv6 {
    type Mac = Vmac;
    type Error = io::Error;

    fn send(&self, to: Recipient<Vmac>, npdu: &[u8]) -> io::Result<()> {
        let own = self.own();
        let Recipient::Unicast(vmac) = to else {
            return self.transmit(
                &Bvll6::to_vec(own, Function6::OriginalBroadcast(npdu)),
                self.group,
            );
        };
        let now = self.now();
        let resolved = {
            let mut state = self.lock();
            let r = state.table.resolve(vmac, now);
            if !matches!(r, Resolve::Known(_)) {
                let held = state.held.entry(vmac).or_default();
                if held.len() >= HELD {
                    held.remove(0);
                }
                held.push(npdu.to_vec());
            }
            r
        };
        match resolved {
            Resolve::Known(a) => {
                let unicast = Function6::OriginalUnicast {
                    destination: vmac,
                    npdu,
                };
                self.transmit(&Bvll6::to_vec(own, unicast), a)
            }
            Resolve::Ask(ask) => {
                let _span = Span::start(&*self.tracer, "bacnet.ipv6.vmac.resolve");
                self.transmit(&ask, self.group)
            }
            Resolve::Waiting => Ok(()),
        }
    }

    fn receive(
        &self,
        buf: &mut [u8],
        timeout: Duration,
    ) -> io::Result<Option<regalium_bacnet_core::datalink::Received<Vmac>>> {
        self.receive_until(buf, Instant::now() + timeout)
    }

    fn max_apdu(&self) -> u16 {
        1476
    }
}
