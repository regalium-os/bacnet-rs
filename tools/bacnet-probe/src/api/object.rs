// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `bacnet.object.v1.ObjectService`, reading: get, list, and Who-Has.

use super::{Api, ApiError, Code, name, next_token, no_filter, page, value};
use crate::scan::{self, ObjectReport};
use crate::service::who::Wanted;
use crate::{Device, ObjectId, Value, explain, names};
use prost_types::Timestamp;
use regalium_bacnet_schema::object as schema;
use std::time::{Duration, SystemTime};

/// What an Object carries: name, description, the value and how it is
/// commanded, units, the names of its states, and its health.
pub(super) const OBJECT_PROPS: [u32; 12] = [77, 28, 85, 87, 117, 4, 46, 110, 111, 36, 103, 81];

impl Api {
    /// `GetObject`: the object's properties, read now.
    ///
    /// # Errors
    ///
    /// `NotFound` for an unknown device or object; `Unavailable` when silent.
    pub fn get_object(
        &mut self,
        req: &schema::GetObjectRequest,
    ) -> Result<schema::Object, ApiError> {
        let (instance, id) = name::parse_object(&req.name)?;
        let device = self.device(instance)?;
        // A typed read first, so a missing object is NotFound and a silent
        // device Unavailable, rather than both arriving as error text.
        let result = self
            .client
            .read_property(&device, id, 77, None)
            .map_err(|e| ApiError::from_read(&req.name, &e));
        self.answered(instance, &result);
        result?;
        Ok(self.read_objects(&device, &[id]).pop().unwrap_or_default())
    }

    /// `ListObjects`: one page of the device's object-list, each object read.
    ///
    /// # Errors
    ///
    /// As [`Api::get_object`], and `Unimplemented` for a filter.
    pub fn list_objects(
        &mut self,
        req: &schema::ListObjectsRequest,
    ) -> Result<schema::ListObjectsResponse, ApiError> {
        no_filter(&req.filter)?;
        let instance = name::parse_device(&req.parent)?;
        let device = self.device(instance)?;
        let count = scan::object_count(&mut self.client, &device)
            .map_err(|e| ApiError::new(Code::Unavailable, e));
        self.answered(instance, &count);
        let total = usize::try_from(count?).unwrap_or(0);
        let (start, size) = page(&req.page_token, req.page_size, 1000)?;
        let end = (start + size).min(total);
        let first = u32::try_from(start + 1).unwrap_or(u32::MAX);
        let last = u32::try_from(end).unwrap_or(0);
        let ids = scan::object_ids_in(&mut self.client, &device, first, last)
            .map_err(|e| ApiError::new(Code::Unavailable, e))?;
        Ok(schema::ListObjectsResponse {
            objects: self.read_objects(&device, &ids),
            next_page_token: next_token(end, total),
        })
    }

    /// `FindObjects`: Who-Has, by identifier or name, of one device or all.
    ///
    /// # Errors
    ///
    /// `InvalidArgument` without a target; `Unavailable` when the socket fails.
    pub fn find_objects(
        &mut self,
        req: &schema::FindObjectsRequest,
    ) -> Result<schema::FindObjectsResponse, ApiError> {
        use schema::find_objects_request::Target;
        let range = match req.parent.as_str() {
            "devices/-" => None,
            parent => name::parse_device(parent).map(|i| Some((i, i)))?,
        };
        let wanted = match &req.target {
            Some(Target::ObjectId(id)) => match value::from_schema(&schema::Value {
                kind: Some(schema::value::Kind::ObjectIdValue(id.clone())),
                encoded: Vec::new(),
            })?
            .as_slice()
            {
                [Value::Object(id)] => Wanted::Object(*id),
                _ => return Err(ApiError::invalid("an object identifier")),
            },
            Some(Target::DisplayName(n)) => Wanted::Name(n.clone()),
            None => {
                return Err(ApiError::invalid(
                    "Who-Has needs an object_id or a display_name",
                ));
            }
        };
        let wait = req
            .wait
            .map(Duration::try_from)
            .transpose()
            .map_err(|_| ApiError::invalid("a negative wait"))?;
        let mut plan = self.plan.clone().with_range(range);
        if let Some(wait) = wait {
            plan = plan.with_wait(wait);
        }
        let found = self
            .client
            .who_has(&plan, &wanted)
            .map_err(|e| ApiError::new(Code::Unavailable, format!("Who-Has: {e}")))?;
        let objects = found
            .into_iter()
            .map(|(device, have)| {
                let instance = device.instance;
                self.remember(device);
                schema::Object {
                    name: name::object(instance, have.object),
                    object_type: names::object_type(have.object.kind),
                    instance: i32::try_from(have.object.instance).unwrap_or(i32::MAX),
                    display_name: have.name,
                    ..schema::Object::default()
                }
            })
            .collect();
        Ok(schema::FindObjectsResponse { objects })
    }

    /// Reads [`OBJECT_PROPS`] of each object, batched.
    pub(super) fn read_objects(
        &mut self,
        device: &Device,
        ids: &[ObjectId],
    ) -> Vec<schema::Object> {
        let wanted: Vec<(ObjectId, Vec<u32>)> =
            ids.iter().map(|id| (*id, OBJECT_PROPS.to_vec())).collect();
        let now = SystemTime::now();
        wanted
            .chunks(scan::BATCH)
            .flat_map(|chunk| scan::read_many(&mut self.client, device, chunk))
            .map(|r| object_message(device.instance, &r, now))
            .collect()
    }
}

/// An Object message from what was read.
pub(super) fn object_message(instance: u32, r: &ObjectReport, read: SystemTime) -> schema::Object {
    let text = |p| match r.get(p) {
        Some([Value::Text(s)]) => s.clone(),
        _ => String::new(),
    };
    let label = |p, kind: Option<u16>| match r.get(p) {
        Some([Value::Enumerated(e)]) => explain::enum_label(p, kind, *e)
            .unwrap_or_else(|| format!("{}-{e}", crate::props::name(p))),
        _ => String::new(),
    };
    let status_flags = match r.get(111) {
        Some([Value::Bits(b)]) => Some(schema::StatusFlags {
            alarm: b.first().copied().unwrap_or(false),
            fault: b.get(1).copied().unwrap_or(false),
            overridden: b.get(2).copied().unwrap_or(false),
            isolated: b.get(3).copied().unwrap_or(false),
        }),
        _ => None,
    };
    schema::Object {
        name: name::object(instance, r.id),
        object_type: names::object_type(r.id.kind),
        instance: i32::try_from(r.id.instance).unwrap_or(i32::MAX),
        display_name: r.name(),
        description: text(28),
        present_value: r.get(85).map(value::values_to_schema),
        write_priority: 0,
        priority_array: r
            .get(87)
            .map(|s| s.iter().map(value::to_schema).collect())
            .unwrap_or_default(),
        units: r
            .units()
            .map(|u| names::unit_label(u).unwrap_or_else(|| format!("units-{u}")))
            .unwrap_or_default(),
        state_texts: r
            .labels()
            .iter()
            .filter_map(|v| match v {
                Value::Text(s) => Some(s.clone()),
                _ => None,
            })
            .collect(),
        status_flags,
        event_state: label(36, Some(r.id.kind)),
        reliability: label(103, Some(r.id.kind)),
        isolated: r.get(81) == Some(&[Value::Bool(true)]),
        read_time: Some(Timestamp::from(read)),
    }
}
