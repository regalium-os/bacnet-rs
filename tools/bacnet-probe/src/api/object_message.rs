// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! An `Object` message from what was read: the typed view of a point.

use super::{name, value};
use crate::scan::ObjectReport;
use crate::{Value, explain, names};
use prost_types::Timestamp;
use regalium_bacnet_schema::object as schema;
use std::time::SystemTime;

/// An Object message from what was read.
pub(super) fn object_message(instance: u32, r: &ObjectReport, read: SystemTime) -> schema::Object {
    let text = |p| match r.get(p) {
        Some([Value::Text(s)]) => s.clone(),
        _ => String::new(),
    };
    let label = |p, kind: Option<u16>| match r.get(p) {
        Some([Value::Enumerated(e)]) => explain::enum_label(p, kind, *e)
            .unwrap_or_else(|| format!("{}-{e}", crate::props::name(p))),
        _ => String::new(),
    };
    let status_flags = match r.get(111) {
        Some([Value::Bits(b)]) => Some(schema::StatusFlags {
            alarm: b.first().copied().unwrap_or(false),
            fault: b.get(1).copied().unwrap_or(false),
            overridden: b.get(2).copied().unwrap_or(false),
            isolated: b.get(3).copied().unwrap_or(false),
        }),
        _ => None,
    };
    schema::Object {
        name: name::object(instance, r.id),
        object_type: names::object_type(r.id.kind),
        instance: i32::try_from(r.id.instance).unwrap_or(i32::MAX),
        display_name: r.name(),
        description: text(28),
        present_value: r.get(85).map(value::values_to_schema),
        write_priority: 0,
        priority_array: r
            .get(87)
            .map(|s| s.iter().map(value::to_schema).collect())
            .unwrap_or_default(),
        units: r
            .units()
            .map(|u| names::unit_label(u).unwrap_or_else(|| format!("units-{u}")))
            .unwrap_or_default(),
        state_texts: r
            .labels()
            .iter()
            .filter_map(|v| match v {
                Value::Text(s) => Some(s.clone()),
                _ => None,
            })
            .collect(),
        status_flags,
        event_state: label(36, Some(r.id.kind)),
        reliability: label(103, Some(r.id.kind)),
        isolated: r.get(81) == Some(&[Value::Bool(true)]),
        read_time: Some(Timestamp::from(read)),
    }
}
