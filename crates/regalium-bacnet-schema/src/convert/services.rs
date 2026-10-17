// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The services beyond `ReadProperty`, both ways: a `ReadPropertyMultiple`
//! answer and a COV notification into the model, and a subscription or an
//! updated property out of it as the request that makes it so.

use super::encode::encode_value;
use super::names::{object_name, parse_object_segment, parse_property_name, property_name};
use super::{property_error, value};
use crate::object::{Notification, Property, Subscription};
use regalium_bacnet_core::Error;
use regalium_bacnet_core::enumerated::parse_property;
use regalium_bacnet_core::primitive::ObjectIdentifier;
use regalium_bacnet_core::service::{
    CovNotification, PropertyReference, PropertyValue, ReadProperty, ReadPropertyMultipleAck,
    SubscribeCov, SubscribeCovProperty, WriteProperty,
};

/// Every property a `ReadPropertyMultiple-ACK` from `device` carries, each
/// with its value or its error, in the device's order.
///
/// # Errors
///
/// When a value's octets are not well-formed elements.
pub fn properties<'a>(
    device: u32,
    ack: &ReadPropertyMultipleAck<'a>,
) -> Result<Vec<Property>, Error<'a>> {
    let mut all = Vec::new();
    for o in &ack.objects {
        for r in &o.results {
            let read = ReadProperty {
                object: o.object,
                property: r.property.property,
                index: r.property.index,
            };
            all.push(match r.result {
                Ok(octets) => Property {
                    name: property_name(device, o.object, r.property.property),
                    value: Some(value(octets)?),
                    ..Property::default()
                },
                Err(e) => property_error(device, &read, &e),
            });
        }
    }
    Ok(all)
}

/// The subscription a notification answers: its process identifier is the
/// subscription's segment of the name.
#[must_use]
pub fn subscription_name(device: u32, object: ObjectIdentifier, process: u32) -> String {
    format!("{}/subscriptions/{process}", object_name(device, object))
}

/// A COV notification as the model's [`Notification`].
///
/// # Errors
///
/// When a value's octets are not well-formed elements.
pub fn notification<'a>(n: &CovNotification<'a>) -> Result<Notification, Error<'a>> {
    let device = n.device.instance;
    let values = n
        .values
        .iter()
        .map(|v| {
            Ok(Property {
                name: property_name(device, n.object, v.property.property),
                value: Some(value(v.value)?),
                ..Property::default()
            })
        })
        .collect::<Result<_, Error<'a>>>()?;
    Ok(Notification {
        subscription: subscription_name(device, n.object, n.process),
        values,
        time_remaining: Some(prost_types::Duration {
            seconds: n.time_remaining.into(),
            nanos: 0,
        }),
    })
}

/// The request a [`Subscription`] stands for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CovRequest {
    /// `SubscribeCOV`: the object's standard COV properties.
    Object(SubscribeCov),
    /// `SubscribeCOVProperty`: one property, with its own increment.
    Property(SubscribeCovProperty),
}

/// The device a subscription is on, and the request that makes it. Its
/// name's last segment is the process identifier; the lifetime defaults to
/// 120 s, as the schema says. `None` for a name that is not a subscription's.
#[must_use]
pub fn subscribe(s: &Subscription) -> Option<(u32, CovRequest)> {
    let (object_path, process) = s.name.rsplit_once("/subscriptions/")?;
    let (device, object) = object_path
        .strip_prefix("devices/")?
        .split_once("/objects/")?;
    let device: u32 = device.parse().ok()?;
    let object = parse_object_segment(object)?;
    let lifetime = s
        .lifetime
        .as_ref()
        .map_or(Some(120), |d| u32::try_from(d.seconds).ok())?;
    let subscription = SubscribeCov {
        process: process.parse().ok()?,
        object,
        confirmed: Some(s.confirmed),
        lifetime: Some(lifetime),
    };
    if s.property.is_empty() {
        return Some((device, CovRequest::Object(subscription)));
    }
    #[allow(
        clippy::cast_possible_truncation,
        reason = "an increment is a REAL on the wire"
    )]
    let increment = s.increment.map(|i| i as f32);
    let property = PropertyReference::new(parse_property(&s.property)?);
    Some((
        device,
        CovRequest::Property(SubscribeCovProperty {
            subscription,
            property,
            increment,
        }),
    ))
}

/// A write an updated [`Property`] asks for, owning its value's octets.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Write {
    /// The device to send it to.
    pub device: u32,
    /// The object.
    pub object: ObjectIdentifier,
    /// The property.
    pub property: PropertyReference,
    /// The value, encoded.
    pub value: Vec<u8>,
    /// The priority, when the property set one.
    pub priority: Option<u8>,
}

impl Write {
    /// The `WriteProperty` request, borrowing the value.
    #[must_use]
    pub fn request(&self) -> WriteProperty<'_> {
        WriteProperty {
            object: self.object,
            value: PropertyValue {
                property: self.property,
                value: &self.value,
                priority: self.priority,
            },
        }
    }
}

/// The write an updated property asks for: `UpdateProperty` as the
/// protocol spells it. `None` for a name that is not a property's, or a
/// value that cannot be encoded.
#[must_use]
pub fn write(p: &Property) -> Option<Write> {
    let (device, object, property) = parse_property_name(&p.name)?;
    Some(Write {
        device,
        object,
        property: PropertyReference::new(property),
        value: encode_value(p.value.as_ref()?)?,
        priority: u8::try_from(p.write_priority)
            .ok()
            .filter(|p| (1..=16).contains(p)),
    })
}
