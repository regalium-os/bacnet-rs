// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Seeking devices that went quiet: a Who-Is for each one's instance, with
//! a backoff that doubles on every miss, so a device that is gone costs a
//! datagram every few minutes rather than every few seconds.

use crate::client::Client;
use regalium_bacnet_core::datalink::{Datalink, Recipient};
use regalium_bacnet_core::npdu::{Address, Body, Destination, GLOBAL, Npdu};
use regalium_bacnet_core::service::{WhoIs, unconfirmed};
use std::time::Duration;

/// The longest wait between two attempts.
const CEILING: Duration = Duration::from_secs(300);

impl<D: Datalink> Client<D> {
    /// Asks again for every quiet device whose time has come. Fire and
    /// forget: the I-Am, when it comes, binds the device wherever it now is
    /// and reports it reachable.
    pub(crate) fn rebind_due(&mut self) -> Result<(), D::Error> {
        let now = self.now();
        let due: Vec<u32> = self
            .rebinding
            .iter()
            .filter(|(_, (at, _))| *at <= now)
            .map(|(i, _)| *i)
            .collect();
        for instance in due {
            let mut apdu = vec![0x10, unconfirmed::WHO_IS];
            WhoIs {
                range: Some((instance, instance)),
            }
            .encode(&mut apdu);
            let npdu = Npdu {
                control: 0,
                destination: Some(Destination {
                    address: Address {
                        network: GLOBAL,
                        mac: &[],
                    },
                    hop_count: 255,
                }),
                source: None,
                body: Body::Apdu(&apdu),
            };
            let mut out = Vec::new();
            npdu.encode(&mut out);
            self.link.send(Recipient::LocalBroadcast, &out)?;
            if let Some((at, wait)) = self.rebinding.get_mut(&instance) {
                *wait = wait.saturating_mul(2).min(CEILING);
                *at = now.saturating_add(*wait);
            }
        }
        Ok(())
    }

    /// When the next device is due to be sought.
    pub(crate) fn rebind_deadline(&self) -> Option<Duration> {
        self.rebinding.values().map(|(at, _)| *at).min()
    }
}
