// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! One value, for a person (`60 percent`, `Independent (3)`) or as proto3
//! JSON (`{"realValue": 60}`).

use bacnet_probe::json::Json;
use regalium_bacnet_schema::object::{self as obj, value::Kind};

/// A value as a person reads it: `60`, `"Car 1 load"`, `analog-input-1`,
/// `[null, null, 60]`.
pub(crate) fn value_text(v: &obj::Value) -> String {
    match &v.kind {
        None => "(not decoded)".into(),
        Some(Kind::NullValue(_)) => "null".into(),
        Some(Kind::BooleanValue(b)) => b.to_string(),
        Some(Kind::UnsignedValue(n) | Kind::SignedValue(n) | Kind::EnumeratedValue(n)) => {
            n.to_string()
        }
        Some(Kind::RealValue(f)) => trim(f64::from(*f)),
        Some(Kind::DoubleValue(d)) => trim(*d),
        Some(Kind::OctetStringValue(o)) => {
            format!("0x{}", bacnet_probe::dissect::hex(o).replace(' ', ""))
        }
        Some(Kind::CharacterStringValue(s)) => format!("{:?}", s.text),
        Some(Kind::BitStringValue(b)) => b
            .bits
            .iter()
            .map(|on| if *on { '1' } else { '0' })
            .collect(),
        Some(Kind::DateValue(d)) => {
            let f =
                |x: Option<i32>, w: usize| x.map_or_else(|| "*".repeat(w), |x| format!("{x:0w$}"));
            format!("{}-{}-{}", f(d.year, 4), f(d.month, 2), f(d.day, 2))
        }
        Some(Kind::TimeValue(t)) => {
            let f = |x: Option<i32>| x.map_or_else(|| "**".into(), |x| format!("{x:02}"));
            format!(
                "{}:{}:{}.{}",
                f(t.hour),
                f(t.minute),
                f(t.second),
                f(t.hundredths)
            )
        }
        Some(Kind::ObjectIdValue(id)) => format!("{}-{}", id.object_type, id.instance),
        Some(Kind::ConstructedValue(c)) => {
            let parts: Vec<String> = c
                .elements
                .iter()
                .map(|e| e.value.as_ref().map_or_else(String::new, value_text))
                .collect();
            format!("[{}]", parts.join(", "))
        }
    }
}

/// A present-value with its meaning: units for an analog, the state's name
/// for a binary or multi-state.
pub(crate) fn present_text(o: &obj::Object) -> String {
    let Some(v) = &o.present_value else {
        return String::new();
    };
    let named = |i: i64, base: i64| {
        usize::try_from(i - base)
            .ok()
            .and_then(|i| o.state_texts.get(i))
    };
    match &v.kind {
        Some(Kind::EnumeratedValue(e)) if o.state_texts.len() == 2 => {
            named(*e, 0).map_or_else(|| value_text(v), |s| format!("{s} ({e})"))
        }
        Some(Kind::UnsignedValue(n)) if !o.state_texts.is_empty() => {
            named(*n, 1).map_or_else(|| value_text(v), |s| format!("{s} ({n})"))
        }
        _ if !o.units.is_empty() && o.units != "no-units" => {
            format!("{} {}", value_text(v), o.units)
        }
        _ => value_text(v),
    }
}

fn trim(f: f64) -> String {
    let s = format!("{f:.4}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// A value as JSON: the proto3 JSON shape, `{"realValue": 60}`.
pub(crate) fn value_json(v: &obj::Value) -> Json {
    let (key, json) = match &v.kind {
        None => return Json::Null,
        Some(Kind::NullValue(_)) => ("nullValue", Json::Null),
        Some(Kind::BooleanValue(b)) => ("booleanValue", Json::Bool(*b)),
        Some(Kind::UnsignedValue(n)) => ("unsignedValue", Json::Int(*n)),
        Some(Kind::SignedValue(n)) => ("signedValue", Json::Int(*n)),
        Some(Kind::EnumeratedValue(n)) => ("enumeratedValue", Json::Int(*n)),
        Some(Kind::RealValue(f)) => ("realValue", Json::Num(f64::from(*f))),
        Some(Kind::DoubleValue(d)) => ("doubleValue", Json::Num(*d)),
        Some(Kind::CharacterStringValue(s)) => (
            "characterStringValue",
            Json::obj([("text", Json::str(s.text.clone()))]),
        ),
        Some(Kind::ConstructedValue(c)) => (
            "constructedValue",
            Json::obj([(
                "elements",
                Json::Arr(
                    c.elements
                        .iter()
                        .map(|e| e.value.as_ref().map_or(Json::Null, value_json))
                        .collect(),
                ),
            )]),
        ),
        Some(_) => ("text", Json::str(value_text(v))),
    };
    Json::obj([(key, json)])
}
