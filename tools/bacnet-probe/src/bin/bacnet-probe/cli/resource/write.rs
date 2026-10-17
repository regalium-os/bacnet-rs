// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `write`: typed by what the property holds, sent as `WriteProperty`, and
//! answered with what the device runs on afterwards.

use super::{WriteArgs, input, property, render};
use crate::Result;
use crate::args::Format;
use bacnet_probe::api::{Api, name};
use regalium_bacnet_schema::object as obj;

/// `write NAME VALUE [--priority N]`: `WriteProperty`, then the read-back.
pub(crate) fn write(api: &mut Api, a: &WriteArgs) -> Result {
    let target = if a.name.contains("/properties/") {
        a.name.clone()
    } else {
        format!("{}/properties/present-value", a.name)
    };
    let (instance, id, prop) = name::parse_property(&target)?;
    // What the property holds now decides how the text is read, and an
    // object's states name its values.
    let parent = api.get_object(&obj::GetObjectRequest {
        name: name::object(instance, id),
    })?;
    let current = if prop == 85 {
        parent.present_value.clone()
    } else {
        api.get_property(&obj::GetPropertyRequest {
            name: target.clone(),
        })?
        .value
    };
    let states = if prop == 85 {
        parent.state_texts.clone()
    } else {
        Vec::new()
    };
    let value = input::parse(&a.value, current.as_ref(), &states)?;
    let req = obj::UpdatePropertyRequest {
        property: Some(obj::Property {
            name: target,
            value: Some(value.clone()),
            write_priority: a.priority.map_or(0, i32::from),
            ..Default::default()
        }),
        update_mask: None,
    };
    let after = match api.update_property(&req) {
        Err(e) if prop == 85 && e.bacnet == Some((2, 40)) && !parent.isolated => {
            return Err(format!("{e}\nhint: an input follows its sensor; write {}/properties/out-of-service true first", name::object(instance, id)).into());
        }
        other => other?,
    };
    if prop == 85 && a.format == Format::Table {
        // The value with its meaning: units, or the state's name.
        let now = obj::Object {
            present_value: after.value.clone(),
            ..parent
        };
        println!("present-value = {}", render::present_text(&now));
    } else {
        property(&after, a.format);
    }
    let stands = after.value.as_ref().map(|v| &v.kind) == Some(&value.kind);
    if !stands
        && !matches!(value.kind, Some(obj::value::Kind::NullValue(_)))
        && a.format == Format::Table
    {
        eprintln!(
            "note: written, but a higher-priority slot holds the value; `get {}` shows the priority array",
            name::object(instance, id)
        );
    }
    Ok(())
}
