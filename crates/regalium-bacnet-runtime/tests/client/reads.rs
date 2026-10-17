// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `ReadProperty` through the client: a value, an error, a device behind a
//! router, a batch in flight, and a device that stops answering — which is
//! an event, reported once, and reported again when it comes back.

use super::client;
use regalium_bacnet_core::primitive::{ObjectIdentifier, Primitive};
use regalium_bacnet_core::service::ServiceError;
use regalium_bacnet_core::tsm::Abandoned;
use regalium_bacnet_runtime::{Error, Event, ReadRequest, RequestError};
use regalium_bacnet_schema::object::value::Kind;
use std::sync::atomic::Ordering;
use std::time::Duration;

const AI1: ObjectIdentifier = ObjectIdentifier {
    object_type: 0,
    instance: 1,
};
const AI99: ObjectIdentifier = ObjectIdentifier {
    object_type: 0,
    instance: 99,
};

#[test]
fn a_read_returns_the_value_and_its_place_in_the_model() {
    let mut c = client();
    c.who_is(None, Duration::from_millis(50)).unwrap();
    let r = c.read_property(599, AI1, 85).unwrap();
    assert_eq!(r.primitive(), Some(Primitive::Real(21.5)));
    let p = r.property().unwrap();
    assert_eq!(
        p.name,
        "devices/599/objects/analog-input-1/properties/present-value"
    );
    assert_eq!(p.value.unwrap().kind, Some(Kind::RealValue(21.5)));
}

#[test]
fn a_read_through_a_router_names_the_device_network_in_the_npdu() {
    let mut c = client();
    c.who_is(None, Duration::from_millis(50)).unwrap();
    assert_eq!(
        c.read_property(10, AI1, 85).unwrap().primitive(),
        Some(Primitive::Real(21.5))
    );
    let sent = c.link().sent.lock().unwrap().clone();
    let (_, npdu) = sent.last().unwrap();
    // Expects a reply, DNET 5, DLEN 1, DADR 0x0a, hop count 255.
    assert_eq!(&npdu[..7], &[0x01, 0x24, 0x00, 0x05, 0x01, 0x0a, 0xff]);
}

#[test]
fn a_device_that_refuses_and_one_never_bound_are_told_apart() {
    let mut c = client();
    c.who_is(None, Duration::from_millis(50)).unwrap();
    let e = c.read_property(599, AI99, 85).unwrap_err();
    assert!(
        matches!(
            e,
            Error::Request(RequestError::Error(ServiceError { class: 1, code: 31 }))
        ),
        "{e}"
    );
    let e = c.read_property(42, AI1, 85).unwrap_err();
    assert!(matches!(e, Error::Request(RequestError::NotBound)), "{e}");
}

#[test]
fn a_batch_answers_every_read_in_the_order_asked() {
    let mut c = client();
    c.who_is(None, Duration::from_millis(50)).unwrap();
    let asks: Vec<_> = (0..40)
        .map(|i| {
            ReadRequest::new(
                if i % 2 == 0 { 599 } else { 10 },
                if i % 5 == 0 { AI99 } else { AI1 },
                85,
            )
        })
        .collect();
    let all = c.read_many(&asks).unwrap();
    for (ask, got) in asks.iter().zip(&all) {
        match got {
            Ok(r) => assert_eq!((r.device, r.read.object), (ask.device, AI1)),
            Err(RequestError::Error(_)) => assert_eq!(ask.read.object, AI99),
            Err(e) => panic!("{e}"),
        }
    }
}

#[test]
fn a_device_going_quiet_is_an_event_once_and_coming_back_is_another() {
    let mut c = client();
    c.who_is(None, Duration::from_millis(50)).unwrap();
    let _ = c.events().count();
    c.link().silent.store(true, Ordering::SeqCst);
    for _ in 0..2 {
        let e = c.read_property(599, AI1, 85).unwrap_err();
        assert!(
            matches!(
                e,
                Error::Request(RequestError::Offline(Abandoned::Timeout { attempts: 2 }))
            ),
            "{e}"
        );
    }
    let offline = Event::Offline {
        instance: 599,
        reason: Abandoned::Timeout { attempts: 2 },
    };
    assert_eq!(c.events().collect::<Vec<_>>(), [offline]);
    c.link().silent.store(false, Ordering::SeqCst);
    c.read_property(599, AI1, 85).unwrap();
    assert_eq!(c.events().collect::<Vec<_>>(), [Event::Reachable(599)]);
}
