// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The loop that waits: poll the transaction state machine, send what it
//! says, receive until its next deadline, and hand back what arrives.

use crate::client::Client;
use crate::device::{Device, Event, Route};
use regalium_bacnet_core::apdu::{Apdu, Pdu};
use regalium_bacnet_core::datalink::{Datalink, Recipient};
use regalium_bacnet_core::npdu::{Address, Body, Destination, Npdu};
use regalium_bacnet_core::service::{IAm, unconfirmed};
use regalium_bacnet_core::tsm::Step;
use std::time::Duration;

/// Larger than any NPDU a datalink carries.
const BUFFER: usize = 2048;

impl<D: Datalink> Client<D> {
    /// Drives everything in flight until `done` holds or `deadline` passes.
    ///
    /// # Errors
    ///
    /// Only the datalink's: a device that does not answer is a step, not an
    /// error.
    pub(crate) fn pump(
        &mut self,
        deadline: Duration,
        mut done: impl FnMut(&Self) -> bool,
    ) -> Result<(), D::Error> {
        let mut buf = vec![0u8; BUFFER];
        loop {
            if done(self) {
                return Ok(());
            }
            let now = self.now();
            match self.tsm.poll(now) {
                Step::Transmit { peer, apdu, .. } => self.transmit(peer, &apdu)?,
                Step::Complete {
                    peer,
                    invoke,
                    reply,
                } => {
                    self.reached(peer);
                    self.done.insert((peer, invoke), Ok(reply));
                }
                Step::Abandon {
                    peer,
                    invoke,
                    reason,
                } => {
                    self.lost(peer, reason);
                    self.done.insert((peer, invoke), Err(reason));
                }
                Step::Idle { wake_at } => {
                    if now >= deadline {
                        return Ok(());
                    }
                    let until = wake_at.map_or(deadline, |w| w.min(deadline));
                    if let Some(got) = self.link.receive(&mut buf, until.saturating_sub(now))? {
                        let octets = buf.get(..got.len).unwrap_or(&[]);
                        self.dispatch(octets, got.from);
                    }
                }
            }
        }
    }

    /// Wraps an APDU for `instance` in an NPDU — routed when the device is
    /// behind a router — and sends it.
    fn transmit(&self, instance: u32, apdu: &[u8]) -> Result<(), D::Error> {
        let Some(device) = self.devices.get(&instance) else {
            return Ok(());
        };
        let destination = device.route.as_ref().map(|r| Destination {
            address: Address {
                network: r.network,
                mac: &r.mac,
            },
            hop_count: 255,
        });
        // A confirmed request expects a reply (§6.2.2); a Segment-ACK or
        // Abort does not.
        let confirmed = apdu.first().is_some_and(|o| o >> 4 == 0);
        let npdu = Npdu {
            control: if confirmed { 0x04 } else { 0 },
            destination,
            source: None,
            body: Body::Apdu(apdu),
        };
        let mut out = Vec::new();
        npdu.encode(&mut out);
        self.link.send(Recipient::Unicast(device.address), &out)
    }

    /// One NPDU: an I-Am is learnt from; anything else goes to the
    /// transaction it answers. What fits neither is dropped.
    fn dispatch(&mut self, octets: &[u8], from: D::Mac) {
        let Ok(npdu) = Npdu::decode(octets) else {
            return;
        };
        let Body::Apdu(body) = npdu.body else { return };
        let Ok(apdu) = Apdu::decode(body) else { return };
        let route = npdu.source.map(|s| Route {
            network: s.network,
            mac: s.mac.to_vec(),
        });
        if let Pdu::UnconfirmedRequest {
            service: unconfirmed::I_AM,
            body,
        } = apdu.pdu
        {
            if let Ok(i) = IAm::decode(body) {
                self.learn(&i, from, route);
            }
            return;
        }
        if self.notified(from, route.as_ref(), &apdu) || self.alarmed(from, route.as_ref(), &apdu) {
            return;
        }
        if let Some(instance) = self.find(&from, route.as_ref()) {
            let now = self.now();
            self.tsm.receive(instance, &apdu, now);
        }
    }

    /// Binds the device an I-Am announces, at the address it came from.
    fn learn(&mut self, i: &IAm, from: D::Mac, route: Option<Route>) {
        let instance = i.device.instance;
        let device = Device {
            instance,
            address: from,
            route,
            max_apdu: i.max_apdu,
            segmentation: i.segmentation,
            vendor_id: i.vendor_id,
        };
        if self.devices.insert(instance, device).is_none() {
            self.event(Event::Discovered(instance));
        }
        self.heard.insert(instance);
        self.reached(instance);
    }
}
