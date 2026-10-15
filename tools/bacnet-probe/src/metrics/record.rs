// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Readings into series: which labels each instrument carries.

use super::{
    DEVICE_OBJECTS, DEVICE_UP, DISCOVERED, POINT_EVENT_STATE, POINT_REACHABLE, POINT_RELIABILITY,
    POINT_STATUS, POINT_VALUE,
};
use crate::client::Device;
use crate::scan::ObjectReport;
use crate::tag::{ObjectId, Value};
use crate::{names, property};
use regalium_bacnet_telemetry::{Attribute, Meter, keys};

/// What identifies a device in a series.
#[derive(Clone, Copy, Debug)]
pub struct DeviceLabels<'a> {
    /// Device instance.
    pub instance: u32,
    /// Its `object-name`, or empty when unread.
    pub name: &'a str,
}

/// A value as a number, when it is one: text and constructed values are not.
#[must_use]
pub fn numeric(value: &Value) -> Option<f64> {
    #[allow(clippy::cast_precision_loss)] // A point reading fits f64.
    match value {
        Value::Real(v) => Some(f64::from(*v)),
        Value::Double(v) => Some(*v),
        Value::Unsigned(v) => Some(*v as f64),
        Value::Signed(v) => Some(*v as f64),
        Value::Enumerated(v) => Some(f64::from(*v)),
        Value::Bool(b) => Some(f64::from(u8::from(*b))),
        _ => None,
    }
}

/// Records a point's present-value (or its unreachability), with every
/// identifying label.
pub fn record_value(
    meter: &dyn Meter,
    device: DeviceLabels<'_>,
    object: ObjectId,
    name: &str,
    units: Option<u32>,
    value: Option<&Value>,
) {
    let kind = names::object_type(object.kind);
    let unit = units.and_then(names::unit_label).unwrap_or_default();
    let labels = [
        Attribute::int(keys::DEVICE_INSTANCE, device.instance.into()),
        Attribute::str(keys::DEVICE_NAME, device.name),
        Attribute::str(keys::OBJECT_TYPE, &kind),
        Attribute::int(keys::OBJECT_INSTANCE, object.instance.into()),
        Attribute::str(keys::OBJECT_NAME, name),
        Attribute::str(keys::UNITS, &unit),
    ];
    if let Some(v) = value.and_then(numeric) {
        meter.record(&POINT_VALUE, v, &labels);
    }
    let reachable = labels.get(..5).unwrap_or_default();
    meter.record(
        &POINT_REACHABLE,
        f64::from(u8::from(value.is_some())),
        reachable,
    );
}

/// Records everything known about one object: value, flags, health.
pub fn record_object(meter: &dyn Meter, device: DeviceLabels<'_>, o: &ObjectReport) {
    if !names::is_point(o.id.kind) {
        return;
    }
    let name = o.name();
    let read = o.error(property::PRESENT_VALUE).is_none();
    record_value(
        meter,
        device,
        o.id,
        &name,
        o.units(),
        o.present_value().filter(|_| read),
    );
    let kind = names::object_type(o.id.kind);
    let base = [
        Attribute::int(keys::DEVICE_INSTANCE, device.instance.into()),
        Attribute::str(keys::DEVICE_NAME, device.name),
        Attribute::str(keys::OBJECT_TYPE, &kind),
        Attribute::int(keys::OBJECT_INSTANCE, o.id.instance.into()),
        Attribute::str(keys::OBJECT_NAME, &name),
    ];
    if let Some([Value::Bits(bits)]) = o.get(111) {
        for (i, flag) in ["in-alarm", "fault", "overridden", "out-of-service"]
            .iter()
            .enumerate()
        {
            let on = bits.get(i).copied().unwrap_or(false);
            let mut labels = base.to_vec();
            labels.push(Attribute::str(keys::STATUS_FLAG, flag));
            meter.record(&POINT_STATUS, f64::from(u8::from(on)), &labels);
        }
    }
    for (prop, instrument) in [(36, &POINT_EVENT_STATE), (103, &POINT_RELIABILITY)] {
        if let Some([Value::Enumerated(code)]) = o.get(prop) {
            meter.record(instrument, f64::from(*code), &base);
        }
    }
}

/// Records whether a device answers, and what it says about itself.
pub fn record_device(
    meter: &dyn Meter,
    device: &Device,
    info: Option<&ObjectReport>,
    up: bool,
    objects: Option<u32>,
) {
    let name = info.map(ObjectReport::name).unwrap_or_default();
    let vendor = match info.and_then(|i| i.get(121)) {
        Some([Value::Text(v)]) => v.clone(),
        _ => String::new(),
    };
    let address = device.addr.to_string();
    let network = device.route.as_ref().map_or(0, |r| i64::from(r.net));
    let labels = [
        Attribute::int(keys::DEVICE_INSTANCE, device.instance.into()),
        Attribute::str(keys::DEVICE_NAME, &name),
        Attribute::str(keys::VENDOR_NAME, &vendor),
        Attribute::str(keys::SERVER_ADDRESS, &address),
        Attribute::int(keys::NETWORK, network),
    ];
    meter.record(&DEVICE_UP, f64::from(u8::from(up)), &labels);
    if let Some(n) = objects {
        meter.record(
            &DEVICE_OBJECTS,
            f64::from(n),
            labels.get(..2).unwrap_or_default(),
        );
    }
}

/// Records how many devices the last discovery found.
pub fn record_discovery(meter: &dyn Meter, found: usize) {
    #[allow(clippy::cast_precision_loss)] // Devices on one network.
    meter.record(&DISCOVERED, found as f64, &[]);
}
