// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Change of value (§13.1, §13.14): subscribe once, and the device sends a
//! notification whenever present-value moves by more than its
//! `cov-increment` or its status flags change, instead of being polled.
//!
//! A subscription has a lifetime and lapses silently when it runs out, so a
//! subscriber renews before then. The `subscriber-process-identifier` is the
//! subscriber's own number, echoed back in every notification, which is how
//! a notification is matched to the subscription that asked for it.
//!
//! The grammar is the driver's; the values are the probe's.

use super::PropertyRef;
use super::bridge::{id, object, octets, property, reference, values};
use crate::tag::{ObjectId, Value};
use regalium_bacnet::service::{CovNotification, PropertyValue, SubscribeCov};

/// `SubscribeCOV` parameters. Without `confirmed` and `lifetime` it is a
/// cancellation (§13.14.1.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Subscribe {
    /// The subscriber's own identifier for this subscription.
    pub process: u32,
    /// The object to watch.
    pub object: ObjectId,
    /// Ask for confirmed notifications (each one acknowledged)?
    pub confirmed: Option<bool>,
    /// Seconds until it lapses; 0 is "indefinite", which few devices accept.
    pub lifetime: Option<u32>,
}

/// One notification: the values that changed.
#[derive(Clone, Debug, PartialEq)]
pub struct Notification {
    /// Echoes [`Subscribe::process`].
    pub process: u32,
    /// The device that sent it.
    pub device: ObjectId,
    /// The object that changed.
    pub object: ObjectId,
    /// Seconds left on the subscription.
    pub time_remaining: u32,
    /// Usually present-value and status-flags.
    pub values: Vec<(PropertyRef, Vec<Value>)>,
}

/// A `SubscribeCOV` request body.
#[must_use]
pub fn subscribe(s: &Subscribe) -> Vec<u8> {
    let mut out = Vec::new();
    SubscribeCov {
        process: s.process,
        object: id(s.object),
        confirmed: s.confirmed,
        lifetime: s.lifetime,
    }
    .encode(&mut out);
    out
}

/// Decodes a `SubscribeCOV` body, borrowing it.
#[must_use]
pub fn decode_subscribe(body: &[u8]) -> Option<Subscribe> {
    let s = SubscribeCov::decode(body).ok()?;
    Some(Subscribe {
        process: s.process,
        object: object(s.object),
        confirmed: s.confirmed,
        lifetime: s.lifetime,
    })
}

/// A COV notification body: the same for the confirmed and unconfirmed forms.
#[must_use]
pub fn notification(n: &Notification) -> Vec<u8> {
    let encoded: Vec<Vec<u8>> = n.values.iter().map(|(_, v)| octets(v)).collect();
    let values = n
        .values
        .iter()
        .zip(&encoded)
        .map(|((p, _), value)| PropertyValue {
            property: reference(*p),
            value,
            priority: None,
        })
        .collect();
    let mut out = Vec::new();
    CovNotification {
        process: n.process,
        device: id(n.device),
        object: id(n.object),
        time_remaining: n.time_remaining,
        values,
    }
    .encode(&mut out);
    out
}

/// Decodes a notification body, borrowing it.
#[must_use]
pub fn decode_notification(body: &[u8]) -> Option<Notification> {
    let n = CovNotification::decode(body).ok()?;
    let values = n
        .values
        .iter()
        .map(|v| Some((property(v.property), values(v.value)?)))
        .collect::<Option<Vec<_>>>()?;
    Some(Notification {
        process: n.process,
        device: object(n.device),
        object: object(n.object),
        time_remaining: n.time_remaining,
        values,
    })
}
