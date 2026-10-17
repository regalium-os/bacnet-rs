// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `--format json` for the API's messages: proto3 JSON field names, empty
//! fields left out, so a script reads what a transport would later send.

use super::value::value_json;
use bacnet_probe::json::Json;
use regalium_bacnet_schema::device as dev;
use regalium_bacnet_schema::object as obj;

/// An object from pairs, dropping the empty ones.
fn fields(pairs: Vec<(&str, Json)>) -> Json {
    Json::obj(pairs.into_iter().filter(|(_, v)| {
        !matches!(v, Json::Null)
            && *v != Json::str("")
            && *v != Json::Arr(Vec::new())
            && *v != Json::Int(0)
    }))
}

fn text(s: &str) -> Json {
    Json::str(s)
}

pub(crate) fn device(d: &dev::Device) -> Json {
    let address = d.address.as_ref().map_or(Json::Null, |a| {
        fields(vec![
            ("endpoint", text(&a.endpoint)),
            ("networkNumber", Json::Int(a.network_number.into())),
            (
                "macAddress",
                text(&bacnet_probe::dissect::hex(&a.mac_address)),
            ),
        ])
    });
    fields(vec![
        ("name", text(&d.name)),
        ("instance", Json::Int(d.instance.into())),
        ("displayName", text(&d.display_name)),
        ("description", text(&d.description)),
        ("location", text(&d.location)),
        ("vendor", text(&d.vendor)),
        ("vendorId", Json::Int(d.vendor_id.into())),
        ("model", text(&d.model)),
        ("firmwareRevision", text(&d.firmware_revision)),
        (
            "applicationSoftwareVersion",
            text(&d.application_software_version),
        ),
        ("protocolRevision", Json::Int(d.protocol_revision.into())),
        ("maxApduLength", Json::Int(d.max_apdu_length.into())),
        ("segmentation", text(d.segmentation().as_str_name())),
        ("systemStatus", text(&d.system_status)),
        ("address", address),
        ("state", text(d.state().as_str_name())),
        ("objectCount", Json::Int(d.object_count.into())),
    ])
}

pub(crate) fn object(o: &obj::Object) -> Json {
    let flags = o.status_flags.map_or(Json::Null, |f| {
        Json::obj([
            ("alarm", Json::Bool(f.alarm)),
            ("fault", Json::Bool(f.fault)),
            ("overridden", Json::Bool(f.overridden)),
            ("isolated", Json::Bool(f.isolated)),
        ])
    });
    fields(vec![
        ("name", text(&o.name)),
        ("objectType", text(&o.object_type)),
        ("instance", Json::Int(o.instance.into())),
        ("displayName", text(&o.display_name)),
        ("description", text(&o.description)),
        (
            "presentValue",
            o.present_value.as_ref().map_or(Json::Null, value_json),
        ),
        (
            "priorityArray",
            Json::Arr(o.priority_array.iter().map(value_json).collect()),
        ),
        ("units", text(&o.units)),
        (
            "stateTexts",
            Json::Arr(o.state_texts.iter().map(|s| text(s)).collect()),
        ),
        ("statusFlags", flags),
        ("eventState", text(&o.event_state)),
        ("reliability", text(&o.reliability)),
        (
            "isolated",
            if o.isolated {
                Json::Bool(true)
            } else {
                Json::Null
            },
        ),
    ])
}

pub(crate) fn property(p: &obj::Property) -> Json {
    let error = p.error.as_ref().map_or(Json::Null, |e| {
        Json::obj([
            ("errorClass", text(&e.error_class)),
            ("errorCode", text(&e.error_code)),
        ])
    });
    fields(vec![
        ("name", text(&p.name)),
        ("value", p.value.as_ref().map_or(Json::Null, value_json)),
        ("error", error),
    ])
}
