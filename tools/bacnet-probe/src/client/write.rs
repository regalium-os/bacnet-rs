// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Changing a device: `WriteProperty`, `WritePropertyMultiple`,
//! `CreateObject` and `DeleteObject`.
//!
//! Each sends one confirmed request and is not retried past the client's
//! retry count. A write that times out may still have been applied — the
//! reply was lost, not necessarily the request — so a caller that must know
//! reads the property back rather than writing again.

use super::{Client, Device, ReadError};
use crate::service::object::{self, Specifier};
use crate::service::write::{self, PropertyValue};
use crate::service::{PropertyRef, confirmed};
use crate::tag::{ObjectId, Value};

impl Client {
    /// Writes one property: `WriteProperty` (§15.9). `priority` applies to a
    /// commandable property, and a [`Value::Null`] written at one
    /// relinquishes that slot.
    ///
    /// # Errors
    ///
    /// See [`ReadError`]; a refusal is [`ReadError::Bacnet`], e.g.
    /// property: write-access-denied.
    pub fn write_property(
        &mut self,
        device: &Device,
        object: ObjectId,
        property: PropertyRef,
        values: Vec<Value>,
        priority: Option<u8>,
    ) -> Result<(), ReadError> {
        let write = PropertyValue {
            property,
            values,
            priority,
        };
        let body = write::request(object, &write);
        self.confirm(device, confirmed::WRITE_PROPERTY, &body)
            .map(drop)
    }

    /// Writes several properties of several objects in one request:
    /// `WritePropertyMultiple` (§15.10). The device applies them in order and
    /// stops at the first it refuses; the ones before it stand.
    ///
    /// # Errors
    ///
    /// The first refusal, as [`ReadError::Bacnet`]; [`ReadError::unsupported`]
    /// when the device has no `WritePropertyMultiple`, and the caller writes
    /// property by property instead.
    pub fn write_property_multiple(
        &mut self,
        device: &Device,
        writes: &[(ObjectId, Vec<PropertyValue>)],
    ) -> Result<(), ReadError> {
        let body = write::request_multiple(writes);
        self.confirm(device, confirmed::WRITE_PROPERTY_MULTIPLE, &body)
            .map(drop)
    }

    /// Creates an object: `CreateObject` (§15.3). Returns the identifier the
    /// device gave it, which for [`Specifier::Type`] is the device's choice.
    ///
    /// # Errors
    ///
    /// See [`ReadError`]; object: dynamic-creation-not-supported from a
    /// device whose database is fixed, which is most of them.
    pub fn create_object(
        &mut self,
        device: &Device,
        what: Specifier,
        initial: &[PropertyValue],
    ) -> Result<ObjectId, ReadError> {
        let body = object::create_request(what, initial);
        let ack = self.confirm(device, confirmed::CREATE_OBJECT, &body)?;
        object::decode_object_body(&ack).ok_or(ReadError::Malformed)
    }

    /// Deletes an object: `DeleteObject` (§15.4).
    ///
    /// # Errors
    ///
    /// See [`ReadError`]; object: object-deletion-not-permitted for an
    /// object the device's configuration depends on.
    pub fn delete_object(&mut self, device: &Device, object: ObjectId) -> Result<(), ReadError> {
        let body = object::object_body(object);
        self.confirm(device, confirmed::DELETE_OBJECT, &body)
            .map(drop)
    }
}
