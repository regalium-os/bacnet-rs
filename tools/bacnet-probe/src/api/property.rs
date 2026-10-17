// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `bacnet.object.v1.PropertyService`: any property, read or written.

use super::{Api, ApiError, name, next_token, page, value};
use crate::scan::{self, ObjectReport};
use crate::service::PropertyRef;
use crate::{ObjectId, props};
use prost_types::Timestamp;
use regalium_bacnet_schema::object as schema;
use std::time::SystemTime;

impl Api {
    /// `GetProperty`: `ReadProperty` (§15.5).
    ///
    /// # Errors
    ///
    /// `NotFound` for an unknown object or property; `Unavailable` when silent.
    pub fn get_property(
        &mut self,
        req: &schema::GetPropertyRequest,
    ) -> Result<schema::Property, ApiError> {
        let (instance, id, prop) = name::parse_property(&req.name)?;
        let device = self.device(instance)?;
        let result = self
            .client
            .read_property(&device, id, prop, None)
            .map_err(|e| ApiError::from_read(&req.name, &e));
        self.answered(instance, &result);
        Ok(schema::Property {
            name: name::property(instance, id, prop),
            value: Some(value::values_to_schema(&result?)),
            read_time: Some(Timestamp::from(SystemTime::now())),
            ..schema::Property::default()
        })
    }

    /// `ListProperties`: every standard property the object's type has,
    /// those the device does not implement left out, any other refusal kept.
    ///
    /// # Errors
    ///
    /// As [`Api::get_property`].
    pub fn list_properties(
        &mut self,
        req: &schema::ListPropertiesRequest,
    ) -> Result<schema::ListPropertiesResponse, ApiError> {
        let (instance, id) = name::parse_object(&req.parent)?;
        let all = props::for_kind(id.kind);
        let (start, size) = page(&req.page_token, req.page_size, 1000)?;
        let end = (start + size).min(all.len());
        let wanted = all.get(start..end).unwrap_or_default().to_vec();
        let report = self.read_props(instance, id, &wanted)?;
        Ok(schema::ListPropertiesResponse {
            properties: messages(instance, &report, &wanted, false),
            next_page_token: next_token(end, all.len()),
        })
    }

    /// `BatchGetProperties`: `ReadPropertyMultiple` (§15.7), in the order
    /// asked, each with its value or the error that replaced it.
    ///
    /// # Errors
    ///
    /// `InvalidArgument` for a name outside `parent`; as [`Api::get_property`].
    pub fn batch_get_properties(
        &mut self,
        req: &schema::BatchGetPropertiesRequest,
    ) -> Result<schema::BatchGetPropertiesResponse, ApiError> {
        let (instance, id) = name::parse_object(&req.parent)?;
        let wanted = req
            .names
            .iter()
            .map(|n| match name::parse_property(n)? {
                (i, o, p) if i == instance && o == id => Ok(p),
                _ => Err(ApiError::invalid(format!(
                    "{n} is not beneath {}",
                    req.parent
                ))),
            })
            .collect::<Result<Vec<u32>, _>>()?;
        let report = self.read_props(instance, id, &wanted)?;
        Ok(schema::BatchGetPropertiesResponse {
            properties: messages(instance, &report, &wanted, true),
        })
    }

    /// `UpdateProperty`: `WriteProperty` (§15.9), at `write_priority` for a
    /// commandable present-value. Returns the value read back, which for a
    /// commandable object is the winner of its priority array — not
    /// necessarily what was written.
    ///
    /// # Errors
    ///
    /// `InvalidArgument` for a mask naming anything but `value`; the
    /// device's refusal otherwise.
    pub fn update_property(
        &mut self,
        req: &schema::UpdatePropertyRequest,
    ) -> Result<schema::Property, ApiError> {
        let property = req
            .property
            .as_ref()
            .ok_or_else(|| ApiError::invalid("a property to update"))?;
        if let Some(mask) = &req.update_mask
            && mask.paths.iter().any(|p| p != "value")
        {
            return Err(ApiError::invalid("only value is writable"));
        }
        let (instance, id, prop) = name::parse_property(&property.name)?;
        let values = value::from_schema(
            property
                .value
                .as_ref()
                .ok_or_else(|| ApiError::invalid("a value to write"))?,
        )?;
        let priority = match property.write_priority {
            0 => None,
            p => Some(
                u8::try_from(p)
                    .ok()
                    .filter(|p| (1..=16).contains(p))
                    .ok_or_else(|| ApiError::invalid("write_priority is 1 to 16"))?,
            ),
        };
        let device = self.device(instance)?;
        let result = self
            .client
            .write_property(&device, id, PropertyRef::whole(prop), values, priority)
            .map_err(|e| ApiError::from_read(&property.name, &e));
        self.answered(instance, &result);
        result?;
        self.get_property(&schema::GetPropertyRequest {
            name: property.name.clone(),
        })
    }

    fn read_props(
        &mut self,
        instance: u32,
        id: ObjectId,
        wanted: &[u32],
    ) -> Result<ObjectReport, ApiError> {
        let device = self.device(instance)?;
        // Object-name first, typed, so a missing object is NotFound.
        let probe = self
            .client
            .read_property(&device, id, 77, None)
            .map_err(|e| ApiError::from_read(&name::object(instance, id), &e));
        self.answered(instance, &probe);
        probe?;
        Ok(
            scan::read_many(&mut self.client, &device, &[(id, wanted.to_vec())])
                .pop()
                .unwrap_or(ObjectReport {
                    id,
                    reads: Vec::new(),
                }),
        )
    }
}

/// Properties from a report. With `every`, a property the device left out
/// is listed as unknown-property rather than dropped, since it was asked for.
fn messages(instance: u32, r: &ObjectReport, wanted: &[u32], every: bool) -> Vec<schema::Property> {
    let now = Some(Timestamp::from(SystemTime::now()));
    wanted
        .iter()
        .filter_map(|&p| {
            let read = r.reads.iter().find(|(q, _)| *q == p).map(|(_, v)| v);
            let (value, error) = match read {
                Some(Ok(v)) => (Some(value::values_to_schema(v)), None),
                Some(Err(e)) => (None, Some(error_from(e))),
                None if every => (None, Some(error_from("property: unknown-property"))),
                None => return None,
            };
            Some(schema::Property {
                name: name::property(instance, r.id, p),
                value,
                write_priority: 0,
                error,
                read_time: now,
            })
        })
        .collect()
}

/// `class: code`, as the scan records a refusal, into the schema's error.
fn error_from(text: &str) -> schema::PropertyError {
    let (class, code) = text.split_once(": ").unwrap_or(("communication", text));
    schema::PropertyError {
        error_class: class.to_owned(),
        error_code: code.to_owned(),
    }
}
