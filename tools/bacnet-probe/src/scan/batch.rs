// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Batched reads, with `ReadPropertyMultiple` where it works and a graceful
//! descent to `ReadProperty` where it does not.
//!
//! A device without `ReadPropertyMultiple` says so once (a reject or a
//! services error) and is then read singly for the rest of the session. A
//! request too big for the device's APDU or segmentation is split in half
//! and retried, down to a single property, rather than failed.

use super::ObjectReport;
use crate::client::{Client, Device, ReadError};
use crate::names;
use crate::service::PropertyRef;
use crate::service::rpm::Spec;
use crate::tag::{ObjectId, Value};

/// Objects per `ReadPropertyMultiple` when scanning. Eight summary reads fit a
/// 480-octet MS/TP APDU with room to spare.
pub const BATCH: usize = 8;

/// Object-list elements per `ReadPropertyMultiple`.
const INDICES: u32 = 24;

/// Reads `wanted` properties of each object, through `ReadPropertyMultiple`
/// when the device supports it. Results come back in request order;
/// unknown-property is omitted, any other failure kept per property.
pub fn read_many(
    client: &mut Client,
    device: &Device,
    wanted: &[(ObjectId, Vec<u32>)],
) -> Vec<ObjectReport> {
    if !client.supports_rpm(device.instance)
        || wanted.iter().map(|(_, p)| p.len()).sum::<usize>() <= 1
    {
        return wanted
            .iter()
            .map(|(id, props)| read(client, device, *id, props))
            .collect();
    }
    let specs: Vec<Spec> = wanted
        .iter()
        .map(|(id, props)| Spec {
            object: *id,
            properties: props.iter().map(|p| PropertyRef::whole(*p)).collect(),
        })
        .collect();
    match client.read_property_multiple(device, &specs) {
        Ok(results) => results
            .into_iter()
            .map(|r| ObjectReport {
                id: r.object,
                reads: r
                    .properties
                    .into_iter()
                    .filter_map(|(p, v)| Some((p.property, keep(v)?)))
                    .collect(),
            })
            .collect(),
        Err(e) if e.unsupported() => match wanted {
            [(id, props)] => {
                let (a, b) = props.split_at(props.len() / 2);
                let mut halves = read_many(client, device, &[(*id, a.to_vec())]);
                let rest = read_many(client, device, &[(*id, b.to_vec())]);
                if let (Some(first), Some(second)) = (halves.first_mut(), rest.into_iter().next()) {
                    first.reads.extend(second.reads);
                }
                halves
            }
            many => {
                let (a, b) = many.split_at(many.len() / 2);
                let mut out = read_many(client, device, a);
                out.extend(read_many(client, device, b));
                out
            }
        },
        Err(e) => wanted
            .iter()
            .map(|(id, props)| ObjectReport {
                id: *id,
                reads: props.iter().map(|p| (*p, Err(e.to_string()))).collect(),
            })
            .collect(),
    }
}

/// A per-property result, dropping unknown-property: optional and simply
/// not implemented, not worth a line.
fn keep(result: Result<Vec<Value>, (u32, u32)>) -> Option<Result<Vec<Value>, String>> {
    match result {
        Ok(v) => Some(Ok(v)),
        Err((2, 32)) => None,
        Err((class, code)) => Some(Err(names::error(class, code))),
    }
}

/// Reads `wanted` from `id`, one `ReadProperty` each.
pub fn read(client: &mut Client, device: &Device, id: ObjectId, wanted: &[u32]) -> ObjectReport {
    let mut reads = Vec::new();
    for &prop in wanted {
        match client.read_property(device, id, prop, None) {
            Ok(values) => reads.push((prop, Ok(values))),
            Err(ReadError::Bacnet { class: 2, code: 32 }) => {}
            Err(e) => reads.push((prop, Err(e.to_string()))),
        }
    }
    ObjectReport { id, reads }
}

/// The first `count` entries of the device's object-list, read by index,
/// [`INDICES`] at a time through `ReadPropertyMultiple` where possible.
///
/// # Errors
///
/// When an entry cannot be read.
pub fn object_ids(
    client: &mut Client,
    device: &Device,
    count: u32,
) -> Result<Vec<ObjectId>, String> {
    object_ids_in(client, device, 1, count)
}

/// Entries `first..=last` of the device's object-list (1-based), read as
/// [`object_ids`] reads them: one page of a listing, without the rest.
///
/// # Errors
///
/// When an entry cannot be read.
pub fn object_ids_in(
    client: &mut Client,
    device: &Device,
    first: u32,
    end: u32,
) -> Result<Vec<ObjectId>, String> {
    let me = ObjectId::new(8, device.instance);
    let mut ids = Vec::new();
    let mut next = first.max(1);
    while next <= end {
        let last = end.min(next + INDICES - 1);
        let refs: Vec<PropertyRef> = (next..=last)
            .map(|i| PropertyRef {
                property: 76,
                index: Some(i),
            })
            .collect();
        let batch = if client.supports_rpm(device.instance) {
            client
                .read_property_multiple(
                    device,
                    &[Spec {
                        object: me,
                        properties: refs,
                    }],
                )
                .ok()
        } else {
            None
        };
        let got: Vec<ObjectId> = batch
            .into_iter()
            .flatten()
            .flat_map(|r| r.properties)
            .filter_map(|(_, v)| match v.ok()?.as_slice() {
                [Value::Object(id)] => Some(*id),
                _ => None,
            })
            .collect();
        if u32::try_from(got.len()).ok() == Some(last - next + 1) {
            ids.extend(got);
        } else {
            for i in next..=last {
                match client.read_property(device, me, 76, Some(i)).as_deref() {
                    Ok([Value::Object(id)]) => ids.push(*id),
                    Ok(other) => return Err(format!("object-list[{i}]: unexpected {other:?}")),
                    Err(e) => return Err(format!("object-list[{i}]: {e}")),
                }
            }
        }
        next = last + 1;
    }
    Ok(ids)
}
