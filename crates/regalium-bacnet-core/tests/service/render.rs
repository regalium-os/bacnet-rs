// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! How the service corpora spell a decoded service: names where the
//! standard has them, numbers where it does not, values as the tag corpus
//! spells them.

use regalium_bacnet_core::enumerated::{Segmentation, object_type, property};
use regalium_bacnet_core::primitive::{ObjectIdentifier, Primitive};
use regalium_bacnet_core::service::{ReadProperty, ReadPropertyAck, Service};
use regalium_bacnet_core::tag::Lvt;
use regalium_bacnet_core::{Error, ErrorKind};

pub(crate) fn hex(b: &[u8]) -> String {
    use std::fmt::Write;
    b.iter().fold(String::new(), |mut out, x| {
        let _ = write!(out, "{x:02x}");
        out
    })
}

pub(crate) fn error(e: &Error<'_>) -> String {
    match e.kind {
        ErrorKind::Truncated { needed, available } => {
            format!("error truncated {needed}/{available}")
        }
        ErrorKind::MissingField(255) => "error missing-field application".into(),
        ErrorKind::MissingField(n) => format!("error missing-field {n}"),
        ErrorKind::Trailing => "error trailing".into(),
        ErrorKind::Unbalanced => "error unbalanced".into(),
        ErrorKind::InvalidValue { tag } => format!("error invalid-value {tag}"),
        ErrorKind::WrongLength { tag, len } => format!("error wrong-length {tag}/{len}"),
        other => format!("error {other:?}"),
    }
}

fn object(id: ObjectIdentifier) -> String {
    let kind = object_type(id.object_type)
        .map_or_else(|| format!("type-{}", id.object_type), str::to_owned);
    format!("{kind}:{}", id.instance)
}

fn reference(r: &ReadProperty) -> String {
    let prop =
        property(r.property).map_or_else(|| format!("property-{}", r.property), str::to_owned);
    let index = r.index.map(|i| format!(" index {i}")).unwrap_or_default();
    format!("{} {prop}{index}", object(r.object))
}

fn primitive(p: &Primitive<'_>) -> String {
    match p {
        Primitive::Null => "null".into(),
        Primitive::Boolean(b) => format!("boolean {b}"),
        Primitive::Unsigned(v) => format!("unsigned {v}"),
        Primitive::Signed(v) => format!("signed {v}"),
        Primitive::Real(v) => format!("real {v:?}"),
        Primitive::Double(v) => format!("double {v:?}"),
        Primitive::Enumerated(v) => format!("enumerated {v}"),
        Primitive::CharacterString(s) => format!("string {:?}", s.text().unwrap_or_default()),
        Primitive::BitString(b) => format!(
            "bits {}",
            (0..b.len())
                .map(|i| if b.get(i) == Some(true) { '1' } else { '0' })
                .collect::<String>()
        ),
        Primitive::ObjectIdentifier(id) => format!("object {}", object(*id)),
        other => format!("{other:?}"),
    }
}

fn value(ack: &ReadPropertyAck<'_>) -> String {
    let all: Vec<String> = ack
        .values()
        .map(|e| match e {
            Err(e) => error(&e),
            Ok(e) => match e.header.lvt {
                Lvt::Opening => format!("[{}]{{", e.header.number),
                Lvt::Closing => format!("}}[{}]", e.header.number),
                _ if e.header.class == regalium_bacnet_core::tag::Class::Context => {
                    format!("[{}] {}", e.header.number, hex(e.content))
                }
                _ => Primitive::decode(&e).map_or_else(|e| error(&e), |p| primitive(&p)),
            },
        })
        .collect();
    if all.is_empty() {
        "(empty)".into()
    } else {
        all.join(", ")
    }
}

pub(crate) fn service(s: &Service<'_>) -> String {
    match s {
        Service::WhoIs(w) => match w.range {
            None => "who-is all".into(),
            Some((low, high)) => format!("who-is {low}-{high}"),
        },
        Service::IAm(i) => {
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
        Service::ReadProperty(r) => format!("read-property {}", reference(r)),
        Service::ReadPropertyAck(a) => {
            format!("read-property-ack {} = {}", reference(&a.read), value(a))
        }
        Service::Error { error, .. } => format!("error read-property: {}", error.describe()),
        Service::Undecoded => "undecoded".into(),
    }
}
