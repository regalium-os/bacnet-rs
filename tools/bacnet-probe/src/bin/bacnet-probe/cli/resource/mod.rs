// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `get`, `list`, `write`, `create`, `delete`: the API's services from a
//! shell, addressed by resource name.
//!
//! Each verb is one or two API calls, and prints what the device said after
//! it, not what was asked for: a write to a commandable point reads back the
//! winner of its priority array, which is the value the plant is running on.

mod args;
pub(crate) mod input;
mod json;
mod render;
mod value;
mod write;

pub(crate) use args::{CreateArgs, NameArgs, WriteArgs};
pub(crate) use write::write;

use crate::Result;
use crate::args::Format;
use bacnet_probe::api::Api;
use regalium_bacnet_schema::device as dev;
use regalium_bacnet_schema::object as obj;

/// `get NAME`: a device, an object or a property, read now.
pub(crate) fn get(api: &mut Api, a: &NameArgs) -> Result {
    match a.name.matches('/').count() {
        1 => device(
            &api.get_device(&dev::GetDeviceRequest {
                name: a.name.clone(),
            })?,
            a.format,
        ),
        3 => object(
            &api.get_object(&obj::GetObjectRequest {
                name: a.name.clone(),
            })?,
            a.format,
        ),
        _ => property(
            &api.get_property(&obj::GetPropertyRequest {
                name: a.name.clone(),
            })?,
            a.format,
        ),
    }
    Ok(())
}

/// `list devices`, `list devices/599`, `list devices/599/objects/...`.
pub(crate) fn list(api: &mut Api, a: &NameArgs) -> Result {
    if a.name == "devices" {
        // A fresh process knows no devices, so listing them is discovering them.
        let found = api.discover_devices(&dev::DiscoverDevicesRequest::default())?;
        for heard in found.devices {
            // One read each for the name and the rest; a device that stops
            // answering between the two is still listed, from its I-Am.
            let read = api.get_device(&dev::GetDeviceRequest {
                name: heard.name.clone(),
            });
            device(&read.unwrap_or(heard), a.format);
        }
    } else if a.name.contains("/objects/") {
        let mut token = String::new();
        loop {
            let req = obj::ListPropertiesRequest {
                parent: a.name.clone(),
                page_size: 100,
                page_token: token,
            };
            let page = api.list_properties(&req)?;
            page.properties.iter().for_each(|p| property(p, a.format));
            token = page.next_page_token;
            if token.is_empty() {
                break;
            }
        }
    } else {
        let mut token = String::new();
        loop {
            let req = obj::ListObjectsRequest {
                parent: a.name.clone(),
                page_size: 50,
                page_token: token,
                ..Default::default()
            };
            let page = api.list_objects(&req)?;
            page.objects.iter().for_each(|o| object(o, a.format));
            token = page.next_page_token;
            if token.is_empty() {
                break;
            }
        }
    }
    Ok(())
}

/// `create PARENT --type T`: `CreateObject`, then the new object.
pub(crate) fn create(api: &mut Api, a: &CreateArgs) -> Result {
    let present_value = a
        .value
        .as_deref()
        .map(|v| input::parse(v, None, &[]))
        .transpose()?;
    let req = obj::CreateObjectRequest {
        parent: a.parent.clone(),
        object: Some(obj::Object {
            object_type: a.object_type.clone(),
            display_name: a.display_name.clone().unwrap_or_default(),
            present_value,
            ..Default::default()
        }),
        object_id: a.id.clone().unwrap_or_default(),
    };
    object(&api.create_object(&req)?, a.format);
    Ok(())
}

/// `delete NAME`: `DeleteObject`.
pub(crate) fn delete(api: &mut Api, a: &NameArgs) -> Result {
    api.delete_object(&obj::DeleteObjectRequest {
        name: a.name.clone(),
    })?;
    eprintln!("deleted {}", a.name);
    Ok(())
}

fn device(d: &dev::Device, f: Format) {
    show(f, &render::device_text(d), &json::device(d));
}

fn object(o: &obj::Object, f: Format) {
    show(f, &render::object_text(o), &json::object(o));
}

fn property(p: &obj::Property, f: Format) {
    show(f, &render::property_text(p), &json::property(p));
}

fn show(f: Format, text: &str, json: &bacnet_probe::json::Json) {
    match f {
        Format::Table => println!("{}", text.trim_end()),
        Format::Json => println!("{}", json.compact()),
    }
}
