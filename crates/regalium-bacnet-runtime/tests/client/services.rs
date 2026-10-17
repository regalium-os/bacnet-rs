// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Writes, `ReadPropertyMultiple` and its fallback, and watches: COV where
//! the device offers it — renewed, acknowledged, cancelled — and polling
//! where it refuses.

use super::client;
use regalium_bacnet_core::apdu::Apdu;
use regalium_bacnet_core::npdu::{Body, Npdu};
use regalium_bacnet_core::primitive::{ObjectIdentifier, Primitive};
use regalium_bacnet_core::service::{
    PropertyReference, ReadAccess, Service, ServiceError, SubscribeCov,
};
use regalium_bacnet_runtime::{Client, Error, RequestError, Source, Watch};
use std::time::Duration;

const AI1: ObjectIdentifier = ObjectIdentifier {
    object_type: 0,
    instance: 1,
};
const AI2: ObjectIdentifier = ObjectIdentifier {
    object_type: 0,
    instance: 2,
};
const AI99: ObjectIdentifier = ObjectIdentifier {
    object_type: 0,
    instance: 99,
};

fn bound() -> Client<super::Net> {
    let mut c = client();
    c.who_is(None, Duration::from_millis(50)).unwrap();
    c
}

/// Every service the client sent, in order, with the NPDU stripped.
fn sent(c: &Client<super::Net>) -> Vec<Vec<u8>> {
    c.link()
        .sent
        .lock()
        .unwrap()
        .iter()
        .filter_map(|(_, n)| match Npdu::decode(n).unwrap().body {
            Body::Apdu(a) => Some(a.to_vec()),
            Body::Message(_) => None,
        })
        .collect()
}

fn subscriptions(c: &Client<super::Net>) -> Vec<SubscribeCov> {
    sent(c)
        .iter()
        .filter_map(
            |a| match Service::decode(&Apdu::decode(a).unwrap()).unwrap() {
                Service::SubscribeCov(s) => Some(s),
                _ => None,
            },
        )
        .collect()
}

#[test]
fn a_write_is_acknowledged_and_a_refused_one_says_why() {
    let mut c = bound();
    c.write_property(599, AI1, 85, &Primitive::Real(42.5), Some(8))
        .unwrap();
    let e = c
        .write_property(599, AI99, 85, &Primitive::Real(1.0), None)
        .unwrap_err();
    assert!(
        matches!(
            e,
            Error::Request(RequestError::Error(ServiceError { class: 2, code: 40 }))
        ),
        "{e}"
    );
}

#[test]
fn read_multiple_returns_a_result_per_property() {
    let mut c = bound();
    let reads = [ReadAccess {
        object: AI1,
        properties: [85, 111, 28].map(PropertyReference::new).to_vec(),
    }];
    let all = c.read_multiple(599, &reads).unwrap();
    assert_eq!(
        all[0].as_ref().unwrap().primitive(),
        Some(Primitive::Real(21.5))
    );
    assert!(all[1].is_ok());
    assert!(matches!(
        all[2],
        Err(RequestError::Error(ServiceError { class: 2, code: 32 }))
    ));
    assert!(c.supports_read_multiple(599));
}

#[test]
fn a_device_that_rejects_read_multiple_is_read_singly_and_not_asked_again() {
    let mut c = bound();
    let reads = [ReadAccess {
        object: AI1,
        properties: [85, 8].map(PropertyReference::new).to_vec(),
    }];
    for _ in 0..2 {
        let all = c.read_multiple(10, &reads).unwrap();
        assert_eq!(
            all[0].as_ref().unwrap().primitive(),
            Some(Primitive::Real(21.5))
        );
        assert_eq!(
            all[1],
            Err(RequestError::Unsupported),
            "`all` needs ReadPropertyMultiple"
        );
    }
    assert!(!c.supports_read_multiple(10));
    let rpms = sent(&c).iter().filter(|a| a.get(3) == Some(&14)).count();
    assert_eq!(rpms, 1, "asked once, remembered");
}

#[test]
fn a_watch_subscribes_and_the_device_pushes_its_values() {
    let mut c = bound();
    let id = c.watch(Watch::new(599, AI1)).unwrap();
    c.listen(Duration::from_millis(30)).unwrap();
    let updates: Vec<_> = c.updates().collect();
    assert_eq!(updates.len(), 1, "{updates:?}");
    assert_eq!((updates[0].watch, updates[0].source), (id, Source::Cov));
    assert_eq!(
        updates[0].values[0].as_ref().unwrap().primitive(),
        Some(Primitive::Real(22.0))
    );
    let s = subscriptions(&c);
    assert_eq!(
        (s[0].process, s[0].confirmed, s[0].lifetime),
        (id, Some(false), Some(300))
    );
}

#[test]
fn a_confirmed_notification_is_acknowledged() {
    let mut c = bound();
    c.watch(Watch::new(599, AI2)).unwrap();
    c.listen(Duration::from_millis(30)).unwrap();
    assert!(
        sent(&c).contains(&vec![0x20, 77, 0x01]),
        "Simple-ACK for invoke 77"
    );
    assert_eq!(c.updates().count(), 1);
}

#[test]
fn a_refused_subscription_polls_instead() {
    let mut c = bound();
    c.watch(Watch::new(10, AI1).every(Duration::from_millis(40)))
        .unwrap();
    c.listen(Duration::from_millis(100)).unwrap();
    let updates: Vec<_> = c.updates().collect();
    assert!(updates.len() >= 2, "{updates:?}");
    assert!(updates.iter().all(|u| u.source == Source::Poll));
    assert_eq!(
        updates[0].values[0].as_ref().unwrap().primitive(),
        Some(Primitive::Real(21.5))
    );
}

#[test]
fn a_subscription_is_renewed_before_it_lapses_and_cancelled_when_unwatched() {
    let mut c = bound();
    let id = c
        .watch(Watch::new(599, AI1).cov(Some(Duration::from_secs(2))))
        .unwrap();
    c.listen(Duration::from_millis(1100)).unwrap();
    assert_eq!(subscriptions(&c).len(), 2, "renewed half way through 2 s");
    c.unwatch(id).unwrap();
    let last = *subscriptions(&c).last().unwrap();
    assert!(last.is_cancellation() && last.process == id);
}
