// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! A `Device` message from what discovery found and what was just read.

use super::{Known, name};
use crate::Value;
use crate::explain;
use crate::scan::ObjectReport;
use prost_types::Timestamp;
use regalium_bacnet_schema::device as schema;

/// A Device message from what is known, and what was just read if anything.
pub(super) fn message(
    k: &Known,
    report: Option<&ObjectReport>,
    count: Option<u32>,
) -> schema::Device {
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
