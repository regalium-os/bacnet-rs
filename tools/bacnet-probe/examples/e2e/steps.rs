// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! One cycle's steps, each in its own `bacnet.e2e.<step>` span. Values are
//! mock: a setpoint that follows a sine, a load that climbs and wraps, so a
//! dashboard shows something moving and a reader can tell which write made
//! which point change.

use super::Result;
use bacnet_probe::api::{Api, name, to_schema};
use bacnet_probe::scan::{self, Options};
use bacnet_probe::{Discovery, Value, metrics, sim};
use regalium_bacnet_schema::{device as dev, object as obj};
use regalium_bacnet_telemetry::Level;
use std::fmt::Display;
use std::time::Duration;

const DEVICE: u32 = 599;

/// Runs every step; returns a one-line summary for the cycle's log line.
pub(super) fn cycle(api: &mut Api, plan: &Discovery, cycle: u32) -> Result<String> {
    let devices = step(api, "bacnet.e2e.discover", |api| {
        let found = api.discover_devices(&dev::DiscoverDevicesRequest::default())?;
        api.get_device(&dev::GetDeviceRequest {
            name: name::device(DEVICE),
        })?;
        Ok(found.devices.len())
    })?;
    let points = step(api, "bacnet.e2e.poll", |api| poll(api, plan))?;
    let setpoint = step(api, "bacnet.e2e.write", |api| {
        super::change::write(api, cycle)
    })?;
    step(api, "bacnet.e2e.override", |api| {
        super::change::override_input(api, cycle)
    })?;
    step(api, "bacnet.e2e.refusal", super::change::refusal)?;
    step(api, "bacnet.e2e.lifecycle", |api| lifecycle(api, cycle))?;
    let changes = step(api, "bacnet.e2e.cov", cov)?;
    Ok(format!(
        "{devices} device(s), {points} point(s) polled, fan at {setpoint:.1} %, {changes} COV notification(s)"
    ))
}

/// Runs `f` inside a span named `name`, failed with its error if it fails.
fn step<T>(api: &mut Api, name: &'static str, f: impl FnOnce(&mut Api) -> Result<T>) -> Result<T> {
    let span = api.client().enter(name);
    let result = f(api);
    let error = result.as_ref().err().map(ToString::to_string);
    api.client()
        .exit(span, error.as_ref().map(|e| e as &dyn Display));
    result
}

/// Reads every point and records it as `bacnet export` does.
fn poll(api: &mut Api, plan: &Discovery) -> Result<usize> {
    let client = api.client();
    let mut points = 0;
    for device in client.discover(plan)? {
        let options = Options {
            detail: false,
            max_objects: 100,
            only: None,
        };
        let report = scan::device(client, &device, &options, |_, _| {});
        let identity = client.identity_or(device.instance, &report.this.name());
        metrics::record_device(
            client.meter(),
            &device,
            &identity,
            true,
            report.object_count,
        );
        for o in &report.objects {
            metrics::record_object(client.meter(), &identity, o);
        }
        points += report.objects.len();
    }
    Ok(points)
}

/// An object created, read and deleted.
fn lifecycle(api: &mut Api, cycle: u32) -> Result {
    let object = obj::Object {
        object_type: "analog-value".into(),
        display_name: format!("e2e cycle {cycle}"),
        present_value: Some(to_schema(&Value::Real(f32::from(
            u16::try_from(cycle).unwrap_or(u16::MAX),
        )))),
        ..obj::Object::default()
    };
    let made = api.create_object(&obj::CreateObjectRequest {
        parent: name::device(DEVICE),
        object: Some(object),
        object_id: String::new(),
    })?;
    api.get_object(&obj::GetObjectRequest {
        name: made.name.clone(),
    })?;
    api.delete_object(&obj::DeleteObjectRequest {
        name: made.name.clone(),
    })?;
    api.client().log(
        Level::Debug,
        &format_args!("created, read and deleted {}", made.name),
        &[],
    );
    Ok(())
}

/// A COV subscription to the operating mode, its notifications, and its
/// cancellation.
fn cov(api: &mut Api) -> Result<usize> {
    let found = api.discover_devices(&dev::DiscoverDevicesRequest::default())?;
    let endpoint = found
        .devices
        .first()
        .and_then(|d| d.address.as_ref())
        .map(|a| a.endpoint.clone())
        .ok_or("no device")?;
    let device = bacnet_probe::Device {
        instance: DEVICE,
        addr: endpoint.parse()?,
        route: None,
        max_apdu: 1476,
        vendor: 0,
    };
    let client = api.client();
    let process = client.subscribe_cov(&device, sim::MODE, 30)?;
    let notes = client.notifications(Duration::from_millis(1500))?;
    for n in &notes {
        client.log(
            Level::Debug,
            &format_args!(
                "COV from {}: {} value(s)",
                bacnet_probe::names::object(n.object),
                n.values.len()
            ),
            &[],
        );
    }
    client.cancel_cov(&device, sim::MODE, process)?;
    Ok(notes.len())
}
