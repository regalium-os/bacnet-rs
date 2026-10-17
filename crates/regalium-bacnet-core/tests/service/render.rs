// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! How the service corpora spell the parts of a service: names where the
//! standard has them, numbers where it does not, values as the tag corpus
//! spells them. `services.rs` puts the parts together.

use regalium_bacnet_core::enumerated::{object_type, property};
use regalium_bacnet_core::primitive::{CharacterString, ObjectIdentifier, Primitive};
use regalium_bacnet_core::service::{PropertyReference, PropertyValue, ReadProperty, ServiceError};
use regalium_bacnet_core::tag::{Class, Lvt, elements};
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

pub(crate) fn object(id: ObjectIdentifier) -> String {
    let kind = object_type(id.object_type)
        .map_or_else(|| format!("type-{}", id.object_type), str::to_owned);
    format!("{kind}:{}", id.instance)
}

/// `present-value`, or `priority-array index 8`.
pub(crate) fn prop(r: &PropertyReference) -> String {
    let name =
        property(r.property).map_or_else(|| format!("property-{}", r.property), str::to_owned);
    let index = r.index.map(|i| format!(" index {i}")).unwrap_or_default();
    format!("{name}{index}")
}

pub(crate) fn text(s: &CharacterString<'_>) -> String {
    format!("{:?}", s.text().unwrap_or_default())
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
        Primitive::CharacterString(s) => format!("string {}", text(s)),
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

/// A value's elements, as the tag corpus spells them.
pub(crate) fn value(octets: &[u8]) -> String {
    let all: Vec<String> = elements(octets)
        .map(|e| match e {
            Err(e) => error(&e),
            Ok(e) => match e.header.lvt {
                Lvt::Opening => format!("[{}]{{", e.header.number),
                Lvt::Closing => format!("}}[{}]", e.header.number),
                _ if e.header.class == Class::Context => {
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

/// `present-value = real 21.5 priority 8`.
pub(crate) fn property_value(v: &PropertyValue<'_>) -> String {
    let priority = v
        .priority
        .map(|p| format!(" priority {p}"))
        .unwrap_or_default();
    format!("{} = {}{priority}", prop(&v.property), value(v.value))
}

pub(crate) fn values(all: &[PropertyValue<'_>]) -> String {
    all.iter()
        .map(property_value)
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn service_error(e: ServiceError) -> String {
    e.describe()
}

/// The confirmed services an Error PDU names, by number.
pub(crate) fn confirmed_name(n: u8) -> String {
    match n {
        1 => "cov-notification".into(),
        2 => "event-notification".into(),
        5 => "subscribe-cov".into(),
        11 => "delete-object".into(),
        12 => "read-property".into(),
        14 => "read-property-multiple".into(),
        15 => "write-property".into(),
        28 => "subscribe-cov-property".into(),
        n => format!("service-{n}"),
    }
}

pub(crate) fn reference(r: &ReadProperty) -> String {
    let index = r.index.map(|i| format!(" index {i}")).unwrap_or_default();
    let name = super::render::prop(&regalium_bacnet_core::service::PropertyReference::new(
        r.property,
    ));
    format!("{} {name}{index}", object(r.object))
}

pub(crate) fn range(r: Option<(u32, u32)>) -> String {
    r.map(|(l, h)| format!("{l}-{h} ")).unwrap_or_default()
}
