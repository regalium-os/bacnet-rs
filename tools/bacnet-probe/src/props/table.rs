// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The property table: every property the probe names or reads.

use super::{Prop, p};

/// Every property the probe knows, by wire number.
pub const ALL: &[Prop] = &[
    p(
        4,
        "active-text",
        "what 'active' means for this point, e.g. \"open\" or \"running\"",
    ),
    p(
        11,
        "apdu-timeout",
        "ms the device waits for a reply before retrying a confirmed request",
    ),
    p(
        12,
        "application-software-version",
        "version of the application/program loaded in the device",
    ),
    p(
        22,
        "cov-increment",
        "smallest change that triggers a change-of-value (COV) notification",
    ),
    p(
        28,
        "description",
        "free text set by the integrator; often the most human hint you get",
    ),
    p(
        36,
        "event-state",
        "alarm state: 'normal' unless an alarm or fault condition is active",
    ),
    p(44, "firmware-revision", "the device's firmware version"),
    p(
        46,
        "inactive-text",
        "what 'inactive' means for this point, e.g. \"closed\" or \"stopped\"",
    ),
    p(
        56,
        "local-date",
        "the device's own calendar date (year, month, day, weekday)",
    ),
    p(
        57,
        "local-time",
        "the device's own clock: check it when schedules misbehave",
    ),
    p(58, "location", "free text: where the device physically is"),
    p(
        62,
        "max-apdu-length-accepted",
        "largest request it accepts in one packet; bigger needs segmentation",
    ),
    p(65, "max-pres-value", "highest value present-value can take"),
    p(69, "min-pres-value", "lowest value present-value can take"),
    p(70, "model-name", "the vendor's model designation"),
    p(
        73,
        "number-of-apdu-retries",
        "how many times the device retries an unanswered request",
    ),
    p(
        74,
        "number-of-states",
        "how many states this multi-state point has (1..N)",
    ),
    p(
        75,
        "object-identifier",
        "object type + instance: the object's address inside the device",
    ),
    p(
        76,
        "object-list",
        "every object the device contains; index 0 is the count",
    ),
    p(
        77,
        "object-name",
        "the object's name, unique within the device",
    ),
    p(79, "object-type", "what kind of object this is"),
    p(
        81,
        "out-of-service",
        "true = present-value is cut off from the hardware (testing/override)",
    ),
    p(
        84,
        "polarity",
        "'reverse' = the physical signal is inverted relative to present-value",
    ),
    p(
        85,
        "present-value",
        "the live value: what the point measures or commands right now",
    ),
    p(
        87,
        "priority-array",
        "16 command slots; the lowest-numbered non-null slot wins (1 = life safety)",
    ),
    p(
        96,
        "protocol-object-types-supported",
        "object types this device can contain",
    ),
    p(
        97,
        "protocol-services-supported",
        "BACnet services this device will execute for you",
    ),
    p(
        98,
        "protocol-version",
        "BACnet protocol version (always 1 so far)",
    ),
    p(
        103,
        "reliability",
        "why present-value may be untrustworthy (no sensor, over range, ...)",
    ),
    p(
        104,
        "relinquish-default",
        "value used when every priority-array slot is null",
    ),
    p(
        106,
        "resolution",
        "smallest change the sensor can actually distinguish",
    ),
    p(
        107,
        "segmentation-supported",
        "whether it can split/reassemble messages bigger than one packet",
    ),
    p(
        110,
        "state-text",
        "name of each state: present-value N means state-text[N]",
    ),
    p(
        111,
        "status-flags",
        "health at a glance: in-alarm, fault, overridden, out-of-service",
    ),
    p(
        112,
        "system-status",
        "whether the device as a whole is operational",
    ),
    p(117, "units", "engineering units of present-value"),
    p(120, "vendor-identifier", "ASHRAE-assigned vendor number"),
    p(121, "vendor-name", "who made the device"),
    p(
        139,
        "protocol-revision",
        "BACnet revision implemented: decides which objects and services exist",
    ),
    p(
        155,
        "database-revision",
        "bumped when objects are added, removed or renamed: re-discover then",
    ),
];
