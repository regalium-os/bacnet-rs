// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! How the service corpora spell a whole decoded service.

use super::events::event;
use super::render::{
    confirmed_name, object, prop, property_value, range, reference, service_error, text, value,
    values,
};
use regalium_bacnet_core::enumerated::{Segmentation, object_type};
use regalium_bacnet_core::service::{CovNotification, ObjectSpecifier, Service, Sought};

pub(crate) fn service(s: &Service<'_>) -> String {
    match s {
        Service::WhoIs(w) => match w.range {
            None => "who-is all".into(),
            Some((low, high)) => format!("who-is {low}-{high}"),
        },
        Service::IAm(i) => i_am(i),
        Service::WhoHas(w) => match w.object {
            Sought::Identifier(id) => format!("who-has {}object {}", range(w.range), object(id)),
            Sought::Name(n) => format!("who-has {}name {}", range(w.range), text(&n)),
        },
        Service::IHave(i) => format!(
            "i-have {} {} {}",
            object(i.device),
            object(i.object),
            text(&i.name)
        ),
        Service::ReadProperty(r) => format!("read-property {}", reference(r)),
        Service::ReadPropertyAck(a) => format!(
            "read-property-ack {} = {}",
            reference(&a.read),
            value(a.value)
        ),
        Service::ReadPropertyMultiple(r) => {
            let all: Vec<_> = r
                .reads
                .iter()
                .map(|a| {
                    format!(
                        "{} [{}]",
                        object(a.object),
                        a.properties.iter().map(prop).collect::<Vec<_>>().join(", ")
                    )
                })
                .collect();
            format!("read-property-multiple {}", all.join("; "))
        }
        Service::ReadPropertyMultipleAck(a) => rpm_ack(a),
        Service::WriteProperty(w) => format!(
            "write-property {} {}",
            object(w.object),
            property_value(&w.value)
        ),
        Service::WritePropertyMultiple(w) => {
            let all: Vec<_> = w
                .writes
                .iter()
                .map(|a| format!("{} {}", object(a.object), values(&a.values)))
                .collect();
            format!("write-property-multiple {}", all.join("; "))
        }
        Service::SubscribeCov(s) => subscribe(s),
        Service::SubscribeCovProperty(s) => {
            let inc = s
                .increment
                .map(|i| format!(" increment {i:?}"))
                .unwrap_or_default();
            let mut base = service(&Service::SubscribeCov(s.subscription));
            base = base.replacen("subscribe-cov", "subscribe-cov-property", 1);
            format!("{base} property {}{inc}", prop(&s.property))
        }
        Service::CovNotification {
            confirmed,
            notification: n,
        } => cov(*confirmed, n),
        Service::EventNotification {
            confirmed,
            notification,
        } => event(*confirmed, notification),
        Service::CreateObject(c) => create(c),
        Service::CreateObjectAck(o) => format!("create-object-ack {}", object(*o)),
        Service::DeleteObject(o) => format!("delete-object {}", object(*o)),
        Service::Error { service, error } => format!(
            "error {}: {}",
            confirmed_name(*service),
            service_error(*error)
        ),
        Service::CreateObjectError(e) => format!(
            "create-object-error {} at {}",
            service_error(e.error),
            e.first_failed
        ),
        Service::WriteMultipleError(e) => format!(
            "write-property-multiple-error {} at {} {}",
            service_error(e.error),
            object(e.first_failed.object),
            prop(&e.first_failed.property)
        ),
        _ => "undecoded".into(),
    }
}

fn i_am(i: &regalium_bacnet_core::service::IAm) -> String {
    let seg = match i.segmentation {
        Segmentation::Both => "both".into(),
        Segmentation::Transmit => "transmit".into(),
        Segmentation::Receive => "receive".into(),
        Segmentation::NoSegmentation => "no-segmentation".into(),
        Segmentation::Other(n) => n.to_string(),
    };
    format!(
        "i-am {} max-apdu {} segmentation {seg} vendor {}",
        object(i.device),
        i.max_apdu,
        i.vendor_id
    )
}

fn rpm_ack(a: &regalium_bacnet_core::service::ReadPropertyMultipleAck<'_>) -> String {
    let all: Vec<_> = a
        .objects
        .iter()
        .map(|o| {
            let results: Vec<_> = o
                .results
                .iter()
                .map(|r| match r.result {
                    Ok(v) => format!("{} = {}", prop(&r.property), value(v)),
                    Err(e) => {
                        format!("{} = error {}", prop(&r.property), service_error(e))
                    }
                })
                .collect();
            format!("{} {}", object(o.object), results.join(", "))
        })
        .collect();
    format!("read-property-multiple-ack {}", all.join("; "))
}

fn subscribe(s: &regalium_bacnet_core::service::SubscribeCov) -> String {
    use std::fmt::Write;
    let mut out = format!("subscribe-cov process {} {}", s.process, object(s.object));
    if s.is_cancellation() {
        out.push_str(" cancel");
    }
    if let Some(c) = s.confirmed {
        let _ = write!(out, " confirmed {c}");
    }
    if let Some(l) = s.lifetime {
        let _ = write!(out, " lifetime {l}");
    }
    out
}

fn create(c: &regalium_bacnet_core::service::CreateObject<'_>) -> String {
    let what = match c.object {
        ObjectSpecifier::Type(t) => {
            format!(
                "type {}",
                object_type(t).map_or_else(|| format!("type-{t}"), str::to_owned)
            )
        }
        ObjectSpecifier::Identifier(id) => object(id),
    };
    if c.values.is_empty() {
        format!("create-object {what}")
    } else {
        format!("create-object {what} with {}", values(&c.values))
    }
}

fn cov(confirmed: bool, n: &CovNotification<'_>) -> String {
    format!(
        "cov-notification {} process {} {} {} remaining {}: {}",
        if confirmed {
            "confirmed"
        } else {
            "unconfirmed"
        },
        n.process,
        object(n.device),
        object(n.object),
        n.time_remaining,
        values(&n.values)
    )
}
