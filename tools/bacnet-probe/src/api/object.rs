// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `bacnet.object.v1.ObjectService`, reading: get, list, and Who-Has.

use super::object_message::object_message;
use super::{Api, ApiError, Code, name, next_token, no_filter, page, value};
use crate::scan;
use crate::service::who::Wanted;
use crate::{Device, ObjectId, Value, names};
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
        self.call("bacnet.api.GetObject", |api| {
            let (instance, id) = name::parse_object(&req.name)?;
            let device = api.device(instance)?;
            // A typed read first, so a missing object is NotFound and a silent
            // device Unavailable, rather than both arriving as error text.
            let result = api
                .client
                .read_property(&device, id, 77, None)
                .map_err(|e| ApiError::from_read(&req.name, &e));
            api.answered(instance, &result);
            result?;
            Ok(api.read_objects(&device, &[id]).pop().unwrap_or_default())
        })
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
        self.call("bacnet.api.ListObjects", |api| {
            no_filter(&req.filter)?;
            let instance = name::parse_device(&req.parent)?;
            let device = api.device(instance)?;
            let count = scan::object_count(&mut api.client, &device)
                .map_err(|e| ApiError::new(Code::Unavailable, e));
            api.answered(instance, &count);
            let total = usize::try_from(count?).unwrap_or(0);
            let (start, size) = page(&req.page_token, req.page_size, 1000)?;
            let end = (start + size).min(total);
            let first = u32::try_from(start + 1).unwrap_or(u32::MAX);
            let last = u32::try_from(end).unwrap_or(0);
            let ids = scan::object_ids_in(&mut api.client, &device, first, last)
                .map_err(|e| ApiError::new(Code::Unavailable, e))?;
            Ok(schema::ListObjectsResponse {
                objects: api.read_objects(&device, &ids),
                next_page_token: next_token(end, total),
            })
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
        self.call("bacnet.api.FindObjects", |api| {
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
            let mut plan = api.plan.clone().with_range(range);
            if let Some(wait) = wait {
                plan = plan.with_wait(wait);
            }
            let found = api
                .client
                .who_has(&plan, &wanted)
                .map_err(|e| ApiError::new(Code::Unavailable, format!("Who-Has: {e}")))?;
            let objects = found
                .into_iter()
                .map(|(device, have)| {
                    let instance = device.instance;
                    api.remember(device);
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
        })
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
