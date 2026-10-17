// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `ReadProperty` and `ReadPropertyMultiple`.

use super::observe::Target;
use super::{Client, Device, ReadError};
use crate::service::rpm::{self, Results, Spec};
use crate::service::{PropertyRef, confirmed, read};
use crate::tag::{ObjectId, Value};

impl Client {
    /// Reads one property, or one element of an array property with `index`.
    ///
    /// # Errors
    ///
    /// See [`ReadError`]; a device that is offline is [`ReadError::Timeout`].
    pub fn read_property(
        &mut self,
        device: &Device,
        object: ObjectId,
        property: u32,
        index: Option<u32>,
    ) -> Result<Vec<Value>, ReadError> {
        let target = Target::of(device, object, Some(property));
        self.request("bacnet.client.readProperty", &target, |c| {
            let body = read::request(object, PropertyRef { property, index });
            let ack = c.confirm(device, confirmed::READ_PROPERTY, &body)?;
            read::decode_ack(&ack).ok_or(ReadError::Malformed)
        })
    }

    /// Reads many properties of many objects in one round trip. Each
    /// property carries its own value or error in the result.
    ///
    /// # Errors
    ///
    /// When the request as a whole fails. [`ReadError::unsupported`] says
    /// the device cannot do `ReadPropertyMultiple`, or not one this big;
    /// `scan::read_many` falls back on that.
    pub fn read_property_multiple(
        &mut self,
        device: &Device,
        specs: &[Spec],
    ) -> Result<Vec<Results>, ReadError> {
        let first = specs
            .first()
            .map_or(ObjectId::new(8, device.instance), |s| s.object);
        let target = Target::of(device, first, None);
        self.request("bacnet.client.readPropertyMultiple", &target, |c| {
            match c.confirm(
                device,
                confirmed::READ_PROPERTY_MULTIPLE,
                &rpm::request(specs),
            ) {
                Ok(ack) => rpm::decode_ack(&ack).ok_or(ReadError::Malformed),
                Err(e) => {
                    // unrecognized-service, or the services class saying the
                    // same: this device has no ReadPropertyMultiple at all.
                    if matches!(e, ReadError::Reject(9) | ReadError::Bacnet { class: 5, .. }) {
                        c.no_rpm.insert(device.instance);
                    }
                    Err(e)
                }
            }
        })
    }
}
