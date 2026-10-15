// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `whois` (Who-Is: which devices are there) and `find` (Who-Has: which
//! device holds this object).

use super::{parse_object, parse_range, table};
use crate::Result;
use crate::args::{Format, Net};
use bacnet_probe::json::Json;
use bacnet_probe::service::who::Wanted;
use bacnet_probe::{Client, Device, metrics, names};
use clap::Args;

#[derive(Args)]
pub(crate) struct WhoisArgs {
    /// Only instances in LOW-HIGH (or exactly N).
    #[arg(long, value_parser = parse_range)]
    range: Option<(u32, u32)>,
    #[arg(long, value_enum, default_value_t)]
    format: Format,
}

#[derive(Args)]
pub(crate) struct FindArgs {
    /// An object-name ("Car 1 load"), or an identifier (analog-input:1).
    what: String,
    /// Only devices with instances in LOW-HIGH (or exactly N).
    #[arg(long, value_parser = parse_range)]
    range: Option<(u32, u32)>,
    #[arg(long, value_enum, default_value_t)]
    format: Format,
}

pub(crate) fn whois(client: &mut Client, net: &Net, args: &WhoisArgs) -> Result {
    let plan = super::plan(net)?.with_range(args.range);
    let devices = client.discover(&plan)?;
    metrics::record_discovery(client.meter(), devices.len());
    if args.format == Format::Json {
        println!(
            "{}",
            Json::Arr(devices.iter().map(device_json).collect()).pretty()
        );
        return Ok(());
    }
    let rows = devices
        .iter()
        .map(|d| {
            vec![
                d.instance.to_string(),
                d.addr.to_string(),
                route(d),
                d.max_apdu.to_string(),
                d.vendor.to_string(),
            ]
        })
        .collect();
    print!(
        "{}",
        table::render(
            &["DEVICE", "ADDRESS", "ROUTE", "MAX-APDU", "VENDOR-ID"],
            rows
        )
    );
    eprintln!(
        "{} device(s) from {} datagram(s)",
        devices.len(),
        plan.len()
    );
    Ok(())
}

pub(crate) fn find(client: &mut Client, net: &Net, args: &FindArgs) -> Result {
    let wanted =
        parse_object(&args.what).map_or_else(|_| Wanted::Name(args.what.clone()), Wanted::Object);
    let found = client.who_has(&super::plan(net)?.with_range(args.range), &wanted)?;
    if args.format == Format::Json {
        let list = found.iter().map(|(d, have)| {
            Json::obj([
                ("device", device_json(d)),
                ("object", Json::Str(names::object(have.object))),
                ("name", Json::str(have.name.clone())),
            ])
        });
        println!("{}", Json::Arr(list.collect()).pretty());
    } else {
        let rows = found
            .iter()
            .map(|(d, h)| {
                vec![
                    d.instance.to_string(),
                    d.addr.to_string(),
                    names::object(h.object),
                    h.name.clone(),
                ]
            })
            .collect();
        print!(
            "{}",
            table::render(&["DEVICE", "ADDRESS", "OBJECT", "NAME"], rows)
        );
    }
    if found.is_empty() {
        return Err(format!("no device answered Who-Has for {:?}", args.what).into());
    }
    Ok(())
}

fn route(d: &Device) -> String {
    d.route.as_ref().map_or_else(
        || "direct".into(),
        |r| format!("net {} mac {:02x?}", r.net, r.mac),
    )
}

pub(crate) fn device_json(d: &Device) -> Json {
    let route = d.route.as_ref().map_or(Json::Null, |r| {
        let mac: String = bacnet_probe::dissect::hex(&r.mac);
        Json::obj([("net", Json::UInt(r.net.into())), ("mac", Json::Str(mac))])
    });
    Json::obj([
        ("instance", Json::UInt(d.instance.into())),
        ("address", Json::Str(d.addr.to_string())),
        ("route", route),
        ("max_apdu", Json::UInt(d.max_apdu)),
        ("vendor_id", Json::UInt(d.vendor)),
    ])
}
