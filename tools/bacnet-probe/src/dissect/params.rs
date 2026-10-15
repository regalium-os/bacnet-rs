// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Walks a service body tag by tag, naming each from the `schema` and
//! interpreting each value by the property it belongs to.

use super::Cursor;
use super::schema::{Service, Shape};
use crate::tag::{self, ObjectId, Value};
use crate::{explain, names, props};

pub(super) fn walk(c: &mut Cursor<'_>, service: Service) -> Option<()> {
    let mut path: Vec<u8> = Vec::new();
    let mut positions: Vec<usize> = vec![0];
    let (mut object, mut property) = (None::<ObjectId>, None::<u32>);
    while c.pos < c.buf.len() {
        let (h, n) = tag::header(c.buf.get(c.pos..)?)?;
        if h.opening {
            let (name, _) = service.context(&path, h.number);
            c.take(n, format!("[{}] opening tag {{ {name}", h.number))?;
            c.depth += 1;
            path.push(h.number);
            positions.push(0);
            continue;
        }
        if h.closing {
            c.depth = c.depth.saturating_sub(1);
            path.pop();
            positions.pop();
            c.take(n, format!("[{}] closing tag }}", h.number))?;
            continue;
        }
        let total = n.checked_add(h.len)?;
        let data = c.peek(total)?.get(n..)?;
        let text = if h.context {
            let (name, shape) = service.context(&path, h.number);
            let meaning = match shape {
                Shape::Object => {
                    let id = ObjectId::from_raw(u32::from_be_bytes(data.try_into().ok()?));
                    object = Some(id);
                    names::object(id)
                }
                Shape::Property => {
                    let id = u32::try_from(tag::be(data)).ok()?;
                    property = Some(id);
                    format!("{} ({id})", props::name(id))
                }
                Shape::Unsigned if name == "property-array-index" && tag::be(data) == 0 => {
                    "0: element 0 is the array's length".into()
                }
                Shape::Unsigned if data.len() <= 8 => tag::be(data).to_string(),
                Shape::Bool => (data.first().copied().unwrap_or(0) != 0).to_string(),
                Shape::Text => format!("{:?}", tag::text(data)),
                _ => format!("{data:02x?}"),
            };
            format!("ctx [{}] len {}: {name} = {meaning}", h.number, h.len)
        } else {
            let (value, _) = tag::decode(c.peek(total)?)?;
            let position = positions.last().copied().unwrap_or(0);
            let name = service.positional(&path, position);
            let meaning = match (name, &value) {
                (Some("segmentation-supported"), v) => explain::one(Some(107), None, v),
                (Some("error-class"), Value::Enumerated(e)) => names::error_class(*e),
                (Some("error-code"), Value::Enumerated(e)) => names::error_code(*e),
                (_, v) => explain::one(property, object.map(|o| o.kind), v),
            };
            if let Some(p) = positions.last_mut() {
                *p += 1;
            }
            format!(
                "app {} len {}: {} = {meaning}",
                app_type(h.number),
                h.len,
                name.unwrap_or("value")
            )
        };
        c.take(total, text)?;
    }
    Some(())
}

fn app_type(n: u8) -> &'static str {
    const TYPES: [&str; 13] = [
        "Null",
        "Boolean",
        "Unsigned",
        "Signed",
        "Real",
        "Double",
        "OctetString",
        "CharacterString",
        "BitString",
        "Enumerated",
        "Date",
        "Time",
        "ObjectIdentifier",
    ];
    TYPES.get(usize::from(n)).unwrap_or(&"reserved")
}
