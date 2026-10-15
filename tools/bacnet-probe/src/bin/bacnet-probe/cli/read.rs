// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `read`: properties of one object, located with a Who-Is for its device's
//! instance alone. Several properties go in one `ReadPropertyMultiple` where
//! the device supports it. Unlike a scan, a requested property that fails is
//! shown, unknown-property included.

use super::{locate, parse_object, parse_property, table};
use crate::Result;
use crate::args::{Format, Net};
use bacnet_probe::json::{self, Json};
use bacnet_probe::service::PropertyRef;
use bacnet_probe::service::rpm::Spec;
use bacnet_probe::{Client, Device, ObjectId, Value, explain, names, property, props, scan};
use clap::Args;

#[derive(Args)]
pub(crate) struct ReadArgs {
    /// Device instance.
    device: u32,
    /// Object, e.g. analog-input:1, device:599 or 0:1.
    #[arg(value_parser = parse_object)]
    object: ObjectId,
    /// Properties by name or number [default: present-value].
    #[arg(value_parser = parse_property)]
    properties: Vec<u32>,
    /// Array index (0 reads the array's length).
    #[arg(long)]
    index: Option<u32>,
    /// Every standard property of the object.
    #[arg(long, conflicts_with = "properties")]
    all: bool,
    #[arg(long, value_enum, default_value_t)]
    format: Format,
}

type Reads = Vec<(u32, std::result::Result<Vec<Value>, String>)>;

pub(crate) fn read(client: &mut Client, net: &Net, args: &ReadArgs) -> Result {
    let device = locate(client, net, args.device)?;
    let wanted = match (&args.properties[..], args.all) {
        (_, true) => props::for_kind(args.object.kind),
        ([], false) => vec![property::PRESENT_VALUE],
        (some, false) => some.to_vec(),
    };
    let reads = fetch(client, &device, args.object, &wanted, args.index);
    let label = |p: u32| {
        args.index
            .map_or_else(|| props::name(p), |i| format!("{}[{i}]", props::name(p)))
    };
    let kind = args.object.kind;
    if args.format == Format::Json {
        let ok = reads
            .iter()
            .filter_map(|(p, r)| Some((label(*p), json::values(*p, kind, r.as_ref().ok()?))));
        let errors: Vec<(String, Json)> = reads
            .iter()
            .filter_map(|(p, r)| Some((label(*p), Json::str(r.as_ref().err()?.clone()))))
            .collect();
        let mut doc = vec![
            ("device", Json::UInt(device.instance.into())),
            ("object", Json::Str(names::object(args.object))),
            ("properties", Json::Obj(ok.collect())),
        ];
        if !errors.is_empty() {
            doc.push(("errors", Json::Obj(errors)));
        }
        println!("{}", Json::obj(doc).pretty());
    } else {
        // Units and state names make a bare present-value readable; fetch
        // them quietly (they are not shown unless asked for).
        let mut context = scan::object(client, &device, args.object, false);
        context.reads.extend(reads.iter().cloned());
        let (units, labels) = (context.units(), context.labels());
        let rows = reads
            .iter()
            .map(|(p, r)| {
                let value = match r {
                    Ok(v) if args.index.is_some() => v
                        .iter()
                        .map(|v| explain::one(Some(*p), Some(kind), v))
                        .collect::<Vec<_>>()
                        .join(", "),
                    Ok(v) => match (*p, v.as_slice()) {
                        (property::PRESENT_VALUE, [one]) => {
                            explain::present(kind, one, units, &labels)
                        }
                        _ => explain::many(*p, kind, v, units),
                    },
                    Err(e) => format!("<{e}>"),
                };
                vec![label(*p), value]
            })
            .collect();
        print!("{}", table::render(&["PROPERTY", "VALUE"], rows));
    }
    let failed = reads.iter().filter(|(_, r)| r.is_err()).count();
    if failed == reads.len() {
        Err(format!("{failed} of {} reads failed", reads.len()).into())
    } else {
        Ok(())
    }
}

/// One `ReadPropertyMultiple` when it can, one `ReadProperty` each when not.
fn fetch(
    client: &mut Client,
    device: &Device,
    object: ObjectId,
    wanted: &[u32],
    index: Option<u32>,
) -> Reads {
    if wanted.len() > 1 && client.supports_rpm(device.instance) {
        let spec = Spec {
            object,
            properties: wanted
                .iter()
                .map(|p| PropertyRef {
                    property: *p,
                    index,
                })
                .collect(),
        };
        if let Ok(results) = client.read_property_multiple(device, &[spec]) {
            return results
                .into_iter()
                .flat_map(|r| r.properties)
                .map(|(p, v)| {
                    (
                        p.property,
                        v.map_err(|(class, code)| names::error(class, code)),
                    )
                })
                .collect();
        }
    }
    wanted
        .iter()
        .map(|&p| {
            (
                p,
                client
                    .read_property(device, object, p, index)
                    .map_err(|e| e.to_string()),
            )
        })
        .collect()
}
