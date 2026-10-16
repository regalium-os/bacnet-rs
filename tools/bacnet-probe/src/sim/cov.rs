// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The simulator's COV subscriptions: an initial notification on subscribe
//! (§13.1.2), another whenever present-value changes, none after lapse.

use super::{DEVICE, Sim};
use crate::frame::{self, Delivery};
use crate::service::cov::{self, Notification, Subscribe};
use crate::service::{self, PropertyRef, unconfirmed};
use crate::tag::{ObjectId, Value};
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

pub(super) struct Sub {
    to: SocketAddr,
    process: u32,
    object: ObjectId,
    until: Instant,
    last: Option<Value>,
}

impl Sub {
    /// The object this subscription follows.
    pub(super) fn object(&self) -> ObjectId {
        self.object
    }
}

impl Sim {
    /// Adds, renews or (with no lifetime) cancels a subscription; returns
    /// the initial notification for a new or renewed one.
    pub(super) fn subscribe(&mut self, to: SocketAddr, s: Subscribe) -> Option<Vec<u8>> {
        self.subs
            .retain(|sub| !(sub.to == to && sub.process == s.process && sub.object == s.object));
        let lifetime = s.lifetime?;
        let last = self.present(s.object);
        let sub = Sub {
            to,
            process: s.process,
            object: s.object,
            until: Instant::now() + Duration::from_secs(lifetime.into()),
            last,
        };
        let first = self.notification(&sub);
        self.subs.push(sub);
        Some(first)
    }

    /// Drops lapsed subscriptions and notifies the rest of any change.
    pub(super) fn tick(&mut self, socket: &UdpSocket) -> io::Result<()> {
        let now = Instant::now();
        self.subs.retain(|s| s.until > now);
        let mut outgoing = Vec::new();
        for i in 0..self.subs.len() {
            let Some(sub) = self.subs.get(i) else {
                continue;
            };
            let current = self.present(sub.object);
            if current != sub.last {
                let to = sub.to;
                if let Some(sub) = self.subs.get_mut(i) {
                    sub.last = current;
                }
                if let Some(sub) = self.subs.get(i) {
                    outgoing.push((to, self.notification(sub)));
                }
            }
        }
        for (to, apdu) in outgoing {
            socket.send_to(&frame::encode(Delivery::Unicast, None, false, &apdu), to)?;
        }
        Ok(())
    }

    fn notification(&self, sub: &Sub) -> Vec<u8> {
        let flags = self
            .props
            .get(&(sub.object, 111))
            .cloned()
            .unwrap_or_default();
        let remaining = sub
            .until
            .saturating_duration_since(Instant::now())
            .as_secs();
        let n = Notification {
            process: sub.process,
            device: DEVICE,
            object: sub.object,
            time_remaining: u32::try_from(remaining).unwrap_or(u32::MAX),
            values: vec![
                (
                    PropertyRef::whole(85),
                    sub.last.clone().into_iter().collect(),
                ),
                (PropertyRef::whole(111), flags),
            ],
        };
        service::unconfirmed_request(unconfirmed::COV_NOTIFICATION, &cov::notification(&n))
    }
}
