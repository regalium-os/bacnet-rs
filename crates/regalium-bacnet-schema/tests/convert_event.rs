// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! An event notification into the model's `Event`: the standard's words for
//! its type and states, the device's own timestamp in whichever form it
//! used, and the event values with their octets.

use prost::Message;
use regalium_bacnet_core::service::EventNotification;
use regalium_bacnet_schema::convert;
use regalium_bacnet_schema::object::Event;
use regalium_bacnet_schema::object::event::DeviceStamp;

fn octets(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect()
}

/// The events corpus's first record, without its APDU header.
const HIGH_LIMIT: &str = "09 01 1c 02 00 02 57 2c 00 00 00 01 3e 2e a4 7e 0a 11 06 b4 0e 1e 00 00 2f 3f 49 05 59 64 69 05 7d 0a 00 48 69 67 68 20 6c 6f 61 64 89 00 99 01 a9 00 b9 03 ce 5e 0c 43 e1 00 00 1a 04 80 2c 3f 80 00 00 3c 43 c8 00 00 5f cf";

#[test]
fn a_high_limit_alarm_becomes_an_event_of_its_object() {
    let body = octets(HIGH_LIMIT);
    let e = convert::event(&EventNotification::decode(&body).unwrap(), 7).unwrap();
    assert_eq!(e.name, "devices/599/objects/analog-input-1/events/7");
    assert_eq!(
        (e.event_type.as_str(), e.notify_type.as_str()),
        ("out-of-range", "alarm")
    );
    assert_eq!(
        (e.previous_event_state.as_str(), e.event_state.as_str()),
        ("normal", "high-limit")
    );
    assert_eq!(
        (
            e.notification_class,
            e.priority,
            e.message.as_str(),
            e.ack_required
        ),
        (5, 100, "High load", true)
    );
    let Some(DeviceStamp::ClockAndCalendar(dt)) = &e.device_stamp else {
        panic!("{:?}", e.device_stamp)
    };
    assert_eq!(dt.calendar.unwrap().year, Some(2026));
    assert_eq!(dt.clock.unwrap().hour, Some(14));
    assert!(
        !e.values.as_ref().unwrap().encoded.is_empty(),
        "event values keep their octets"
    );
    assert_eq!(Event::decode(e.encode_to_vec().as_slice()).unwrap(), e);
}

#[test]
fn an_acknowledgement_by_sequence_number_has_no_previous_state() {
    let body =
        octets("09 02 1c 02 00 02 57 2c 00 00 00 01 3e 19 2a 3f 49 05 59 64 69 05 89 02 b9 00");
    let e = convert::event(&EventNotification::decode(&body).unwrap(), 8).unwrap();
    assert_eq!(e.notify_type, "ack-notification");
    assert_eq!(e.previous_event_state, "");
    assert_eq!(e.device_stamp, Some(DeviceStamp::SequenceNumber(42)));
}
