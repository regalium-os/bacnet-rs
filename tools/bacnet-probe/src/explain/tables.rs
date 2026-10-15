// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Enumeration tables (§21), indexed by wire value.

/// `BACnetEventState`.
pub(super) const EVENT_STATE: &[&str] = &[
    "normal",
    "fault",
    "offnormal",
    "high-limit",
    "low-limit",
    "life-safety-alarm",
];

/// `BACnetReliability`; 11 is unassigned.
pub(super) const RELIABILITY: &[&str] = &[
    "no-fault-detected",
    "no-sensor",
    "over-range",
    "under-range",
    "open-loop",
    "shorted-loop",
    "no-output",
    "unreliable-other",
    "process-error",
    "multi-state-fault",
    "configuration-error",
    "",
    "communication-failure",
    "member-fault",
    "monitored-object-fault",
    "tripped",
];

/// `BACnetDeviceStatus`.
pub(super) const SYSTEM_STATUS: &[&str] = &[
    "operational",
    "operational-read-only",
    "download-required",
    "download-in-progress",
    "non-operational",
    "backup-in-progress",
];

/// `BACnetSegmentation`.
pub(super) const SEGMENTATION: &[&str] = &[
    "segmented-both",
    "segmented-transmit",
    "segmented-receive",
    "no-segmentation",
];

/// `BACnetServicesSupported` bit positions (§21).
pub const SERVICES: &[&str] = &[
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
    "i-Am",
    "i-Have",
    "unconfirmedCOVNotification",
    "unconfirmedEventNotification",
    "unconfirmedPrivateTransfer",
    "unconfirmedTextMessage",
    "timeSynchronization",
    "who-Has",
    "who-Is",
    "readRange",
    "utcTimeSynchronization",
    "lifeSafetyOperation",
    "subscribeCOVProperty",
    "getEventInformation",
    "writeGroup",
];
