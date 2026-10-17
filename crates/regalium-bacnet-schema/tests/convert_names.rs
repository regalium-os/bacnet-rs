// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used)]

//! Resource names: every name the model issues reads back as the request
//! that gets it, a type or property this build cannot name included, and a
//! name it never issues — a bare number, an instance out of range — reads
//! as nothing.

use regalium_bacnet_schema::convert;

#[test]
fn every_property_name_reads_back_as_the_read_property_that_gets_it() {
    for (name, kind, instance, prop) in [
        (
            "devices/599/objects/analog-input-1/properties/present-value",
            0,
            1,
            85,
        ),
        (
            "devices/599/objects/device-599/properties/property-list",
            8,
            599,
            371,
        ),
        (
            "devices/7/objects/type-130-7/properties/property-512",
            130,
            7,
            512,
        ),
        (
            "devices/7/objects/multi-state-value-4194303/properties/state-text",
            19,
            4_194_303,
            110,
        ),
    ] {
        let (device, read) = convert::read_property(name).unwrap();
        assert_eq!(
            (read.object.object_type, read.object.instance, read.property),
            (kind, instance, prop)
        );
        assert_eq!(
            convert::property_name(device, read.object, read.property),
            name
        );
    }
    for bad in [
        "devices/599/objects/0-1/properties/present-value",
        "devices/599/objects/analog-input-1/properties/85",
        "devices/599/objects/analog-input-4194304/properties/present-value",
        "devices/x/objects/analog-input-1/properties/present-value",
        "objects/analog-input-1/properties/present-value",
    ] {
        assert_eq!(convert::read_property(bad), None, "{bad}");
    }
}
