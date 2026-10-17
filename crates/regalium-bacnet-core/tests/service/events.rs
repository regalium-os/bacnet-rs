// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! How the service corpora spell an event notification.

use super::render::{object, text, value};
use regalium_bacnet_core::service::{EventNotification, TimeStamp};

fn stamp(t: &TimeStamp) -> String {
    let time = |t: &regalium_bacnet_core::primitive::Time| {
        format!(
            "{:02}:{:02}:{:02}.{:02}",
            t.hour, t.minute, t.second, t.hundredths
        )
    };
    match t {
        TimeStamp::Time(t) => format!("time {}", time(t)),
        TimeStamp::Sequence(n) => format!("sequence {n}"),
        TimeStamp::DateTime(d, t) => {
            format!(
                "{:04}-{:02}-{:02} {}",
                u16::from(d.year) + 1900,
                d.month,
                d.day,
                time(t)
            )
        }
    }
}

pub(crate) fn event(confirmed: bool, n: &EventNotification<'_>) -> String {
    use std::fmt::Write;
    let mut out = format!(
        "event-notification {} process {} {} {} at {} class {} priority {} type {}",
        if confirmed {
            "confirmed"
        } else {
            "unconfirmed"
        },
        n.process,
        object(n.device),
        object(n.object),
        stamp(&n.time),
        n.class,
        n.priority,
        n.event_type
    );
    out.push_str(match n.notify_type {
        0 => " alarm",
        1 => " event",
        _ => " ack-notification",
    });
    if let Some(m) = &n.message {
        let _ = write!(out, " {}", text(m));
    }
    if n.ack_required == Some(true) {
        out.push_str(" ack-required");
    }
    if let Some(f) = n.from_state {
        let _ = write!(out, " from {f}");
    }
    let _ = write!(out, " to {}", n.to_state);
    if let Some(v) = n.values {
        let _ = write!(out, " values {}", value(v));
    }
    out
}
