// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Reading a device's objects into plain data that any front end (CLI table,
//! JSON, TUI, metrics) can render. Nothing here prints.
//!
//! Reads go through `ReadPropertyMultiple` where the device has it, batched
//! (`batch`), and fall back to one `ReadProperty` at a time where it does not.

mod batch;
mod report;

pub use batch::{BATCH, object_ids, object_ids_in, read, read_many};
pub use report::ObjectReport;

use crate::client::{Client, Device};
use crate::tag::{ObjectId, Value};
use crate::{names, property, props};

/// Everything read from one device.
#[derive(Clone, Debug)]
pub struct DeviceReport {
    /// The device.
    pub device: Device,
    /// Its device object.
    pub this: ObjectReport,
    /// Length of its object-list, if read.
    pub object_count: Option<u32>,
    /// Every other object read.
    pub objects: Vec<ObjectReport>,
    /// The last thing that went wrong, if anything did.
    pub problem: Option<String>,
}

/// What [`device`] reads.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Every standard property, not just the summary.
    pub detail: bool,
    /// Read at most this many objects.
    pub max_objects: u32,
    /// Read only this object, in detail.
    pub only: Option<ObjectId>,
}

/// The device object's properties worth a summary.
pub const DEVICE_SUMMARY: [u32; 8] = crate::identity::IDENTITY_PROPS;

/// Reads a whole device. `progress(done, of)` is called as objects arrive.
pub fn device(
    client: &mut Client,
    device: &Device,
    opts: &Options,
    mut progress: impl FnMut(u32, u32),
) -> DeviceReport {
    let me = ObjectId::new(8, device.instance);
    let device_props = if opts.detail {
        props::for_kind(8)
    } else {
        DEVICE_SUMMARY.to_vec()
    };
    let this = read_many(client, device, &[(me, device_props)])
        .pop()
        .unwrap_or_default();
    // From here on, every request to it is labelled with who it is.
    client.learn(crate::Identity::from_report(device.instance, &this));
    let mut report = DeviceReport {
        device: device.clone(),
        this,
        object_count: None,
        objects: Vec::new(),
        problem: None,
    };
    if let Some(only) = opts.only {
        report.objects.push(object(client, device, only, true));
        return report;
    }
    let count = match object_count(client, device) {
        Ok(n) => n,
        Err(e) => {
            report.problem = Some(e);
            return report;
        }
    };
    report.object_count = Some(count);
    let ids = match object_ids(client, device, count.min(opts.max_objects)) {
        Ok(ids) => ids,
        Err(e) => {
            report.problem = Some(e);
            return report;
        }
    };
    let ids: Vec<ObjectId> = ids.into_iter().filter(|id| *id != me).collect();
    let total = u32::try_from(ids.len()).unwrap_or(u32::MAX);
    for chunk in ids.chunks(BATCH) {
        let wanted: Vec<(ObjectId, Vec<u32>)> = chunk
            .iter()
            .map(|id| (*id, wanted(id.kind, opts.detail)))
            .collect();
        report.objects.extend(read_many(client, device, &wanted));
        progress(
            u32::try_from(report.objects.len()).unwrap_or(u32::MAX),
            total,
        );
    }
    report
}

/// The length of the device's object-list. Index 0 of an array is its
/// length (§12.1); reading element by element keeps every reply small
/// enough to need no segmentation.
///
/// # Errors
///
/// When the device does not answer or has no object-list.
pub fn object_count(client: &mut Client, device: &Device) -> Result<u32, String> {
    let me = ObjectId::new(8, device.instance);
    match client
        .read_property(device, me, property::OBJECT_LIST, Some(0))
        .as_deref()
    {
        Ok([Value::Unsigned(n)]) => {
            u32::try_from(*n).map_err(|_| format!("object-list length {n} is not believable"))
        }
        Ok(other) => Err(format!("object-list length: unexpected {other:?}")),
        Err(e) => Err(format!("object-list: {e}")),
    }
}

/// Reads the summary set of properties for `id`, or every standard one.
pub fn object(client: &mut Client, device: &Device, id: ObjectId, detail: bool) -> ObjectReport {
    read_many(client, device, &[(id, wanted(id.kind, detail))])
        .pop()
        .unwrap_or(ObjectReport {
            id,
            reads: Vec::new(),
        })
}

fn wanted(kind: u16, detail: bool) -> Vec<u32> {
    if detail {
        props::for_kind(kind)
    } else {
        summary_props(kind)
    }
}

/// Name, then what is needed to render present-value well, then health.
#[must_use]
pub fn summary_props(kind: u16) -> Vec<u32> {
    let mut wanted = vec![property::OBJECT_NAME];
    if names::is_point(kind) {
        match kind {
            k if names::is_analog(k) => wanted.push(property::UNITS),
            3..=5 => wanted.extend([4, 46]),
            13 | 14 | 19 => wanted.push(110),
            _ => {}
        }
        wanted.extend([property::PRESENT_VALUE, 111, 36, 103]);
    }
    wanted
}
