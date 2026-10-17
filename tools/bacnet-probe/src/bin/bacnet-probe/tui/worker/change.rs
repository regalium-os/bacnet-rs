// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Changing a device from the TUI: one write, create or delete per command,
//! each answered by `Event::Changed` with what the device said.
//!
//! A change is never superseded the way a load is. It is sent once, in the
//! order the user confirmed it, and a write that times out is reported as
//! unknown rather than resent: the request may have arrived and only its
//! reply been lost.

use super::{Event, Worker};
use bacnet_probe::service::PropertyRef;
use bacnet_probe::service::object::Specifier;
use bacnet_probe::service::write::PropertyValue;
use bacnet_probe::{Device, ObjectId, ReadError, Value, names, props};

/// What the user confirmed.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Change {
    /// `WriteProperty`: `values` to `property`, at `priority` if given.
    Write {
        device: Device,
        object: ObjectId,
        property: u32,
        values: Vec<Value>,
        priority: Option<u8>,
    },
    /// `CreateObject` of `kind`, the device choosing the instance.
    Create {
        device: Device,
        kind: u16,
        initial: Vec<PropertyValue>,
    },
    /// `DeleteObject`.
    Delete { device: Device, object: ObjectId },
}

impl Worker {
    pub(super) fn change(&mut self, change: Change) {
        let structural = !matches!(change, Change::Write { .. });
        let (instance, what, result) = match change {
            Change::Write {
                device,
                object,
                property,
                values,
                priority,
            } => {
                let at = priority.map_or_else(String::new, |p| format!(" at priority {p}"));
                let what = format!(
                    "write {} {}{at}",
                    names::object(object),
                    props::name(property)
                );
                let r = self.client.write_property(
                    &device,
                    object,
                    PropertyRef::whole(property),
                    values,
                    priority,
                );
                (device.instance, what, r.map(|()| Some(object)))
            }
            Change::Create {
                device,
                kind,
                initial,
            } => {
                let what = format!("create {}", names::object_type(kind));
                let r = self
                    .client
                    .create_object(&device, Specifier::Type(kind), &initial);
                (device.instance, what, r.map(Some))
            }
            Change::Delete { device, object } => {
                let what = format!("delete {}", names::object(object));
                (
                    device.instance,
                    what,
                    self.client.delete_object(&device, object).map(|()| None),
                )
            }
        };
        let result = result.map_err(|e| match e {
            ReadError::Timeout => {
                "no reply: it may or may not have been applied; read it back".to_owned()
            }
            e => e.to_string(),
        });
        self.emit(Event::Changed {
            instance,
            what,
            structural,
            result,
        });
    }
}
