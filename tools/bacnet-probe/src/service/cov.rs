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

use super::{PropertyRef, Reader};
use crate::tag::{self, ObjectId, Value};

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
    tag::push_unsigned(&mut out, 0, true, s.process.into());
    tag::push_object(&mut out, 1, true, s.object);
    if let Some(confirmed) = s.confirmed {
        tag::push_header(&mut out, 2, true, 1);
        out.push(u8::from(confirmed));
    }
    if let Some(lifetime) = s.lifetime {
        tag::push_unsigned(&mut out, 3, true, lifetime.into());
    }
    out
}

/// Decodes a `SubscribeCOV` body, borrowing it.
#[must_use]
pub fn decode_subscribe(body: &[u8]) -> Option<Subscribe> {
    let mut r = Reader::new(body);
    let process = u32::try_from(r.unsigned(0)?).ok()?;
    let object = r.object(1)?;
    let confirmed = if r.at_context(2) {
        Some(r.boolean(2)?)
    } else {
        None
    };
    let lifetime = r.optional_unsigned(3)?.and_then(|l| u32::try_from(l).ok());
    Some(Subscribe {
        process,
        object,
        confirmed,
        lifetime,
    })
}

/// A COV notification body: the same for the confirmed and unconfirmed forms.
#[must_use]
pub fn notification(n: &Notification) -> Vec<u8> {
    let mut out = Vec::new();
    tag::push_unsigned(&mut out, 0, true, n.process.into());
    tag::push_object(&mut out, 1, true, n.device);
    tag::push_object(&mut out, 2, true, n.object);
    tag::push_unsigned(&mut out, 3, true, n.time_remaining.into());
    tag::push_opening(&mut out, 4);
    for (p, values) in &n.values {
        tag::push_unsigned(&mut out, 0, true, p.property.into());
        if let Some(i) = p.index {
            tag::push_unsigned(&mut out, 1, true, i.into());
        }
        tag::push_opening(&mut out, 2);
        for v in values {
            tag::push_value(&mut out, v);
        }
        tag::push_closing(&mut out, 2);
    }
    tag::push_closing(&mut out, 4);
    out
}

/// Decodes a notification body, borrowing it.
#[must_use]
pub fn decode_notification(body: &[u8]) -> Option<Notification> {
    let mut r = Reader::new(body);
    let process = u32::try_from(r.unsigned(0)?).ok()?;
    let device = r.object(1)?;
    let object = r.object(2)?;
    let time_remaining = u32::try_from(r.unsigned(3)?).ok()?;
    r.opening(4)?;
    let mut values = Vec::new();
    while !r.at_closing(4) {
        let property = u32::try_from(r.unsigned(0)?).ok()?;
        let index = r.optional_unsigned(1)?.and_then(|i| u32::try_from(i).ok());
        r.opening(2)?;
        let v = r.values_until(2)?;
        r.optional_unsigned(3)?; // Priority, for commandable properties.
        values.push((PropertyRef { property, index }, v));
    }
    r.closing(4)?;
    Some(Notification {
        process,
        device,
        object,
        time_remaining,
        values,
    })
}
