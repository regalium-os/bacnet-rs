// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The service layer's spans, `bacnet.service.<service>`: one per decoded
//! service, saying what was asked of which object — object, property,
//! device — and never a value read, written or notified.

use super::confirmed as c;
use super::unconfirmed as u;
use super::{ReadProperty, Service};
use crate::apdu::{Apdu, Pdu};
use crate::enumerated::{error_class, error_code, object_type, property};
use crate::error::Error;
use crate::primitive::ObjectIdentifier;
use regalium_bacnet_telemetry::{Attribute, Span, Tracer, keys};

/// The span a service gets, chosen from the APDU before decoding so a
/// failed decode is recorded under the service that failed.
const fn span_name(apdu: &Apdu<'_>) -> Option<&'static str> {
    Some(match apdu.pdu {
        Pdu::UnconfirmedRequest { service, .. } => match service {
            u::WHO_IS => "bacnet.service.who_is",
            u::I_AM => "bacnet.service.i_am",
            u::WHO_HAS => "bacnet.service.who_has",
            u::I_HAVE => "bacnet.service.i_have",
            u::COV_NOTIFICATION => "bacnet.service.cov_notification",
            u::EVENT_NOTIFICATION => "bacnet.service.event_notification",
            _ => return None,
        },
        Pdu::ConfirmedRequest { service, .. } => match service {
            c::READ_PROPERTY => "bacnet.service.read_property",
            c::READ_PROPERTY_MULTIPLE => "bacnet.service.read_property_multiple",
            c::WRITE_PROPERTY => "bacnet.service.write_property",
            c::WRITE_PROPERTY_MULTIPLE => "bacnet.service.write_property_multiple",
            c::SUBSCRIBE_COV => "bacnet.service.subscribe_cov",
            c::SUBSCRIBE_COV_PROPERTY => "bacnet.service.subscribe_cov_property",
            c::COV_NOTIFICATION => "bacnet.service.cov_notification",
            c::EVENT_NOTIFICATION => "bacnet.service.event_notification",
            c::CREATE_OBJECT => "bacnet.service.create_object",
            c::DELETE_OBJECT => "bacnet.service.delete_object",
            _ => return None,
        },
        Pdu::ComplexAck { service, .. } => match service {
            c::READ_PROPERTY => "bacnet.service.read_property_ack",
            c::READ_PROPERTY_MULTIPLE => "bacnet.service.read_property_multiple_ack",
            c::CREATE_OBJECT => "bacnet.service.create_object_ack",
            _ => return None,
        },
        Pdu::Error { .. } => "bacnet.service.error",
        _ => return None,
    })
}

fn named(name: Option<&'static str>, prefix: &str, n: u32) -> String {
    name.map_or_else(|| format!("{prefix}-{n}"), str::to_owned)
}

fn object_attrs(span: &Span<'_>, o: ObjectIdentifier) {
    let kind = named(object_type(o.object_type), "type", o.object_type.into());
    span.set(Attribute::str(keys::OBJECT_TYPE, &kind));
    span.set(Attribute::int(keys::OBJECT_INSTANCE, o.instance.into()));
}

fn property_attr(span: &Span<'_>, p: u32) {
    span.set(Attribute::str(
        keys::PROPERTY,
        &named(property(p), "property", p),
    ));
}

fn reference(span: &Span<'_>, r: &ReadProperty) {
    object_attrs(span, r.object);
    property_attr(span, r.property);
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
        Service::IHave(i) => {
            span.set(Attribute::int(
                keys::DEVICE_INSTANCE,
                i.device.instance.into(),
            ));
            object_attrs(&span, i.object);
        }
        Service::ReadProperty(r) => reference(&span, r),
        Service::ReadPropertyAck(a) => reference(&span, &a.read),
        Service::WriteProperty(w) => {
            object_attrs(&span, w.object);
            property_attr(&span, w.value.property.property);
            if let Some(p) = w.value.priority {
                span.set(Attribute::int(keys::PRIORITY, p.into()));
            }
        }
        Service::SubscribeCov(s) => object_attrs(&span, s.object),
        Service::SubscribeCovProperty(s) => {
            object_attrs(&span, s.subscription.object);
            property_attr(&span, s.property.property);
        }
        Service::CovNotification {
            notification: n, ..
        } => {
            span.set(Attribute::int(
                keys::DEVICE_INSTANCE,
                n.device.instance.into(),
            ));
            object_attrs(&span, n.object);
        }
        Service::EventNotification {
            notification: n, ..
        } => {
            span.set(Attribute::int(
                keys::DEVICE_INSTANCE,
                n.device.instance.into(),
            ));
            object_attrs(&span, n.object);
        }
        Service::DeleteObject(o) | Service::CreateObjectAck(o) => object_attrs(&span, *o),
        Service::Error { error, .. } => {
            let class = named(error_class(error.class), "class", error.class);
            let code = named(error_code(error.code), "code", error.code);
            span.set(Attribute::str(keys::ERROR_CLASS, &class));
            span.set(Attribute::str(keys::ERROR_CODE, &code));
        }
        _ => {}
    }
    span.end();
    Ok(service)
}
