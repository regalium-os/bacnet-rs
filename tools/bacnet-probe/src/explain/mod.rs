// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Turns a raw property value into what it means: enumerations named, bit
//! strings expanded, dates and times formatted, the priority array resolved.

use crate::names::{self, object_type};
use crate::tag::Value;

/// One value of property `prop` on an object of type `kind`.
#[must_use]
pub fn one(prop: Option<u32>, kind: Option<u16>, value: &Value) -> String {
    match (prop, value) {
        (Some(p), Value::Enumerated(e)) if p != 85 => match enum_label(p, kind, *e) {
            Some(name) => format!("{name} ({e})"),
            None => format!("enum {e}"),
        },
        (Some(111), Value::Bits(b)) => {
            let set = bit_names(111, b).unwrap_or_default();
            if set.is_empty() {
                "all clear".into()
            } else {
                set.join(", ")
            }
        }
        (Some(p @ (96 | 97)), Value::Bits(b)) => {
            let set = bit_names(p, b).unwrap_or_default();
            format!("{} of {}: {}", set.len(), b.len(), set.join(", "))
        }
        (_, Value::Date([year, month, day, weekday])) => date(*year, *month, *day, *weekday),
        (_, Value::Time([hour, minute, second, hundredths])) => {
            let f = |v: u8| {
                if v == 255 {
                    "**".into()
                } else {
                    format!("{v:02}")
                }
            };
            format!(
                "{}:{}:{}.{}",
                f(*hour),
                f(*minute),
                f(*second),
                f(*hundredths)
            )
        }
        _ => names::value(kind.unwrap_or(u16::MAX), value, None),
    }
}

/// The standard name of enumerated value `e` of property `prop`, if known.
pub fn enum_label(prop: u32, kind: Option<u16>, e: u32) -> Option<String> {
    let table: &[&'static str] = match prop {
        36 => EVENT_STATE,
        103 => RELIABILITY,
        112 => SYSTEM_STATUS,
        107 => SEGMENTATION,
        84 => &["normal", "reverse"],
        85 if matches!(kind, Some(3..=5)) => &["inactive", "active"],
        117 => return names::unit_label(e),
        79 => return Some(object_type(u16::try_from(e).unwrap_or(u16::MAX))),
        _ => return None,
    };
    enum_name(table, e).map(str::to_owned)
}

/// The names of the set bits of a named bit string, if `prop` is one.
#[must_use]
pub fn bit_names(prop: u32, bits: &[bool]) -> Option<Vec<String>> {
    let name = |i: usize| match prop {
        111 => ["in-alarm", "fault", "overridden", "out-of-service"]
            .get(i)
            .map(|s| (*s).to_owned()),
        97 => SERVICES.get(i).map(|s| (*s).to_owned()),
        96 => Some(object_type(u16::try_from(i).unwrap_or(u16::MAX))),
        _ => None,
    };
    matches!(prop, 96 | 97 | 111).then(|| {
        bits.iter()
            .enumerate()
            .filter(|(_, b)| **b)
            .map(|(i, _)| name(i).unwrap_or_else(|| format!("#{i}")))
            .collect()
    })
}

/// A present-value with units and the device's own name for the state:
/// `420.00 kg`, `state 2 (Normal)`, `inactive (closed)`.
#[must_use]
pub fn present(kind: u16, value: &Value, units: Option<u32>, labels: &[Value]) -> String {
    let shown = names::value(kind, value, units);
    let label = match value {
        Value::Unsigned(n) => usize::try_from(*n)
            .ok()
            .and_then(|n| labels.get(n.checked_sub(1)?)),
        Value::Enumerated(e) => usize::try_from(*e).ok().and_then(|e| labels.get(e)),
        _ => None,
    };
    match label {
        Some(Value::Text(s)) => format!("{shown} ({s})"),
        _ => shown,
    }
}

/// All values of a property read whole (arrays and lists included).
#[must_use]
pub fn many(prop: u32, kind: u16, values: &[Value], units: Option<u32>) -> String {
    match (prop, values) {
        (87, slots) => priority_array(kind, slots, units),
        (110, texts) => texts
            .iter()
            .enumerate()
            .map(|(i, t)| match t {
                Value::Text(s) => format!("{}={s}", i + 1),
                t => format!("{}={}", i + 1, names::value(kind, t, None)),
            })
            .collect::<Vec<_>>()
            .join(", "),
        (85 | 65 | 69 | 22 | 106 | 104, [v]) => names::value(kind, v, units),
        (11, [Value::Unsigned(ms)]) => format!("{ms} ms"),
        (_, [v]) => one(Some(prop), Some(kind), v),
        (_, vs) => vs
            .iter()
            .map(|v| one(Some(prop), Some(kind), v))
            .collect::<Vec<_>>()
            .join(", "),
    }
}

mod format;

use format::{date, enum_name, priority_array};
mod tables;

pub use tables::SERVICES;
use tables::{EVENT_STATE, RELIABILITY, SEGMENTATION, SYSTEM_STATUS};

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn status_flags_name_only_the_bits_that_are_set() {
        let v = Value::Bits(vec![true, false, true, false]);
        assert_eq!(one(Some(111), None, &v), "in-alarm, overridden");
        assert_eq!(
            one(Some(111), None, &Value::Bits(vec![false; 4])),
            "all clear"
        );
    }

    #[test]
    fn the_lowest_numbered_non_null_slot_is_the_one_in_control() {
        let mut slots = vec![Value::Null; 16];
        slots[7] = Value::Real(60.0);
        slots[15] = Value::Real(10.0);
        assert_eq!(
            many(87, 1, &slots, Some(98)),
            "slot 8 = 60.00 % (in control); also slot 16 = 10.00 %"
        );
    }

    #[test]
    fn an_unspecified_date_field_prints_as_a_wildcard() {
        assert_eq!(
            one(None, None, &Value::Date([126, 10, 15, 255])),
            "2026-10-15 *"
        );
    }
}
