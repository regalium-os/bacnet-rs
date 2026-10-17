// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `bacnet.device.v1.DeviceService`: Who-Is, and the Device object.

use super::{Api, ApiError, Code, Known, name, next_token, no_filter, page};
use crate::scan::{self, ObjectReport};
use crate::{ObjectId, Value, explain};
use prost_types::Timestamp;
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
        let mut plan = self.plan.clone().with_range(range);
        if let Some(wait) = &req.wait {
            let wait =
                Duration::try_from(*wait).map_err(|_| ApiError::invalid("a negative wait"))?;
            plan = plan.with_wait(wait);
        }
        let found = self
            .client
            .discover(&plan)
            .map_err(|e| ApiError::new(Code::Unavailable, format!("Who-Is: {e}")))?;
        let mut devices = Vec::new();
        for device in found {
            let instance = device.instance;
            self.remember(device);
            devices.extend(self.known.get(&instance).map(|k| message(k, None, None)));
        }
        Ok(schema::DiscoverDevicesResponse { devices })
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
        let instance = name::parse_device(&req.name)?;
        let device = self.device(instance)?;
        let me = ObjectId::new(8, instance);
        let result = self
            .client
            .read_property(&device, me, 77, None)
            .map_err(|e| ApiError::from_read(&req.name, &e));
        self.answered(instance, &result);
        result?;
        let report =
            scan::read_many(&mut self.client, &device, &[(me, DEVICE_PROPS.to_vec())]).pop();
        let count = scan::object_count(&mut self.client, &device).ok();
        let known = self
            .known
            .get_mut(&instance)
            .ok_or_else(|| ApiError::new(Code::NotFound, "forgotten"))?;
        if let Some(r) = &report {
            known.name = r.name();
        }
        Ok(message(known, report.as_ref(), count))
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
        no_filter(&req.filter)?;
        if !req.order_by.is_empty() {
            return Err(ApiError::new(
                Code::Unimplemented,
                "devices are listed by instance only",
            ));
        }
        let (start, size) = page(&req.page_token, req.page_size, 1000)?;
        let total = self.known.len();
        let devices = self
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
    }

    /// `DeleteDevice`: forgets it. Nothing is sent to it.
    ///
    /// # Errors
    ///
    /// `NotFound` when it was not known.
    pub fn delete_device(&mut self, req: &schema::DeleteDeviceRequest) -> Result<(), ApiError> {
        let instance = name::parse_device(&req.name)?;
        self.known
            .remove(&instance)
            .map(drop)
            .ok_or_else(|| ApiError::new(Code::NotFound, format!("{} is not known", req.name)))
    }
}

/// A Device message from what is known, and what was just read if anything.
fn message(k: &Known, report: Option<&ObjectReport>, count: Option<u32>) -> schema::Device {
    let text = |p| match report.and_then(|r| r.get(p)) {
        Some([Value::Text(s)]) => s.clone(),
        _ => String::new(),
    };
    let number = |p| match report.and_then(|r| r.get(p)) {
        Some([Value::Unsigned(n)]) => i32::try_from(*n).ok(),
        _ => None,
    };
    let segmentation = match report.and_then(|r| r.get(107)) {
        Some([Value::Enumerated(s)]) => match s {
            0 => schema::Segmentation::Both,
            1 => schema::Segmentation::Transmit,
            2 => schema::Segmentation::Receive,
            _ => schema::Segmentation::None,
        },
        _ => schema::Segmentation::Unspecified,
    };
    let status = match report.and_then(|r| r.get(112)) {
        Some([Value::Enumerated(s)]) => {
            explain::enum_label(112, Some(8), *s).unwrap_or_else(|| format!("status-{s}"))
        }
        _ => String::new(),
    };
    let (network_number, mac_address) = k
        .device
        .route
        .as_ref()
        .map_or((0, Vec::new()), |r| (i32::from(r.net), r.mac.clone()));
    let display_name = match text(77) {
        s if s.is_empty() => k.name.clone(),
        s => s,
    };
    schema::Device {
        name: name::device(k.device.instance),
        instance: i32::try_from(k.device.instance).unwrap_or(i32::MAX),
        display_name,
        description: text(28),
        location: text(58),
        vendor: text(121),
        vendor_id: number(120).unwrap_or(i32::try_from(k.device.vendor).unwrap_or(0)),
        model: text(70),
        firmware_revision: text(44),
        application_software_version: text(12),
        protocol_revision: number(139).unwrap_or(0),
        max_apdu_length: number(62).unwrap_or(i32::try_from(k.device.max_apdu).unwrap_or(0)),
        segmentation: segmentation.into(),
        system_status: status,
        address: Some(schema::Address {
            datalink: schema::Datalink::BacnetIp.into(),
            endpoint: k.device.addr.to_string(),
            network_number,
            mac_address,
        }),
        state: if k.reachable {
            schema::DeviceState::Reachable
        } else {
            schema::DeviceState::Unreachable
        }
        .into(),
        object_count: count.and_then(|c| i32::try_from(c).ok()).unwrap_or(0),
        discover_time: Some(Timestamp::from(k.discovered)),
        last_seen_time: Some(Timestamp::from(k.seen)),
    }
}
