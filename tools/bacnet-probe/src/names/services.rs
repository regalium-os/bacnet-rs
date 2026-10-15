// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Services, reject and abort reasons, and error classes and codes (§18,
//! §21) by their standard names. Shared by the dissector and the metric
//! labels, so a packet and a series always agree on what a service is called.

/// A confirmed service by its standard name, e.g. 12 `readProperty`.
#[must_use]
pub fn confirmed_service(s: u8) -> &'static str {
    const NAMES: &[&str] = &[
        "acknowledgeAlarm",
        "confirmedCOVNotification",
        "confirmedEventNotification",
        "getAlarmSummary",
        "getEnrollmentSummary",
        "subscribeCOV",
        "atomicReadFile",
        "atomicWriteFile",
        "addListElement",
        "removeListElement",
        "createObject",
        "deleteObject",
        "readProperty",
        "readPropertyConditional",
        "readPropertyMultiple",
        "writeProperty",
        "writePropertyMultiple",
        "deviceCommunicationControl",
        "confirmedPrivateTransfer",
        "confirmedTextMessage",
        "reinitializeDevice",
        "vtOpen",
        "vtClose",
        "vtData",
        "authenticate",
        "requestKey",
        "readRange",
        "lifeSafetyOperation",
        "subscribeCOVProperty",
        "getEventInformation",
    ];
    NAMES.get(usize::from(s)).unwrap_or(&"unknown")
}

/// An unconfirmed service by its standard name, e.g. 8 `who-Is`.
#[must_use]
pub fn unconfirmed_service(s: u8) -> &'static str {
    const NAMES: &[&str] = &[
        "i-Am",
        "i-Have",
        "unconfirmedCOVNotification",
        "unconfirmedEventNotification",
        "unconfirmedPrivateTransfer",
        "unconfirmedTextMessage",
        "timeSynchronization",
        "who-Has",
        "who-Is",
        "utcTimeSynchronization",
        "writeGroup",
    ];
    NAMES.get(usize::from(s)).unwrap_or(&"unknown")
}

/// `BACnetRejectReason` (§18.8).
#[must_use]
pub fn reject_reason(r: u8) -> &'static str {
    const NAMES: &[&str] = &[
        "other",
        "buffer-overflow",
        "inconsistent-parameters",
        "invalid-parameter-data-type",
        "invalid-tag",
        "missing-required-parameter",
        "parameter-out-of-range",
        "too-many-arguments",
        "undefined-enumeration",
        "unrecognized-service",
    ];
    NAMES.get(usize::from(r)).unwrap_or(&"unknown")
}

/// `BACnetAbortReason` (§18.9).
#[must_use]
pub fn abort_reason(r: u8) -> &'static str {
    const NAMES: &[&str] = &[
        "other",
        "buffer-overflow",
        "invalid-apdu-in-this-state",
        "preempted-by-higher-priority-task",
        "segmentation-not-supported",
        "security-error",
        "insufficient-security",
        "window-size-out-of-range",
        "application-exceeded-reply-time",
        "out-of-resources",
        "tsm-timeout",
        "apdu-too-long",
    ];
    NAMES.get(usize::from(r)).unwrap_or(&"unknown")
}

/// `BACnetErrorClass` by name.
#[must_use]
pub fn error_class(class: u32) -> String {
    const CLASSES: &[&str] = &[
        "device",
        "object",
        "property",
        "resources",
        "security",
        "services",
        "vt",
        "communication",
    ];
    CLASSES
        .get(class as usize)
        .map_or_else(|| format!("class-{class}"), |c| (*c).to_owned())
}

/// `BACnetErrorCode` by name, where common.
#[must_use]
pub fn error_code(code: u32) -> String {
    let name = match code {
        0 => "other",
        2 => "configuration-in-progress",
        3 => "device-busy",
        9 => "invalid-data-type",
        25 => "operational-problem",
        27 => "read-access-denied",
        31 => "unknown-object",
        32 => "unknown-property",
        37 => "value-out-of-range",
        40 => "write-access-denied",
        42 => "invalid-array-index",
        45 => "optional-functionality-not-supported",
        50 => "property-is-not-an-array",
        _ => return format!("code-{code}"),
    };
    name.to_owned()
}

/// Error-PDU class and code (§18): `property: unknown-property`.
#[must_use]
pub fn error(class: u32, code: u32) -> String {
    format!("{}: {}", error_class(class), error_code(code))
}
