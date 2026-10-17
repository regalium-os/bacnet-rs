// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Every instrument the probe records, and how a reading becomes series.
//!
//! Points are gauges labelled by *which* point — device, object type and
//! instance, names, units — never by what it reads; the reading is the
//! value. Prometheus, via an OpenTelemetry collector, sees for example
//! `bacnet_point_value{bacnet_device_instance="599",bacnet_object_type=
//! "analog-input",bacnet_object_instance="1",bacnet_object_name="Car 1
//! load",bacnet_units="kilograms"} 420`.

use regalium_bacnet_telemetry::Instrument;

// Units in braces are annotations (UCUM): they say what is counted without
// claiming a physical unit, and the Prometheus translation leaves them out
// of the series name. A unit of "1" means a dimensionless ratio, and earned
// these gauges a `_ratio` suffix they are not: a 0/1 flag, a state code, and
// a present-value whose real unit is its `bacnet.units` label.

/// Confirmed requests sent, by service and outcome.
pub const REQUESTS: Instrument = Instrument::counter(
    "bacnet.client.requests",
    "{request}",
    "Confirmed requests, by service and outcome",
);
/// Round-trip time of a confirmed request, retries included.
pub const REQUEST_DURATION: Instrument = Instrument::histogram(
    "bacnet.client.request.duration",
    "ms",
    "Time from request to final reply or give-up",
);
/// Datagrams on the socket.
pub const PACKETS: Instrument = Instrument::counter(
    "bacnet.client.packets",
    "{packet}",
    "BACnet/IP datagrams sent and received",
);
/// Octets on the socket.
pub const BYTES: Instrument = Instrument::counter(
    "bacnet.client.bytes",
    "By",
    "BACnet/IP octets sent and received",
);
/// COV notifications received.
pub const COV_NOTIFICATIONS: Instrument = Instrument::counter(
    "bacnet.cov.notifications",
    "{notification}",
    "Change-of-value notifications received",
);
/// A point's present-value: analog as read, binary 0/1, multi-state its state number.
pub const POINT_VALUE: Instrument = Instrument::gauge(
    "bacnet.point.value",
    "{value}",
    "Present-value; the unit is the bacnet.units label",
);
/// One gauge per status flag, 1 when set.
pub const POINT_STATUS: Instrument = Instrument::gauge(
    "bacnet.point.status",
    "{flag}",
    "Status flags (in-alarm, fault, overridden, out-of-service), 1 when set",
);
/// `event-state` code: 0 normal, 1 fault, 2 offnormal, 3 high-limit, 4 low-limit.
pub const POINT_EVENT_STATE: Instrument = Instrument::gauge(
    "bacnet.point.event_state",
    "{code}",
    "Event-state; 0 is normal",
);
/// `reliability` code: 0 no-fault-detected.
pub const POINT_RELIABILITY: Instrument = Instrument::gauge(
    "bacnet.point.reliability",
    "{code}",
    "Reliability; 0 is no-fault-detected",
);
/// 1 when the last read of present-value succeeded.
pub const POINT_REACHABLE: Instrument = Instrument::gauge(
    "bacnet.point.reachable",
    "{flag}",
    "1 when the last present-value read succeeded",
);
/// 1 while a device answers.
pub const DEVICE_UP: Instrument = Instrument::gauge(
    "bacnet.device.up",
    "{flag}",
    "1 while the device answers requests",
);
/// Objects a device reports in its object-list.
pub const DEVICE_OBJECTS: Instrument = Instrument::gauge(
    "bacnet.device.objects",
    "{object}",
    "Length of the device's object-list",
);
/// Devices that answered the last discovery.
pub const DISCOVERED: Instrument = Instrument::gauge(
    "bacnet.discovery.devices",
    "{device}",
    "Devices that answered the last Who-Is",
);

mod record;

pub use record::*;
