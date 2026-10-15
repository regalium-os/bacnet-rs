// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `BACnet` values as JSON: raw numbers stay numbers, enumerations gain
//! their standard name, named bit strings become names or booleans.

use super::Json;
use crate::tag::Value;
use crate::{explain, names, props};

/// One value of `prop` on an object of type `kind`.
pub fn value(prop: Option<u32>, kind: Option<u16>, v: &Value) -> Json {
    match v {
        Value::Null => Json::Null,
        Value::Bool(b) => Json::Bool(*b),
        Value::Unsigned(n) => Json::UInt(*n),
        Value::Signed(n) => Json::Int(*n),
        Value::Real(f) => Json::Num(f64::from(*f)),
        Value::Double(f) => Json::Num(*f),
        Value::Text(s) => Json::str(s.clone()),
        Value::Octets(o) => Json::Str(crate::dissect::hex(o)),
        Value::Object(id) => Json::Str(names::object(*id)),
        Value::Date(_) | Value::Time(_) => Json::Str(explain::one(prop, kind, v)),
        Value::Context(n) => Json::Str(format!("context[{n}]")),
        Value::Enumerated(e) => match prop.and_then(|p| explain::enum_label(p, kind, *e)) {
            Some(name) => Json::obj([
                ("value", Json::UInt((*e).into())),
                ("name", Json::Str(name)),
            ]),
            None => Json::UInt((*e).into()),
        },
        Value::Bits(bits) => match (prop, prop.and_then(|p| explain::bit_names(p, bits))) {
            // status-flags reads best as named booleans.
            (Some(111), _) => Json::Obj(
                ["in-alarm", "fault", "overridden", "out-of-service"]
                    .iter()
                    .zip(bits.iter().chain(std::iter::repeat(&false)))
                    .map(|(n, b)| ((*n).to_owned(), Json::Bool(*b)))
                    .collect(),
            ),
            (_, Some(set)) => Json::Arr(set.into_iter().map(Json::Str).collect()),
            (_, None) => Json::Arr(bits.iter().map(|b| Json::Bool(*b)).collect()),
        },
    }
}

/// A property read whole: an array property is always a JSON array, a
/// scalar property is its one value.
#[must_use]
pub fn values(prop: u32, kind: u16, vs: &[Value]) -> Json {
    match vs {
        [v] if !props::is_array(prop) => value(Some(prop), Some(kind), v),
        vs => Json::Arr(
            vs.iter()
                .map(|v| value(Some(prop), Some(kind), v))
                .collect(),
        ),
    }
}
