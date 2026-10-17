// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! What a device says about itself, as labels: the one declaration every
//! series, span and log line about the device carries, so "every device on
//! firmware 11" or "every lift from this vendor" is a filter, not a join.
//!
//! The labels are per device, not per reading: a building of a few hundred
//! devices adds a few hundred label values, however many points each has.
//! A device's identity changes when it is upgraded, and its series change
//! with it, which is the point — a dashboard shows the upgrade happen.

use crate::scan::ObjectReport;
use crate::tag::Value;
use regalium_bacnet_telemetry::{keys, trace_attrs};

/// The properties of the Device object that identify it (§12.11).
pub const IDENTITY_PROPS: [u32; 8] = [77, 121, 120, 70, 44, 12, 139, 58];

/// A device's identity. Fields it has not read are empty (or 0), and an
/// empty label is no label: a filter on firmware simply does not match it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Identity {
    /// The device instance.
    pub instance: u32,
    /// object-name.
    pub name: String,
    /// vendor-name.
    pub vendor: String,
    /// vendor-identifier, ASHRAE's registry number.
    pub vendor_id: u32,
    /// model-name.
    pub model: String,
    /// firmware-revision.
    pub firmware: String,
    /// application-software-version.
    pub software: String,
    /// protocol-revision.
    pub protocol_revision: u32,
    /// location.
    pub location: String,
}

trace_attrs! { Identity => {
    instance: keys::DEVICE_INSTANCE,
    name: keys::DEVICE_NAME,
    vendor: keys::VENDOR_NAME,
    vendor_id: keys::VENDOR_ID,
    model: keys::DEVICE_MODEL,
    firmware: keys::DEVICE_FIRMWARE,
    software: keys::DEVICE_SOFTWARE,
    protocol_revision: keys::PROTOCOL_REVISION,
    location: keys::DEVICE_LOCATION,
} }

impl Identity {
    /// A device known only by its instance, before its properties are read.
    #[must_use]
    pub fn unknown(instance: u32) -> Self {
        Self {
            instance,
            ..Self::default()
        }
    }

    /// The identity a read of [`IDENTITY_PROPS`] gives.
    #[must_use]
    pub fn from_report(instance: u32, report: &ObjectReport) -> Self {
        let text = |p| match report.get(p) {
            Some([Value::Text(s)]) => s.clone(),
            _ => String::new(),
        };
        let number = |p| match report.get(p) {
            Some([Value::Unsigned(n)]) => u32::try_from(*n).unwrap_or(0),
            _ => 0,
        };
        Self {
            instance,
            name: text(77),
            vendor: text(121),
            vendor_id: number(120),
            model: text(70),
            firmware: text(44),
            software: text(12),
            protocol_revision: number(139),
            location: text(58),
        }
    }
}

/// What kind of point an object type is, for the `bacnet.object.kind`
/// label: `analog`, `binary`, `multi-state`, or `other`.
#[must_use]
pub fn object_kind(kind: u16) -> &'static str {
    match kind {
        0..=2 | 46 => "analog",
        3..=5 => "binary",
        13 | 14 | 19 => "multi-state",
        45 | 48 => "integer",
        _ => "other",
    }
}
