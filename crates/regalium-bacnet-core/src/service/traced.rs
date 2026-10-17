// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The service layer's spans, `bacnet.service.<service>`: one per decoded
//! service, saying what was asked of which object — the object, property
//! and device — and never the value read.

use super::confirmed::READ_PROPERTY;
use super::unconfirmed::{I_AM, WHO_IS};
use super::{ReadProperty, Service};
use crate::apdu::{Apdu, Pdu};
use crate::enumerated::{error_class, error_code, object_type, property};
use crate::error::Error;
use regalium_bacnet_telemetry::{Attribute, Span, Tracer, keys};

/// The span a service gets, chosen from the APDU before decoding so a
/// failed decode is recorded under the service that failed.
const fn span_name(apdu: &Apdu<'_>) -> Option<&'static str> {
    Some(match apdu.pdu {
        Pdu::UnconfirmedRequest {
            service: WHO_IS, ..
        } => "bacnet.service.who_is",
        Pdu::UnconfirmedRequest { service: I_AM, .. } => "bacnet.service.i_am",
        Pdu::ConfirmedRequest {
            service: READ_PROPERTY,
            ..
        } => "bacnet.service.read_property",
        Pdu::ComplexAck {
            service: READ_PROPERTY,
            ..
        } => "bacnet.service.read_property_ack",
        Pdu::Error {
            service: READ_PROPERTY,
            ..
        } => "bacnet.service.error",
        _ => return None,
    })
}

fn named(name: Option<&'static str>, prefix: &str, n: u32) -> String {
    name.map_or_else(|| format!("{prefix}-{n}"), str::to_owned)
}

fn reference(span: &Span<'_>, r: &ReadProperty) {
    let kind = named(
        object_type(r.object.object_type),
        "type",
        r.object.object_type.into(),
    );
    let prop = named(property(r.property), "property", r.property);
    span.set(Attribute::str(keys::OBJECT_TYPE, &kind));
    span.set(Attribute::int(
        keys::OBJECT_INSTANCE,
        r.object.instance.into(),
    ));
    span.set(Attribute::str(keys::PROPERTY, &prop));
}

/// [`Service::decode`] inside a `bacnet.service.<service>` span. An APDU
/// carrying no service this build reads opens no span.
///
/// # Errors
///
/// As [`Service::decode`]; the span fails with the error's reason.
pub fn decode_traced<'a>(apdu: &Apdu<'a>, tracer: &dyn Tracer) -> Result<Service<'a>, Error<'a>> {
    let Some(name) = span_name(apdu) else {
        return Service::decode(apdu);
    };
    let span = Span::start(tracer, name);
    let service = match Service::decode(apdu) {
        Ok(s) => s,
        Err(e) => {
            span.fail(&e.reason());
            return Err(e);
        }
    };
    span.set(Attribute::str(keys::SERVICE, service.name()));
    match &service {
        Service::IAm(i) => {
            span.set(Attribute::int(
                keys::DEVICE_INSTANCE,
                i.device.instance.into(),
            ));
            span.set(Attribute::int(keys::VENDOR_ID, i.vendor_id.into()));
        }
        Service::ReadProperty(r) => reference(&span, r),
        Service::ReadPropertyAck(a) => reference(&span, &a.read),
        Service::Error { error, .. } => {
            let class = named(error_class(error.class), "class", error.class);
            let code = named(error_code(error.code), "code", error.code);
            span.set(Attribute::str(keys::ERROR_CLASS, &class));
            span.set(Attribute::str(keys::ERROR_CODE, &code));
        }
        Service::WhoIs(_) | Service::Undecoded => {}
    }
    span.end();
    Ok(service)
}
