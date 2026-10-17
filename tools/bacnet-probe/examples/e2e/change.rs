// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The steps that change the device: a setpoint written and read back, an
//! input overridden and restored, and a write the device must refuse.

use super::Result;
use bacnet_probe::api::{Api, Code, name, to_schema};
use bacnet_probe::{ObjectId, Value, sim};
use regalium_bacnet_schema::object as obj;
use regalium_bacnet_telemetry::Level;

const DEVICE: u32 = 599;

fn property(id: ObjectId, prop: u32, v: &Value, priority: i32) -> obj::UpdatePropertyRequest {
    let property = obj::Property {
        name: name::property(DEVICE, id, prop),
        value: Some(to_schema(v)),
        write_priority: priority,
        ..obj::Property::default()
    };
    obj::UpdatePropertyRequest {
        property: Some(property),
        update_mask: None,
    }
}

fn real(p: &obj::Property) -> Option<f32> {
    match p.value.as_ref()?.kind.as_ref()? {
        obj::value::Kind::RealValue(v) => Some(*v),
        _ => None,
    }
}

/// A mock setpoint, written at priority 8 and read back.
pub(super) fn write(api: &mut Api, cycle: u32) -> Result<f32> {
    let turn = f32::from(u16::try_from(cycle).unwrap_or(u16::MAX));
    let setpoint = 50.0 + 40.0 * (turn * 0.6).sin();
    let back = api.update_property(&property(sim::FAN, 85, &Value::Real(setpoint), 8))?;
    let read = real(&back).ok_or("the fan's present-value is not a REAL")?;
    if (read - setpoint).abs() > 0.01 {
        api.client().log(
            Level::Error,
            &format_args!("fan written {setpoint:.1} %, read back {read:.1} %"),
            &[],
        );
        return Err("read-back mismatch".into());
    }
    api.client().log(
        Level::Info,
        &format_args!("fan setpoint {setpoint:.1} % written at priority 8, read back {read:.1} %"),
        &[],
    );
    Ok(setpoint)
}

/// The load cell taken out of service, given a mock reading, and restored.
pub(super) fn override_input(api: &mut Api, cycle: u32) -> Result {
    let load = f32::from(u16::try_from(cycle * 85 % 1000).unwrap_or(0));
    api.update_property(&property(sim::CAR_LOAD, 81, &Value::Bool(true), 0))?;
    let back = api.update_property(&property(sim::CAR_LOAD, 85, &Value::Real(load), 0))?;
    api.client().log(
        Level::Info,
        &format_args!(
            "car load overridden to {load} kg, read back {}",
            real(&back).map_or_else(|| "no REAL".to_owned(), |v| format!("{v} kg"))
        ),
        &[],
    );
    api.update_property(&property(sim::CAR_LOAD, 81, &Value::Bool(false), 0))?;
    Ok(())
}

/// A write the device must refuse: an input that is in service.
pub(super) fn refusal(api: &mut Api) -> Result {
    match api.update_property(&property(sim::MACHINE_ROOM, 85, &Value::Real(0.0), 0)) {
        Err(e) if e.code == Code::FailedPrecondition => {
            api.client().log(
                Level::Info,
                &"the in-service input refused the write, as it must",
                &[],
            );
            Ok(())
        }
        Err(e) => Err(e.into()),
        Ok(_) => Err("an input in service accepted a write".into()),
    }
}
