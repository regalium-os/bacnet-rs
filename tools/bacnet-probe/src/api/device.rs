// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `bacnet.device.v1.DeviceService`: Who-Is, and the Device object.

use super::device_message::message;
use super::{Api, ApiError, Code, name, next_token, no_filter, page};
use crate::ObjectId;
use crate::scan;
use regalium_bacnet_schema::device as schema;
use std::time::Duration;

/// What `GetDevice` reads from the Device object (§12.11).
const DEVICE_PROPS: [u32; 12] = [77, 28, 58, 121, 120, 70, 44, 12, 139, 62, 107, 112];

impl Api {
    /// `DiscoverDevices`: Who-Is, and every device that answers I-Am.
    ///
    /// # Errors
    ///
    /// `InvalidArgument` for half a range; `Unavailable` when the socket fails.
    pub fn discover_devices(
        &mut self,
        req: &schema::DiscoverDevicesRequest,
    ) -> Result<schema::DiscoverDevicesResponse, ApiError> {
        self.call("bacnet.api.DiscoverDevices", |api| {
            let instance =
                |i: i32| u32::try_from(i).map_err(|_| ApiError::invalid("a negative instance"));
            let range = match (req.min_instance, req.max_instance) {
                (Some(low), Some(high)) => Some((instance(low)?, instance(high)?)),
                (None, None) => None,
                _ => {
                    return Err(ApiError::invalid(
                        "Who-Is carries both limits of a range or neither",
                    ));
                }
            };
            let mut plan = api.plan.clone().with_range(range);
            if let Some(wait) = &req.wait {
                let wait =
                    Duration::try_from(*wait).map_err(|_| ApiError::invalid("a negative wait"))?;
                plan = plan.with_wait(wait);
            }
            let found = api
                .client
                .discover(&plan)
                .map_err(|e| ApiError::new(Code::Unavailable, format!("Who-Is: {e}")))?;
            let mut devices = Vec::new();
            for device in found {
                let instance = device.instance;
                api.remember(device);
                devices.extend(api.known.get(&instance).map(|k| message(k, None, None)));
            }
            Ok(schema::DiscoverDevicesResponse { devices })
        })
    }

    /// `GetDevice`: the Device object's properties, read now.
    ///
    /// # Errors
    ///
    /// `NotFound` when no such device answers; `Unavailable` when it is silent.
    pub fn get_device(
        &mut self,
        req: &schema::GetDeviceRequest,
    ) -> Result<schema::Device, ApiError> {
        self.call("bacnet.api.GetDevice", |api| {
            let instance = name::parse_device(&req.name)?;
            let device = api.device(instance)?;
            let me = ObjectId::new(8, instance);
            let result = api
                .client
                .read_property(&device, me, 77, None)
                .map_err(|e| ApiError::from_read(&req.name, &e));
            api.answered(instance, &result);
            result?;
            let report =
                scan::read_many(&mut api.client, &device, &[(me, DEVICE_PROPS.to_vec())]).pop();
            let count = scan::object_count(&mut api.client, &device).ok();
            if let Some(r) = &report {
                api.client.learn(crate::Identity::from_report(instance, r));
            }
            let known = api
                .known
                .get_mut(&instance)
                .ok_or_else(|| ApiError::new(Code::NotFound, "forgotten"))?;
            if let Some(r) = &report {
                known.name = r.name();
            }
            Ok(message(known, report.as_ref(), count))
        })
    }

    /// `ListDevices`: what discovery has found, reachable or not.
    ///
    /// # Errors
    ///
    /// `InvalidArgument` for a bad page token; `Unimplemented` for a filter.
    pub fn list_devices(
        &mut self,
        req: &schema::ListDevicesRequest,
    ) -> Result<schema::ListDevicesResponse, ApiError> {
        self.call("bacnet.api.ListDevices", |api| {
            no_filter(&req.filter)?;
            if !req.order_by.is_empty() {
                return Err(ApiError::new(
                    Code::Unimplemented,
                    "devices are listed by instance only",
                ));
            }
            let (start, size) = page(&req.page_token, req.page_size, 1000)?;
            let total = api.known.len();
            let devices = api
                .known
                .values()
                .skip(start)
                .take(size)
                .map(|k| message(k, None, None))
                .collect();
            Ok(schema::ListDevicesResponse {
                devices,
                next_page_token: next_token(start + size, total),
            })
        })
    }

    /// `DeleteDevice`: forgets it. Nothing is sent to it.
    ///
    /// # Errors
    ///
    /// `NotFound` when it was not known.
    pub fn delete_device(&mut self, req: &schema::DeleteDeviceRequest) -> Result<(), ApiError> {
        self.call("bacnet.api.DeleteDevice", |api| {
            let instance = name::parse_device(&req.name)?;
            api.known
                .remove(&instance)
                .map(drop)
                .ok_or_else(|| ApiError::new(Code::NotFound, format!("{} is not known", req.name)))
        })
    }
}
