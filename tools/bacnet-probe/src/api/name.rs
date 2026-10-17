// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Resource names: `devices/599`, `devices/599/objects/analog-input-1`,
//! `devices/599/objects/analog-input-1/properties/present-value`.
//!
//! Segments use the standard's own spelling, so a name read off a PICS
//! document or a screen is the name in a request. A type or property this
//! build cannot name is `type-130-7` or `property-512`, and parses back to
//! exactly the number it came from.

use super::error::ApiError;
use crate::{ObjectId, names, props};

/// `devices/599`.
#[must_use]
pub fn device(instance: u32) -> String {
    format!("devices/{instance}")
}

/// `devices/599/objects/analog-input-1`.
#[must_use]
pub fn object(instance: u32, id: ObjectId) -> String {
    format!("{}/objects/{}", device(instance), object_segment(id))
}

/// `devices/599/objects/analog-input-1/properties/present-value`.
#[must_use]
pub fn property(instance: u32, id: ObjectId, prop: u32) -> String {
    format!("{}/properties/{}", object(instance, id), props::name(prop))
}

/// `analog-input-1`, or `type-130-7` for a type this build does not name.
#[must_use]
pub fn object_segment(id: ObjectId) -> String {
    format!("{}-{}", names::object_type(id.kind), id.instance)
}

/// An object type by name, `analog-input`, or as `type-130`.
#[must_use]
pub fn parse_object_type(s: &str) -> Option<u16> {
    if let Some(n) = s.strip_prefix("type-") {
        return n.parse().ok();
    }
    if s.parse::<u16>().is_ok() {
        return None; // A bare number is not a name this API issues.
    }
    names::parse_object(&format!("{s}:0")).map(|id| id.kind)
}

/// `analog-input-1` back to its identifier.
#[must_use]
pub fn parse_object_segment(s: &str) -> Option<ObjectId> {
    let (kind, instance) = s.rsplit_once('-')?;
    let instance: u32 = instance.parse().ok().filter(|i| *i <= 0x3F_FFFF)?;
    Some(ObjectId::new(parse_object_type(kind)?, instance))
}

/// A property by name, `present-value`, or as `property-512`.
#[must_use]
pub fn parse_property_segment(s: &str) -> Option<u32> {
    match s.strip_prefix("property-") {
        Some(n) => n.parse().ok(),
        // A bare number is not a name this API issues.
        None if s.parse::<u32>().is_ok() => None,
        None => props::parse(s),
    }
}

/// The device instance of `devices/{device}`.
///
/// # Errors
///
/// When `name` is not a device name.
pub fn parse_device(name: &str) -> Result<u32, ApiError> {
    let id = name
        .strip_prefix("devices/")
        .ok_or_else(|| bad(name, "devices/{device}"))?;
    id.parse()
        .ok()
        .filter(|i| *i < 0x3F_FFFF)
        .ok_or_else(|| bad(name, "devices/{device}, a device instance below 4194303"))
}

/// The device and object of `devices/{device}/objects/{object}`.
///
/// # Errors
///
/// When `name` is not an object name.
pub fn parse_object(name: &str) -> Result<(u32, ObjectId), ApiError> {
    let pattern = "devices/{device}/objects/{object}";
    let (parent, segment) = name
        .split_once("/objects/")
        .ok_or_else(|| bad(name, pattern))?;
    let id = parse_object_segment(segment).ok_or_else(|| bad(name, pattern))?;
    Ok((parse_device(parent)?, id))
}

/// The device, object and property of a property name.
///
/// # Errors
///
/// When `name` is not a property name.
pub fn parse_property(name: &str) -> Result<(u32, ObjectId, u32), ApiError> {
    let pattern = "devices/{device}/objects/{object}/properties/{property}";
    let (parent, segment) = name
        .split_once("/properties/")
        .ok_or_else(|| bad(name, pattern))?;
    let prop = parse_property_segment(segment).ok_or_else(|| bad(name, pattern))?;
    let (device, id) = parse_object(parent)?;
    Ok((device, id, prop))
}

fn bad(name: &str, pattern: &str) -> ApiError {
    ApiError::invalid(format!(
        "{name:?} is not a resource name of the form {pattern}"
    ))
}

#[cfg(test)]
mod tests {
    use crate::ObjectId;

    #[test]
    fn names_round_trip_including_what_this_build_cannot_name() {
        for (id, segment) in [
            (ObjectId::new(0, 1), "analog-input-1"),
            (ObjectId::new(130, 7), "type-130-7"),
        ] {
            assert_eq!(super::object_segment(id), segment);
            assert_eq!(super::parse_object_segment(segment), Some(id));
        }
        assert_eq!(super::parse_property_segment("property-512"), Some(512));
        assert_eq!(super::parse_property_segment("present-value"), Some(85));
        assert_eq!(
            super::parse_property_segment("85"),
            None,
            "a bare number is not an issued name"
        );
        assert_eq!(super::parse_object_segment("0-1"), None);
    }
}
