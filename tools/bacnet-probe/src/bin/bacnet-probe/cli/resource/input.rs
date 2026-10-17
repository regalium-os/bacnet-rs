// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! What a person types, as the value the property actually holds.
//!
//! A device refuses a REAL written to an enumerated property, so `75` cannot
//! be sent as whatever it looks like. It is read against the property's
//! current value: a REAL stays a REAL, an unsigned state accepts the
//! state's name, a binary accepts `active`, `inactive` or its own
//! active-text and inactive-text, and `null` relinquishes at a priority.

use prost_types::NullValue;
use regalium_bacnet_schema::object::{self as obj, value::Kind};

/// `text` as a value of `current`'s type, naming states from `states`
/// (inactive then active for a binary, 1-based for a multi-state).
///
/// # Errors
///
/// When `text` cannot be that type; the message says what would be.
pub(crate) fn parse(
    text: &str,
    current: Option<&obj::Value>,
    states: &[String],
) -> Result<obj::Value, String> {
    let t = text.trim();
    let kind = if t.eq_ignore_ascii_case("null") {
        Kind::NullValue(NullValue::NullValue.into())
    } else {
        match current.and_then(|v| v.kind.as_ref()) {
            Some(Kind::RealValue(_)) => Kind::RealValue(number(t)?),
            Some(Kind::DoubleValue(_)) => Kind::DoubleValue(number(t)?),
            Some(Kind::SignedValue(_)) => Kind::SignedValue(integer(t)?),
            Some(Kind::UnsignedValue(_)) => {
                Kind::UnsignedValue(state(t, states, 1).map_or_else(|| integer(t), Ok)?)
            }
            Some(Kind::EnumeratedValue(_)) => {
                let named = state(t, states, 0).or(match t.to_ascii_lowercase().as_str() {
                    "inactive" => Some(0),
                    "active" => Some(1),
                    _ => None,
                });
                Kind::EnumeratedValue(named.map_or_else(|| integer(t), Ok)?)
            }
            Some(Kind::BooleanValue(_)) => Kind::BooleanValue(boolean(t)?),
            Some(Kind::CharacterStringValue(_)) => {
                Kind::CharacterStringValue(obj::CharacterString {
                    text: text.to_owned(),
                    character_set: obj::CharacterSet::Utf8.into(),
                })
            }
            Some(other) => {
                return Err(format!(
                    "writing a {} from the command line is not supported yet",
                    kind_name(other)
                ));
            }
            None => guess(t),
        }
    };
    Ok(obj::Value {
        kind: Some(kind),
        encoded: Vec::new(),
    })
}

/// With nothing to go by: a number with a point is a REAL, a whole number
/// unsigned, true/false a boolean, anything else text.
fn guess(t: &str) -> Kind {
    if let Ok(b) = boolean(t) {
        Kind::BooleanValue(b)
    } else if let Ok(n) = t
        .parse::<i64>()
        .map_err(drop)
        .and_then(|n| u64::try_from(n).map_err(drop))
    {
        Kind::UnsignedValue(i64::try_from(n).unwrap_or(i64::MAX))
    } else if let Ok(f) = t.parse::<f32>() {
        Kind::RealValue(f)
    } else {
        Kind::CharacterStringValue(obj::CharacterString {
            text: t.to_owned(),
            character_set: obj::CharacterSet::Utf8.into(),
        })
    }
}

fn state(t: &str, states: &[String], base: i64) -> Option<i64> {
    let i = states.iter().position(|s| s.eq_ignore_ascii_case(t))?;
    i64::try_from(i).ok().map(|i| i + base)
}

fn number<T: std::str::FromStr>(t: &str) -> Result<T, String> {
    t.parse()
        .map_err(|_| format!("{t:?} is not a number, and this property holds one"))
}

fn integer(t: &str) -> Result<i64, String> {
    t.parse()
        .map_err(|_| format!("{t:?} is not a whole number or a state this property names"))
}

fn boolean(t: &str) -> Result<bool, String> {
    match t.to_ascii_lowercase().as_str() {
        "true" | "on" | "yes" => Ok(true),
        "false" | "off" | "no" => Ok(false),
        _ => Err(format!("{t:?} is not true or false")),
    }
}

fn kind_name(k: &Kind) -> &'static str {
    match k {
        Kind::OctetStringValue(_) => "octet string",
        Kind::BitStringValue(_) => "bit string",
        Kind::DateValue(_) => "date",
        Kind::TimeValue(_) => "time",
        Kind::ObjectIdValue(_) => "object identifier",
        Kind::ConstructedValue(_) => "list or array",
        _ => "value",
    }
}

#[cfg(test)]
mod tests;
