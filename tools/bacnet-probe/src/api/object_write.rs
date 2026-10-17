// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `bacnet.object.v1.ObjectService`, changing: create, update, delete.
//!
//! An update is `WritePropertyMultiple` where the device has it and one
//! `WriteProperty` per field where it does not. `BACnet` has no transaction,
//! so a write that fails part-way leaves the fields before it written; the
//! error names the field that refused.

use super::{Api, ApiError, name, value};
use crate::service::object::Specifier;
use crate::service::write::PropertyValue;
use crate::{ObjectId, Value};
use regalium_bacnet_schema::object as schema;

/// The fields an update may write, and the property each is.
const WRITABLE: [(&str, u32); 4] = [
    ("display_name", 77),
    ("description", 28),
    ("present_value", 85),
    ("isolated", 81),
];

impl Api {
    /// `CreateObject`: `CreateObject` (§15.3), its fields as initial values.
    ///
    /// # Errors
    ///
    /// `InvalidArgument` for a bad type or id; the device's refusal otherwise,
    /// e.g. `Unimplemented` for a fixed object database.
    pub fn create_object(
        &mut self,
        req: &schema::CreateObjectRequest,
    ) -> Result<schema::Object, ApiError> {
        let instance = name::parse_device(&req.parent)?;
        let object = req
            .object
            .as_ref()
            .ok_or_else(|| ApiError::invalid("an object to create"))?;
        let kind = name::parse_object_type(&object.object_type).ok_or_else(|| {
            ApiError::invalid(format!("{:?} is not an object type", object.object_type))
        })?;
        let what = if req.object_id.is_empty() {
            Specifier::Type(kind)
        } else {
            let id = name::parse_object_segment(&req.object_id).ok_or_else(|| {
                ApiError::invalid(format!("{:?} is not an object id", req.object_id))
            })?;
            if id.kind != kind {
                return Err(ApiError::invalid(
                    "object_id names a different type from object_type",
                ));
            }
            Specifier::Id(id)
        };
        let initial = writes(object, &[])?;
        let device = self.device(instance)?;
        let created = self
            .client
            .create_object(&device, what, &initial)
            .map_err(|e| ApiError::from_read(&req.parent, &e));
        self.answered(instance, &created);
        let id = created?;
        Ok(self.read_objects(&device, &[id]).pop().unwrap_or_default())
    }

    /// `UpdateObject`: the masked fields, written in one request where the
    /// device can take it.
    ///
    /// # Errors
    ///
    /// `InvalidArgument` for an unknown mask path; the first refusal otherwise.
    pub fn update_object(
        &mut self,
        req: &schema::UpdateObjectRequest,
    ) -> Result<schema::Object, ApiError> {
        let object = req
            .object
            .as_ref()
            .ok_or_else(|| ApiError::invalid("an object to update"))?;
        let (instance, id) = name::parse_object(&object.name)?;
        let paths: Vec<&str> = req
            .update_mask
            .as_ref()
            .map(|m| m.paths.iter().map(String::as_str).collect())
            .unwrap_or_default();
        let values = writes(object, &paths)?;
        if values.is_empty() {
            return Err(ApiError::invalid(
                "nothing to write: set a field, or name it in update_mask",
            ));
        }
        let device = self.device(instance)?;
        let result = self
            .write_all(&device, id, values)
            .map_err(|e| ApiError::from_read(&object.name, &e));
        self.answered(instance, &result);
        result?;
        Ok(self.read_objects(&device, &[id]).pop().unwrap_or_default())
    }

    /// `DeleteObject`: `DeleteObject` (§15.4).
    ///
    /// # Errors
    ///
    /// The device's refusal, e.g. `FailedPrecondition` for an object its
    /// configuration depends on.
    pub fn delete_object(&mut self, req: &schema::DeleteObjectRequest) -> Result<(), ApiError> {
        let (instance, id) = name::parse_object(&req.name)?;
        let device = self.device(instance)?;
        let result = self
            .client
            .delete_object(&device, id)
            .map_err(|e| ApiError::from_read(&req.name, &e));
        self.answered(instance, &result);
        result
    }

    fn write_all(
        &mut self,
        device: &crate::Device,
        id: ObjectId,
        values: Vec<PropertyValue>,
    ) -> Result<(), crate::ReadError> {
        match self
            .client
            .write_property_multiple(device, &[(id, values.clone())])
        {
            Err(e) if e.unsupported() => {
                for v in values {
                    self.client
                        .write_property(device, id, v.property, v.values, v.priority)?;
                }
                Ok(())
            }
            other => other,
        }
    }
}

/// The writes `object` asks for: the fields `paths` names, or with no paths
/// every writable field that is set (a false `isolated` counts as unset).
fn writes(object: &schema::Object, paths: &[&str]) -> Result<Vec<PropertyValue>, ApiError> {
    if let Some(bad) = paths
        .iter()
        .find(|p| !WRITABLE.iter().any(|(w, _)| w == *p))
    {
        return Err(ApiError::invalid(format!(
            "{bad:?} is not a writable field; these are: display_name, description, present_value, isolated"
        )));
    }
    let mut out = Vec::new();
    for (field, prop) in WRITABLE {
        let set = match field {
            "display_name" => !object.display_name.is_empty(),
            "description" => !object.description.is_empty(),
            "present_value" => object.present_value.is_some(),
            _ => object.isolated,
        };
        // A mask, when given, is the whole of what is written (AIP-134).
        if !(if paths.is_empty() {
            set
        } else {
            paths.contains(&field)
        }) {
            continue;
        }
        let values = match field {
            "display_name" => vec![Value::Text(object.display_name.clone())],
            "description" => vec![Value::Text(object.description.clone())],
            "present_value" => match &object.present_value {
                Some(v) => value::from_schema(v)?,
                None => {
                    return Err(ApiError::invalid(
                        "present_value is in the mask but not set",
                    ));
                }
            },
            _ => vec![Value::Bool(object.isolated)],
        };
        let priority = (prop == 85 && object.write_priority != 0)
            .then(|| u8::try_from(object.write_priority))
            .transpose()
            .map_err(|_| ApiError::invalid("write_priority is 1 to 16"))?;
        out.push(PropertyValue {
            priority,
            ..PropertyValue::new(prop, values)
        });
    }
    Ok(out)
}
