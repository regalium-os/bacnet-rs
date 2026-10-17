// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! [`Memory`]: a database held in memory, enough to make a local device of
//! a gateway, a simulator or a test.

use super::Database;
use super::errors::{INVALID_ARRAY_INDEX, UNKNOWN_OBJECT};
use super::point::{Point, RELINQUISH_DEFAULT, unsigned};
use crate::enumerated::Segmentation;
use crate::primitive::{CharacterString, ObjectIdentifier, Primitive};
use crate::service::ServiceError;
use std::collections::BTreeMap;

/// object-identifier, object-list, object-name, object-type.
const OBJECT_IDENTIFIER: u32 = 75;
const OBJECT_LIST: u32 = 76;
const OBJECT_NAME: u32 = 77;
const OBJECT_TYPE: u32 = 79;

/// The Device object's type.
const DEVICE: u16 = 8;

/// An object database in memory.
///
/// Every object has object-identifier, object-name and object-type from
/// the moment it is added; the Device object has object-list, kept by the
/// database, and what §12.11 requires of a device that only answers.
/// Everything else is set by the caller, and only what it marks writable
/// can be written.
///
/// Plain data: `Send` and `Sync`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Memory {
    device: ObjectIdentifier,
    objects: BTreeMap<ObjectIdentifier, Point>,
    /// Object-list order: the device, then each object as it was added.
    order: Vec<ObjectIdentifier>,
}

impl Memory {
    /// A device `instance` named `name`, of vendor `vendor_id`: 1476-octet
    /// APDUs, no segmentation, protocol revision 22.
    #[must_use]
    pub fn new(instance: u32, name: &str, vendor_id: u32) -> Self {
        let device = ObjectIdentifier {
            object_type: DEVICE,
            instance,
        };
        let mut db = Self {
            device,
            objects: BTreeMap::new(),
            order: Vec::new(),
        };
        db = db.object(device, name);
        for (property, value) in [
            (120, Primitive::Unsigned(vendor_id.into())),
            (62, Primitive::Unsigned(1476)),
            (
                107,
                Primitive::Enumerated(Segmentation::NoSegmentation.to_u32()),
            ),
            (98, Primitive::Unsigned(1)),
            (139, Primitive::Unsigned(22)),
            (112, Primitive::Enumerated(0)),
        ] {
            db = db.with(device, property, &value);
        }
        db
    }

    /// Adds `object`, named `name`.
    #[must_use]
    pub fn object(mut self, object: ObjectIdentifier, name: &str) -> Self {
        let mut point = Point::default();
        point.set(OBJECT_IDENTIFIER, &Primitive::ObjectIdentifier(object));
        point.set(
            OBJECT_TYPE,
            &Primitive::Enumerated(object.object_type.into()),
        );
        point.set(
            OBJECT_NAME,
            &Primitive::CharacterString(CharacterString {
                charset: 0,
                bytes: name.as_bytes(),
            }),
        );
        if self.objects.insert(object, point).is_none() {
            self.order.push(object);
        }
        self
    }

    /// Sets `property` of `object`, which must have been added.
    #[must_use]
    pub fn with(mut self, object: ObjectIdentifier, property: u32, value: &Primitive<'_>) -> Self {
        self.set(object, property, value);
        self
    }

    /// Lets a write change `property` of `object`.
    #[must_use]
    pub fn writable(mut self, object: ObjectIdentifier, property: u32) -> Self {
        if let Some(p) = self.objects.get_mut(&object) {
            p.writable.insert(property);
        }
        self
    }

    /// Makes `object`'s present-value commandable: sixteen priority slots,
    /// and `relinquish` when all are empty (§19.2).
    #[must_use]
    pub fn commandable(mut self, object: ObjectIdentifier, relinquish: &Primitive<'_>) -> Self {
        if let Some(p) = self.objects.get_mut(&object) {
            p.slots = Some(Default::default());
            p.set(RELINQUISH_DEFAULT, relinquish);
        }
        self
    }

    /// Sets `property` of `object` in place — the device's own value moving,
    /// as a sensor's does. Ignored for an object never added.
    pub fn set(&mut self, object: ObjectIdentifier, property: u32, value: &Primitive<'_>) {
        if let Some(p) = self.objects.get_mut(&object) {
            p.set(property, value);
        }
    }

    fn point(&self, object: ObjectIdentifier) -> Result<&Point, ServiceError> {
        self.objects.get(&object).ok_or(UNKNOWN_OBJECT)
    }
}

impl Database for Memory {
    fn device(&self) -> ObjectIdentifier {
        self.device
    }

    fn objects(&self) -> Vec<ObjectIdentifier> {
        self.order.clone()
    }

    fn properties(&self, object: ObjectIdentifier) -> Result<Vec<u32>, ServiceError> {
        let mut all = self.point(object)?.names();
        if object == self.device {
            all.push(OBJECT_LIST);
            all.sort_unstable();
        }
        Ok(all)
    }

    fn read(
        &self,
        object: ObjectIdentifier,
        property: u32,
        index: Option<u32>,
    ) -> Result<Vec<u8>, ServiceError> {
        let point = self.point(object)?;
        if object != self.device || property != OBJECT_LIST {
            return point.read(property, index);
        }
        let id = |o: &ObjectIdentifier| {
            let mut out = Vec::new();
            Primitive::ObjectIdentifier(*o).encode(&mut out);
            out
        };
        match index {
            None => Ok(self.order.iter().flat_map(id).collect()),
            Some(0) => Ok(unsigned(
                u32::try_from(self.order.len()).unwrap_or(u32::MAX),
            )),
            Some(i) => {
                let o = self
                    .order
                    .get(usize::try_from(i - 1).unwrap_or(usize::MAX))
                    .ok_or(INVALID_ARRAY_INDEX)?;
                Ok(id(o))
            }
        }
    }

    fn write(
        &mut self,
        object: ObjectIdentifier,
        property: u32,
        index: Option<u32>,
        value: &[u8],
        priority: Option<u8>,
    ) -> Result<(), ServiceError> {
        let point = self.objects.get_mut(&object).ok_or(UNKNOWN_OBJECT)?;
        point.write(property, index, value, priority)
    }
}
