// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Readings into series: which labels each instrument carries.

use super::{
    DEVICE_OBJECTS, DEVICE_UP, DISCOVERED, POINT_EVENT_STATE, POINT_REACHABLE, POINT_RELIABILITY,
    POINT_STATUS, POINT_VALUE,
};
use crate::client::Device;
use crate::identity::{Identity, object_kind};
use crate::scan::ObjectReport;
use crate::tag::{ObjectId, Value};
use crate::{names, property};
use regalium_bacnet_telemetry::{Attribute, Meter, Traced, keys};

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

/// The labels every series about one object carries: its device's whole
/// identity, then which object it is and what kind.
fn object_labels<'a>(
    device: &'a Identity,
    kind: &'a str,
    object: ObjectId,
    name: &'a str,
) -> Vec<Attribute<'a>> {
    let mut labels = device.attribute_list();
    labels.extend([
        Attribute::str(keys::OBJECT_TYPE, kind),
        Attribute::int(keys::OBJECT_INSTANCE, object.instance.into()),
        Attribute::str(keys::OBJECT_NAME, name),
        Attribute::str(keys::OBJECT_KIND, object_kind(object.kind)),
    ]);
    labels
}

/// Records a point's present-value (or its unreachability), with every
/// identifying label.
pub fn record_value(
    meter: &dyn Meter,
    device: &Identity,
    object: ObjectId,
    name: &str,
    units: Option<u32>,
    value: Option<&Value>,
) {
    let kind = names::object_type(object.kind);
    let unit = units.and_then(names::unit_label).unwrap_or_default();
    let base = object_labels(device, &kind, object, name);
    meter.record(
        &POINT_REACHABLE,
        f64::from(u8::from(value.is_some())),
        &base,
    );
    if let Some(v) = value.and_then(numeric) {
        let mut labels = base;
        labels.push(Attribute::str(keys::UNITS, &unit));
        meter.record(&POINT_VALUE, v, &labels);
    }
}

/// Records everything known about one object: value, flags, health.
pub fn record_object(meter: &dyn Meter, device: &Identity, o: &ObjectReport) {
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
    let base = object_labels(device, &kind, o.id, &name);
    if let Some([Value::Bits(bits)]) = o.get(111) {
        for (i, flag) in ["in-alarm", "fault", "overridden", "out-of-service"]
            .iter()
            .enumerate()
        {
            let on = bits.get(i).copied().unwrap_or(false);
            let mut labels = base.clone();
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

/// Records whether a device answers, labelled with its identity and where
/// it was reached.
pub fn record_device(
    meter: &dyn Meter,
    device: &Device,
    identity: &Identity,
    up: bool,
    objects: Option<u32>,
) {
    let address = device.addr.to_string();
    let network = device.route.as_ref().map_or(0, |r| i64::from(r.net));
    let mut labels = identity.attribute_list();
    if let Some(n) = objects {
        meter.record(&DEVICE_OBJECTS, f64::from(n), &labels);
    }
    labels.push(Attribute::str(keys::SERVER_ADDRESS, &address));
    labels.push(Attribute::int(keys::NETWORK, network));
    meter.record(&DEVICE_UP, f64::from(u8::from(up)), &labels);
}

/// Records how many devices the last discovery found.
pub fn record_discovery(meter: &dyn Meter, found: usize) {
    #[allow(clippy::cast_precision_loss)] // Devices on one network.
    meter.record(&DISCOVERED, found as f64, &[]);
}
