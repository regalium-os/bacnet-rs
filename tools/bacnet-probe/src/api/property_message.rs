// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `Property` messages from a report, each with its value or its refusal.

use super::{name, value};
use crate::scan::ObjectReport;
use prost_types::Timestamp;
use regalium_bacnet_schema::object as schema;
use std::time::SystemTime;

/// Properties from a report. With `every`, a property the device left out
/// is listed as unknown-property rather than dropped, since it was asked for.
pub(super) fn messages(
    instance: u32,
    r: &ObjectReport,
    wanted: &[u32],
    every: bool,
) -> Vec<schema::Property> {
    let now = Some(Timestamp::from(SystemTime::now()));
    wanted
        .iter()
        .filter_map(|&p| {
            let read = r.reads.iter().find(|(q, _)| *q == p).map(|(_, v)| v);
            let (value, error) = match read {
                Some(Ok(v)) => (Some(value::values_to_schema(v)), None),
                Some(Err(e)) => (None, Some(error_from(e))),
                None if every => (None, Some(error_from("property: unknown-property"))),
                None => return None,
            };
            Some(schema::Property {
                name: name::property(instance, r.id, p),
                value,
                write_priority: 0,
                error,
                read_time: now,
            })
        })
        .collect()
}

/// `class: code`, as the scan records a refusal, into the schema's error.
pub(super) fn error_from(text: &str) -> schema::PropertyError {
    let (class, code) = text.split_once(": ").unwrap_or(("communication", text));
    schema::PropertyError {
        error_class: class.to_owned(),
        error_code: code.to_owned(),
    }
}
