// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Formatting for the composite values: the priority array and dates.

use crate::names;
use crate::tag::Value;

pub(super) fn priority_array(kind: u16, slots: &[Value], units: Option<u32>) -> String {
    let active: Vec<String> = slots
        .iter()
        .enumerate()
        .filter(|(_, v)| **v != Value::Null)
        .map(|(i, v)| format!("slot {} = {}", i + 1, names::value(kind, v, units)))
        .collect();
    match active.split_first() {
        None => format!(
            "all {} slots null, so relinquish-default applies",
            slots.len()
        ),
        Some((winner, [])) => format!("{winner} (in control); others null"),
        Some((winner, rest)) => format!("{winner} (in control); also {}", rest.join(", ")),
    }
}

pub(super) fn date(year: u8, month: u8, day: u8, weekday: u8) -> String {
    const DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let any = |v: u8, s: String| if v == 255 { "*".into() } else { s };
    format!(
        "{}-{}-{} {}",
        any(year, (1900 + u32::from(year)).to_string()),
        any(month, format!("{month:02}")),
        any(day, format!("{day:02}")),
        DAYS.get(usize::from(weekday).wrapping_sub(1))
            .unwrap_or(&"*"),
    )
}

pub(super) fn enum_name(table: &[&'static str], e: u32) -> Option<&'static str> {
    table.get(e as usize).copied().filter(|n| !n.is_empty())
}
