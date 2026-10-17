// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The API's messages for people and programs: one `key: value` line per
//! field, or JSON keyed by the proto3 JSON field names. Empty fields are
//! left out of both, as proto3 JSON leaves them out.

use regalium_bacnet_schema::device as dev;
use regalium_bacnet_schema::object::{self as obj, value::Kind};

pub(crate) use super::value::{present_text, value_text};

/// `key: value` lines, empty values left out.
pub(crate) fn lines(fields: &[(&str, String)]) -> String {
    use std::fmt::Write;
    fields
        .iter()
        .filter(|(_, v)| !v.is_empty())
        .fold(String::new(), |mut out, (k, v)| {
            let _ = writeln!(out, "  {k}: {v}");
            out
        })
}

pub(crate) fn device_text(d: &dev::Device) -> String {
    let address = d
        .address
        .as_ref()
        .map(|a| a.endpoint.clone())
        .unwrap_or_default();
    format!(
        "{}\n{}",
        d.name,
        lines(&[
            ("display_name", d.display_name.clone()),
            (
                "vendor",
                if d.vendor.is_empty() {
                    String::new()
                } else {
                    format!("{} ({})", d.vendor, d.vendor_id)
                }
            ),
            ("model", d.model.clone()),
            ("firmware_revision", d.firmware_revision.clone()),
            ("protocol_revision", nonzero(d.protocol_revision)),
            ("max_apdu_length", nonzero(d.max_apdu_length)),
            ("object_count", nonzero(d.object_count)),
            ("location", d.location.clone()),
            ("address", address),
            (
                "state",
                d.state()
                    .as_str_name()
                    .trim_start_matches("DEVICE_STATE_")
                    .to_lowercase()
            ),
        ])
    )
}

pub(crate) fn object_text(o: &obj::Object) -> String {
    let slots: Vec<String> = o
        .priority_array
        .iter()
        .enumerate()
        .filter(|(_, v)| !matches!(v.kind, Some(Kind::NullValue(_))))
        .map(|(i, v)| format!("{}={}", i + 1, value_text(v)))
        .collect();
    let flags = o
        .status_flags
        .map(|f| {
            [
                ("alarm", f.alarm),
                ("fault", f.fault),
                ("overridden", f.overridden),
                ("isolated", f.isolated),
            ]
            .iter()
            .filter(|(_, on)| *on)
            .map(|(n, _)| *n)
            .collect::<Vec<_>>()
            .join(", ")
        })
        .unwrap_or_default();
    format!(
        "{}\n{}",
        o.name,
        lines(&[
            ("display_name", o.display_name.clone()),
            ("present_value", present_text(o)),
            ("priority_array", slots.join(" ")),
            ("states", o.state_texts.join(", ")),
            ("status_flags", flags),
            ("event_state", o.event_state.clone()),
            ("reliability", o.reliability.clone()),
            (
                "isolated",
                if o.isolated {
                    "true".into()
                } else {
                    String::new()
                }
            ),
            ("description", o.description.clone()),
        ])
    )
}

pub(crate) fn property_text(p: &obj::Property) -> String {
    let short = p.name.rsplit('/').next().unwrap_or(&p.name);
    match (&p.value, &p.error) {
        (_, Some(e)) => format!("{short}: refused ({}: {})", e.error_class, e.error_code),
        (Some(v), None) => format!("{short} = {}", value_text(v)),
        (None, None) => format!("{short}: (no value)"),
    }
}

fn nonzero(n: i32) -> String {
    if n == 0 { String::new() } else { n.to_string() }
}
