// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! An event notification as the model's [`Event`].

use super::names::object_name;
use super::value;
use crate::object::event::DeviceStamp;
use crate::object::{Date, DeviceDateTime, Event, Time};
use regalium_bacnet_core::Error;
use regalium_bacnet_core::enumerated::{event_state, event_type, notify_type};
use regalium_bacnet_core::primitive;
use regalium_bacnet_core::service::{EventNotification, TimeStamp};

fn named(name: Option<&'static str>, prefix: &str, n: u32) -> String {
    name.map_or_else(|| format!("{prefix}-{n}"), str::to_owned)
}

fn field(v: u8, offset: i32) -> Option<i32> {
    (v != 255).then(|| i32::from(v) + offset)
}

fn time(t: primitive::Time) -> Time {
    Time {
        hour: field(t.hour, 0),
        minute: field(t.minute, 0),
        second: field(t.second, 0),
        hundredths: field(t.hundredths, 0),
    }
}

fn date(d: primitive::Date) -> Date {
    Date {
        year: field(d.year, 1900),
        month: field(d.month, 0),
        day: field(d.day, 0),
        day_of_week: field(d.weekday, 0),
    }
}

/// The event an event notification reports, numbered `event` by the caller
/// — the order it arrived in, say — beneath the object it is about.
///
/// The event type, states and notify type are the standard's words, or
/// `type-N`, `state-N` and `notify-N` for numbers this build cannot name.
/// `receive_time` is the caller's to set: this reads no clock.
///
/// # Errors
///
/// When the event values are not well-formed elements.
pub fn event<'a>(n: &EventNotification<'a>, event: u64) -> Result<Event, Error<'a>> {
    let device_stamp = Some(match n.time {
        TimeStamp::Time(t) => DeviceStamp::Clock(time(t)),
        TimeStamp::Sequence(s) => DeviceStamp::SequenceNumber(s.into()),
        TimeStamp::DateTime(d, t) => DeviceStamp::ClockAndCalendar(DeviceDateTime {
            calendar: Some(date(d)),
            clock: Some(time(t)),
        }),
    });
    Ok(Event {
        name: format!(
            "{}/events/{event}",
            object_name(n.device.instance, n.object)
        ),
        event_type: named(event_type(n.event_type), "type", n.event_type),
        notify_type: named(notify_type(n.notify_type), "notify", n.notify_type),
        previous_event_state: n
            .from_state
            .map(|s| named(event_state(s), "state", s))
            .unwrap_or_default(),
        event_state: named(event_state(n.to_state), "state", n.to_state),
        notification_class: i32::try_from(n.class).unwrap_or(i32::MAX),
        priority: n.priority.into(),
        message: n
            .message
            .and_then(|m| m.text().map(std::borrow::Cow::into_owned))
            .unwrap_or_default(),
        ack_required: n.ack_required == Some(true),
        values: n.values.map(value).transpose()?,
        receive_time: None,
        device_stamp,
    })
}
