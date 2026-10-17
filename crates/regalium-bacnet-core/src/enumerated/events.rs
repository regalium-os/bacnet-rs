// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `BACnetEventType`, `BACnetEventState` and `BACnetNotifyType` (§21): the
//! words an alarm is described in.

/// Event types by number, `(number, name)`. 7 was buffer-ready before
/// revision 7 renumbered it to 10, and 12 is unassigned; both stay unnamed.
pub(crate) const EVENT_TYPES: &[(u32, &str)] = &[
    (0, "change-of-bitstring"),
    (1, "change-of-state"),
    (2, "change-of-value"),
    (3, "command-failure"),
    (4, "floating-limit"),
    (5, "out-of-range"),
    (6, "complex-event-type"),
    (8, "change-of-life-safety"),
    (9, "extended"),
    (10, "buffer-ready"),
    (11, "unsigned-range"),
    (13, "access-event"),
    (14, "double-out-of-range"),
    (15, "signed-out-of-range"),
    (16, "unsigned-out-of-range"),
    (17, "change-of-characterstring"),
    (18, "change-of-status-flags"),
    (19, "change-of-reliability"),
    (20, "none"),
    (21, "change-of-discrete-value"),
    (22, "change-of-timer"),
];

/// Event states, entry N is state N.
pub(crate) const EVENT_STATES: &[&str] = &[
    "normal",
    "fault",
    "offnormal",
    "high-limit",
    "low-limit",
    "life-safety-alarm",
];

/// Notify types, entry N is type N.
pub(crate) const NOTIFY_TYPES: &[&str] = &["alarm", "event", "ack-notification"];
