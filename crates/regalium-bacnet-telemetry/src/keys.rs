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
