// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! What a request is about: its device, object and property, as a span's
//! attributes and as the words of a log line.

use super::Device;
use crate::{ObjectId, names, props};
use regalium_bacnet_telemetry::{keys, trace_attrs};

/// What a request is about, as its span records it.
pub(crate) struct Target {
    pub(super) device: u32,
    object_type: String,
    object_instance: u32,
    property: String,
}

trace_attrs! { Target => {
    device: keys::DEVICE_INSTANCE,
    object_type: keys::OBJECT_TYPE,
    object_instance: keys::OBJECT_INSTANCE,
    property: keys::PROPERTY,
} }

impl std::fmt::Display for Target {
    /// `analog-output:1 present-value on 599`, for a log line.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.object_type, self.object_instance)?;
        if !self.property.is_empty() {
            write!(f, " {}", self.property)?;
        }
        write!(f, " on {}", self.device)
    }
}

impl Target {
    /// One property of one object; `property` 0 when the request is about
    /// the object as a whole.
    pub(crate) fn of(device: &Device, object: ObjectId, property: Option<u32>) -> Self {
        Self {
            device: device.instance,
            object_type: names::object_type(object.kind),
            object_instance: object.instance,
            property: property.map(props::name).unwrap_or_default(),
        }
    }
}
