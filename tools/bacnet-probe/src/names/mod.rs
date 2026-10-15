// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Human-readable labels for the numbers on the wire. Partial on purpose:
//! anything unlisted prints as its number rather than being guessed at.

mod services;
mod units;

pub use services::*;
pub use units::*;

use crate::tag::{ObjectId, Value};

/// `BACnetObjectType` (§21), indexed by type number.
const OBJECT_TYPES: &[&str] = &[
    "analog-input",
    "analog-output",
    "analog-value",
    "binary-input",
    "binary-output",
    "binary-value",
    "calendar",
    "command",
    "device",
    "event-enrollment",
    "file",
    "group",
    "loop",
    "multi-state-input",
    "multi-state-output",
    "notification-class",
    "program",
    "schedule",
    "averaging",
    "multi-state-value",
    "trend-log",
    "life-safety-point",
    "life-safety-zone",
    "accumulator",
    "pulse-converter",
    "event-log",
    "global-group",
    "trend-log-multiple",
    "load-control",
    "structured-view",
    "access-door",
    "timer",
    "access-credential",
    "access-point",
    "access-rights",
    "access-user",
    "access-zone",
    "credential-data-input",
    "network-security",
    "bitstring-value",
    "characterstring-value",
    "date-pattern-value",
    "date-value",
    "datetime-pattern-value",
    "datetime-value",
    "integer-value",
    "large-analog-value",
    "octetstring-value",
    "positive-integer-value",
    "time-pattern-value",
    "time-value",
    "notification-forwarder",
    "alert-enrollment",
    "channel",
    "lighting-output",
    "binary-lighting-output",
    "network-port",
    "elevator-group",
    "escalator",
    "lift",
];

/// An object type by its standard name, or `type-N` when unknown.
#[must_use]
pub fn object_type(kind: u16) -> String {
    OBJECT_TYPES
        .get(usize::from(kind))
        .map_or_else(|| format!("type-{kind}"), |n| (*n).to_owned())
}

/// `analog-input:1`.
#[must_use]
pub fn object(id: ObjectId) -> String {
    format!("{}:{}", object_type(id.kind), id.instance)
}

/// Parses `analog-input:1` or `0:1`.
#[must_use]
pub fn parse_object(s: &str) -> Option<ObjectId> {
    let (kind, instance) = s.split_once(':')?;
    let kind = kind.parse().ok().or_else(|| {
        let i = OBJECT_TYPES
            .iter()
            .position(|n| n.eq_ignore_ascii_case(kind))?;
        u16::try_from(i).ok()
    })?;
    Some(ObjectId::new(kind, instance.parse().ok()?))
}

/// Object types whose present-value is worth polling.
#[must_use]
pub fn is_point(kind: u16) -> bool {
    matches!(kind, 0..=5 | 13 | 14 | 19 | 45 | 46 | 48)
}

/// Object types whose present-value is a number with units.
#[must_use]
pub fn is_analog(kind: u16) -> bool {
    matches!(kind, 0..=2 | 46)
}

/// Renders a present-value for its object type.
pub fn value(kind: u16, value: &Value, unit: Option<u32>) -> String {
    match value {
        Value::Real(v) => format!("{v:.2}{}", unit.map(units).unwrap_or_default()),
        Value::Double(v) => format!("{v:.2}{}", unit.map(units).unwrap_or_default()),
        Value::Enumerated(0) if matches!(kind, 3..=5) => "inactive".into(),
        Value::Enumerated(1) if matches!(kind, 3..=5) => "active".into(),
        Value::Unsigned(v) if matches!(kind, 13 | 14 | 19) => format!("state {v}"),
        Value::Unsigned(v) => v.to_string(),
        Value::Signed(v) => v.to_string(),
        Value::Enumerated(v) => format!("enum {v}"),
        Value::Bool(b) => b.to_string(),
        Value::Text(s) => format!("{s:?}"),
        Value::Null => "null".into(),
        Value::Object(id) => object(*id),
        Value::Octets(o) => format!("{} octets {:02x?}", o.len(), o),
        Value::Bits(b) => b.iter().map(|on| if *on { '1' } else { '0' }).collect(),
        other => format!("{other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_object_parses_by_name_or_by_number() {
        assert_eq!(parse_object("analog-input:1"), Some(ObjectId::new(0, 1)));
        assert_eq!(parse_object("Lift:3"), Some(ObjectId::new(59, 3)));
        assert_eq!(parse_object("19:2"), Some(ObjectId::new(19, 2)));
        assert_eq!(parse_object("no-such-type:1"), None);
    }

    #[test]
    fn the_object_type_table_is_indexed_by_type_number() {
        assert_eq!(object_type(8), "device");
        assert_eq!(object_type(19), "multi-state-value");
        assert_eq!(object_type(56), "network-port");
        assert_eq!(object_type(59), "lift");
    }
}
