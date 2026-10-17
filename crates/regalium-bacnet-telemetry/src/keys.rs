// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Every attribute key the driver may record, in one place.
//!
//! Names follow OpenTelemetry convention: dotted, lower case, namespaced
//! `bacnet.` except where a general convention already exists (`server.*`).
//! Add a key here, with what it identifies, before recording it anywhere.
//!
//! Each key identifies *which* thing a value belongs to. None carries a value
//! read from equipment: present-values, property contents and anything a
//! person typed into a device are recorded as metric values or not at all.

use crate::Key;

/// A device's instance number, unique on the internetwork (ASHRAE 135
/// §12.11.1): the first thing to filter a dashboard by.
pub const DEVICE_INSTANCE: Key = Key::declare("bacnet.device.instance");

/// The device object's `object-name`, so a series is readable without a
/// lookup table. Stable for a device's lifetime unless commissioned again.
pub const DEVICE_NAME: Key = Key::declare("bacnet.device.name");

/// The device's `vendor-name`.
pub const VENDOR_NAME: Key = Key::declare("bacnet.vendor.name");

/// A `BACnet` network number (§6.2.2), set when a device sits behind a router,
/// e.g. an MS/TP trunk behind a `BACnet`/IP gateway.
pub const NETWORK: Key = Key::declare("bacnet.network");

/// The `IP:port` a device answers from.
pub const SERVER_ADDRESS: Key = Key::declare("server.address");

/// An object type by its standard name, e.g. `analog-input`.
pub const OBJECT_TYPE: Key = Key::declare("bacnet.object.type");

/// An object instance within its type.
pub const OBJECT_INSTANCE: Key = Key::declare("bacnet.object.instance");

/// An object's `object-name`.
pub const OBJECT_NAME: Key = Key::declare("bacnet.object.name");

/// A property by its standard name, e.g. `present-value`.
pub const PROPERTY: Key = Key::declare("bacnet.property.identifier");

/// An engineering unit by its standard name, e.g. `degrees-celsius`.
pub const UNITS: Key = Key::declare("bacnet.units");

/// One of the four status flags (§12.50.11): `in-alarm`, `fault`,
/// `overridden`, `out-of-service`.
pub const STATUS_FLAG: Key = Key::declare("bacnet.status.flag");

/// A service by its standard name, e.g. `readProperty`.
pub const SERVICE: Key = Key::declare("bacnet.service");

/// How a request ended: `ok`, `timeout`, `error`, `reject`, `abort`,
/// `segmented`, `malformed`.
pub const OUTCOME: Key = Key::declare("bacnet.outcome");

/// `sent` or `received`.
pub const DIRECTION: Key = Key::declare("bacnet.direction");

/// Octets a tag decode was handed.
pub const TAG_OCTETS: Key = Key::declare("bacnet.tag.octets");

/// Elements it found.
pub const TAG_ELEMENTS: Key = Key::declare("bacnet.tag.elements");

/// Whether every header was spelled canonically. False marks a device that
/// encodes the long way, which is legal and worth knowing.
pub const TAG_CANONICAL: Key = Key::declare("bacnet.tag.canonical");

/// A confirmed request's invoke id (§20.1.2.3): pairs it with its reply.
pub const INVOKE_ID: Key = Key::declare("bacnet.invoke_id");

/// How many times a request was sent before it was answered or abandoned.
pub const ATTEMPTS: Key = Key::declare("bacnet.request.attempts");

/// The error class a device refused with, by name: `property`, `object`.
pub const ERROR_CLASS: Key = Key::declare("bacnet.error.class");

/// The error code a device refused with, by name: `unknown-property`.
pub const ERROR_CODE: Key = Key::declare("bacnet.error.code");

/// The priority a write was made at, 1 to 16 (§19.2).
pub const PRIORITY: Key = Key::declare("bacnet.write.priority");

/// Datagrams a discovery sent.
pub const DATAGRAMS: Key = Key::declare("bacnet.discovery.datagrams");

/// Devices or objects a discovery found.
pub const FOUND: Key = Key::declare("bacnet.discovery.found");

/// Which pass of a repeating scenario this is.
pub const CYCLE: Key = Key::declare("bacnet.cycle");

/// How an API call ended: `ok`, or its status, `NotFound`, `Unavailable`.
pub const API_STATUS: Key = Key::declare("bacnet.api.status");

/// The vendor-identifier: ASHRAE's registry number for the vendor (§12.11).
pub const VENDOR_ID: Key = Key::declare("bacnet.vendor.id");

/// The model-name property of a device.
pub const DEVICE_MODEL: Key = Key::declare("bacnet.device.model");

/// The firmware-revision property of a device: "every device on 11.x".
pub const DEVICE_FIRMWARE: Key = Key::declare("bacnet.device.firmware");

/// The application-software-version property of a device.
pub const DEVICE_SOFTWARE: Key = Key::declare("bacnet.device.software");

/// The protocol-revision a device claims: revision 12 and revision 19 lift
/// controllers have different object models.
pub const PROTOCOL_REVISION: Key = Key::declare("bacnet.device.protocol_revision");

/// The location property of a device: where the installer said it is.
pub const DEVICE_LOCATION: Key = Key::declare("bacnet.device.location");

/// What kind of point an object is: `analog`, `binary`, `multi-state`.
pub const OBJECT_KIND: Key = Key::declare("bacnet.object.kind");

/// Octets in an NPDU or APDU, header and body.
pub const PDU_OCTETS: Key = Key::declare("bacnet.pdu.octets");

/// Whether an NPDU carries a network layer message rather than an APDU.
pub const NPDU_MESSAGE: Key = Key::declare("bacnet.npdu.message");

/// Whether an NPDU names a destination or source network: it crossed, or
/// is meant to cross, a router.
pub const NPDU_ROUTED: Key = Key::declare("bacnet.npdu.routed");

/// The APDU type, as §20.1 names it: `confirmed-request`, `error`, …
pub const APDU_TYPE: Key = Key::declare("bacnet.apdu.type");

/// The service choice number of an APDU that has one (§21).
pub const SERVICE_CHOICE: Key = Key::declare("bacnet.apdu.service_choice");

/// Whether an APDU is one segment of a larger message (§20.1.2.4).
pub const SEGMENTED: Key = Key::declare("bacnet.apdu.segmented");

/// The BVLC function of a BACnet/IP datagram: `original-unicast-npdu`,
/// `forwarded-npdu`, `register-foreign-device`, …
pub const BVLC_FUNCTION: Key = Key::declare("bacnet.ip.function");
