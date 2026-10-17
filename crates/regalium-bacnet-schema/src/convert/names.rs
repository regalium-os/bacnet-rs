// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Resource names (AIP-122) for what the protocol identifies by number:
//! `devices/599/objects/analog-input-1/properties/present-value`.
//!
//! Every name parses back to the numbers it came from, including a type or
//! property this build cannot name, which is spelled `type-130` or
//! `property-512`. A bare number is never accepted: it is not a name this
//! API issues, and accepting it would give one resource two names.

use regalium_bacnet_core::enumerated::{object_type, parse_object_type, parse_property, property};
use regalium_bacnet_core::primitive::ObjectIdentifier;

/// The largest instance an object identifier can carry (§20.2.14).
const MAX_INSTANCE: u32 = 0x3F_FFFF;

/// `analog-input`, or `type-130`.
#[must_use]
pub fn object_type_name(number: u16) -> String {
    object_type(number).map_or_else(|| format!("type-{number}"), str::to_owned)
}

/// `present-value`, or `property-512`.
#[must_use]
pub fn property_segment(number: u32) -> String {
    property(number).map_or_else(|| format!("property-{number}"), str::to_owned)
}

/// `analog-input-1`: an object's segment of its name.
#[must_use]
pub fn object_segment(id: ObjectIdentifier) -> String {
    format!("{}-{}", object_type_name(id.object_type), id.instance)
}

/// `devices/599`.
#[must_use]
pub fn device_name(instance: u32) -> String {
    format!("devices/{instance}")
}

/// `devices/599/objects/analog-input-1`.
#[must_use]
pub fn object_name(device: u32, id: ObjectIdentifier) -> String {
    format!("{}/objects/{}", device_name(device), object_segment(id))
}

/// `devices/599/objects/analog-input-1/properties/present-value`.
#[must_use]
pub fn property_name(device: u32, id: ObjectIdentifier, prop: u32) -> String {
    format!(
        "{}/properties/{}",
        object_name(device, id),
        property_segment(prop)
    )
}

/// Reads `analog-input-1` back. The instance follows the last hyphen, since
/// type names contain hyphens of their own.
#[must_use]
pub fn parse_object_segment(segment: &str) -> Option<ObjectIdentifier> {
    let (kind, instance) = segment.rsplit_once('-')?;
    let instance: u32 = instance.parse().ok().filter(|i| *i <= MAX_INSTANCE)?;
    Some(ObjectIdentifier {
        object_type: parse_object_type(kind)?,
        instance,
    })
}

/// Reads `devices/599` back.
#[must_use]
pub fn parse_device_name(name: &str) -> Option<u32> {
    name.strip_prefix("devices/")?
        .parse()
        .ok()
        .filter(|i| *i <= MAX_INSTANCE)
}

/// Reads a property's full name back: device, object and property.
#[must_use]
pub fn parse_property_name(name: &str) -> Option<(u32, ObjectIdentifier, u32)> {
    let rest = name.strip_prefix("devices/")?;
    let (device, rest) = rest.split_once("/objects/")?;
    let (object, prop) = rest.split_once("/properties/")?;
    Some((
        parse_device_name(&format!("devices/{device}"))?,
        parse_object_segment(object)?,
        parse_property(prop)?,
    ))
}
