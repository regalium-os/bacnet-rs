// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `--format json`: one document for the scan, one line per watch event.

use bacnet_probe::json::{self, Json};
use bacnet_probe::scan::{DeviceReport, ObjectReport};
use bacnet_probe::{ObjectId, Value, names, props};

pub(crate) fn document(reports: &[DeviceReport]) -> Json {
    Json::obj([("devices", Json::Arr(reports.iter().map(device).collect()))])
}

fn device(r: &DeviceReport) -> Json {
    let d = &r.device;
    let route = d.route.as_ref().map_or(Json::Null, |route| {
        let mac: String = bacnet_probe::dissect::hex(&route.mac);
        Json::obj([
            ("net", Json::UInt(route.net.into())),
            ("mac", Json::Str(mac)),
        ])
    });
    let mut fields = vec![
        ("instance", Json::UInt(d.instance.into())),
        ("address", Json::Str(d.addr.to_string())),
        ("route", route),
        ("max_apdu", Json::UInt(d.max_apdu)),
        ("vendor_id", Json::UInt(d.vendor)),
        ("properties", properties(&r.this)),
    ];
    if let Some(errors) = errors(&r.this) {
        fields.push(("errors", errors));
    }
    if let Some(n) = r.object_count {
        fields.push(("object_count", Json::UInt(n.into())));
    }
    fields.push(("objects", Json::Arr(r.objects.iter().map(object).collect())));
    if let Some(p) = &r.problem {
        fields.push(("problem", Json::str(p.clone())));
    }
    Json::obj(fields)
}

fn object(o: &ObjectReport) -> Json {
    let mut fields = vec![
        ("id", Json::Str(names::object(o.id))),
        ("type", Json::Str(names::object_type(o.id.kind))),
        ("instance", Json::UInt(o.id.instance.into())),
        ("properties", properties(o)),
    ];
    if let Some(errors) = errors(o) {
        fields.push(("errors", errors));
    }
    Json::obj(fields)
}

fn properties(o: &ObjectReport) -> Json {
    Json::Obj(
        o.reads
            .iter()
            .filter_map(|(p, r)| {
                Some((
                    props::name(*p),
                    json::values(*p, o.id.kind, r.as_deref().ok()?),
                ))
            })
            .collect(),
    )
}

fn errors(o: &ObjectReport) -> Option<Json> {
    let errors: Vec<(String, Json)> = o
        .reads
        .iter()
        .filter_map(|(p, r)| Some((props::name(*p), Json::str(r.as_ref().err()?.clone()))))
        .collect();
    (!errors.is_empty()).then_some(Json::Obj(errors))
}

/// One `--watch` event: a present-value change, or the point going offline.
pub(crate) fn event(
    t: f32,
    device: u32,
    object: ObjectId,
    name: &str,
    from: Option<&Value>,
    to: Result<&Value, String>,
) -> Json {
    let raw =
        |v: Option<&Value>| v.map_or(Json::Null, |v| json::value(Some(85), Some(object.kind), v));
    let mut fields = vec![
        ("t", Json::Num((f64::from(t) * 10.0).round() / 10.0)),
        ("device", Json::UInt(device.into())),
        ("object", Json::Str(names::object(object))),
        ("name", Json::str(name)),
        ("from", raw(from)),
    ];
    match to {
        Ok(v) => fields.extend([("event", Json::str("changed")), ("to", raw(Some(v)))]),
        Err(e) => fields.extend([("event", Json::str("offline")), ("error", Json::Str(e))]),
    }
    Json::obj(fields)
}
