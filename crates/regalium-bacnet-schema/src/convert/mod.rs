// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! From the protocol to the model and back: what a decoded service says,
//! as the AIP resources `protobuf/` declares.
//!
//! | Protocol | Model |
//! | --- | --- |
//! | I-Am (§16.1) | [`Device`]: name, instance, vendor, max APDU, segmentation |
//! | `ReadProperty-ACK` (§15.5) | [`Property`] with its [`Value`](crate::object::Value) |
//! | Error for `ReadProperty` (§18) | [`Property`] with its [`PropertyError`] |
//! | a property's resource name | the `ReadProperty` that gets it |
//! | `ReadPropertyMultiple-ACK` (§15.7) | a [`Property`] per result, value or error |
//! | COV notification (§13.6, §13.7) | [`Notification`](crate::object::Notification) |
//! | [`Subscription`](crate::object::Subscription) | the `SubscribeCOV` or `SubscribeCOVProperty` that makes it |
//! | an updated [`Property`] | the `WriteProperty` that sets it |
//!
//! The conversions copy: a model value outlives the datagram it came from.
//! They read no clock, so `read_time` and `discover_time` are the caller's
//! to set when it knows when the datagram arrived.

mod encode;
mod names;
mod services;
mod value;

pub use encode::encode_value;
pub use names::{
    device_name, object_name, object_segment, object_type_name, parse_device_name,
    parse_object_segment, parse_property_name, property_name, property_segment,
};
pub use services::{
    CovRequest, Write, notification, properties, subscribe, subscription_name, write,
};
pub use value::value;

use crate::device::{Device, Segmentation};
use crate::object::{Property, PropertyError};
use regalium_bacnet_core::Error;
use regalium_bacnet_core::enumerated::{self, error_class, error_code};
use regalium_bacnet_core::service::{IAm, ReadProperty, ReadPropertyAck, ServiceError};

/// The device an I-Am announces. Fields an I-Am does not carry — name,
/// vendor name, firmware — stay empty until the Device object is read.
#[must_use]
pub fn device(i_am: &IAm) -> Device {
    let instance = i_am.device.instance;
    Device {
        name: device_name(instance),
        instance: i32::try_from(instance).unwrap_or(i32::MAX),
        vendor_id: i32::try_from(i_am.vendor_id).unwrap_or(i32::MAX),
        max_apdu_length: i32::try_from(i_am.max_apdu).unwrap_or(i32::MAX),
        segmentation: segmentation(i_am.segmentation).into(),
        ..Device::default()
    }
}

fn segmentation(s: enumerated::Segmentation) -> Segmentation {
    match s {
        enumerated::Segmentation::Both => Segmentation::Both,
        enumerated::Segmentation::Transmit => Segmentation::Transmit,
        enumerated::Segmentation::Receive => Segmentation::Receive,
        enumerated::Segmentation::NoSegmentation => Segmentation::None,
        enumerated::Segmentation::Other(_) => Segmentation::Unspecified,
    }
}

/// The property a `ReadProperty-ACK` from `device` answers with.
///
/// An ACK for one element of an array (`index` set) is still named for the
/// whole property: AIP names a resource, and an element is a view of one.
///
/// # Errors
///
/// When the value's octets are not well-formed elements.
pub fn property<'a>(device: u32, ack: &ReadPropertyAck<'a>) -> Result<Property, Error<'a>> {
    Ok(Property {
        name: property_name(device, ack.read.object, ack.read.property),
        value: Some(value(ack.value)?),
        ..Property::default()
    })
}

/// The property a failed read of `read` names, carrying the error instead
/// of a value — so a batch of reads reports each failure where it happened.
#[must_use]
pub fn property_error(device: u32, read: &ReadProperty, error: &ServiceError) -> Property {
    let class =
        error_class(error.class).map_or_else(|| format!("class-{}", error.class), str::to_owned);
    let code = error_code(error.code).map_or_else(|| format!("code-{}", error.code), str::to_owned);
    Property {
        name: property_name(device, read.object, read.property),
        error: Some(PropertyError {
            error_class: class,
            error_code: code,
        }),
        ..Property::default()
    }
}

/// The `ReadProperty` that gets the property `name`, and the device to send
/// it to: `GetProperty` as the protocol spells it. `None` for a name that is
/// not a property's.
#[must_use]
pub fn read_property(name: &str) -> Option<(u32, ReadProperty)> {
    let (device, object, property) = parse_property_name(name)?;
    Some((
        device,
        ReadProperty {
            object,
            property,
            index: None,
        },
    ))
}
