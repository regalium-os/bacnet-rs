// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used)]

//! Every codec round-trips, and the `ReadPropertyMultiple` request matches
//! octets assembled by hand from §20.2 and §21.

use super::rpm::{self, Results, Spec};
use super::who::{self, IAm, IHave, Wanted};
use super::{PropertyRef, cov, read};
use crate::tag::{ObjectId, Value};

const AI16: ObjectId = ObjectId::new(0, 16);

#[test]
fn a_read_property_multiple_request_matches_its_hand_assembled_octets() {
    let specs = [Spec {
        object: AI16,
        properties: vec![PropertyRef::whole(85), PropertyRef::whole(103)],
    }];
    // [0] analog-input:16, [1] { [0] present-value, [0] reliability }.
    let want = [
        0x0C, 0x00, 0x00, 0x00, 0x10, 0x1E, 0x09, 0x55, 0x09, 0x67, 0x1F,
    ];
    assert_eq!(rpm::request(&specs), want);
    assert_eq!(rpm::decode_request(&want).unwrap(), specs);
}

#[test]
fn a_read_property_multiple_ack_carries_a_value_or_an_error_per_property() {
    let results = vec![Results {
        object: AI16,
        properties: vec![
            (PropertyRef::whole(85), Ok(vec![Value::Real(21.5)])),
            (PropertyRef::whole(9999), Err((2, 32))),
            (
                PropertyRef {
                    property: 87,
                    index: Some(8),
                },
                Ok(vec![Value::Null]),
            ),
        ],
    }];
    assert_eq!(rpm::decode_ack(&rpm::ack(&results)).unwrap(), results);
}

#[test]
fn a_read_property_request_and_ack_round_trip() {
    let p = PropertyRef {
        property: 76,
        index: Some(0),
    };
    assert_eq!(
        read::decode_request(&read::request(AI16, p)),
        Some((AI16, p))
    );
    let values = [Value::Unsigned(6)];
    assert_eq!(
        read::decode_ack(&read::ack(AI16, p, &values)).unwrap(),
        values
    );
}

#[test]
fn who_is_limits_round_trip_and_an_empty_body_means_everyone() {
    assert_eq!(
        who::decode_who_is(&who::who_is(Some((500, 599)))),
        Some(Some((500, 599)))
    );
    assert_eq!(who::decode_who_is(&[]), Some(None));
}

#[test]
fn i_am_and_i_have_round_trip() {
    let me = IAm {
        device: ObjectId::new(8, 599),
        max_apdu: 1476,
        segmentation: 3,
        vendor: 999,
    };
    assert_eq!(who::decode_i_am(&who::i_am(&me)), Some(me));
    let have = IHave {
        device: ObjectId::new(8, 599),
        object: AI16,
        name: "Car 1 load".into(),
    };
    assert_eq!(who::decode_i_have(&who::i_have(&have)), Some(have));
}

#[test]
fn who_has_asks_by_name_or_by_identifier() {
    let by_name = Wanted::Name("Car 1 load".into());
    assert_eq!(
        who::decode_who_has(&who::who_has(None, &by_name)),
        Some((None, by_name))
    );
    let by_id = Wanted::Object(AI16);
    assert_eq!(
        who::decode_who_has(&who::who_has(Some((1, 9)), &by_id)),
        Some((Some((1, 9)), by_id))
    );
}

#[test]
fn a_subscription_and_its_cancellation_round_trip() {
    let s = cov::Subscribe {
        process: 7,
        object: AI16,
        confirmed: Some(false),
        lifetime: Some(300),
    };
    assert_eq!(cov::decode_subscribe(&cov::subscribe(&s)), Some(s));
    let cancel = cov::Subscribe {
        confirmed: None,
        lifetime: None,
        ..s
    };
    assert_eq!(
        cov::decode_subscribe(&cov::subscribe(&cancel)),
        Some(cancel)
    );
}

#[test]
fn a_cov_notification_round_trips_with_its_values() {
    let n = cov::Notification {
        process: 7,
        device: ObjectId::new(8, 599),
        object: AI16,
        time_remaining: 240,
        values: vec![
            (PropertyRef::whole(85), vec![Value::Real(610.0)]),
            (PropertyRef::whole(111), vec![Value::Bits(vec![false; 4])]),
        ],
    };
    assert_eq!(cov::decode_notification(&cov::notification(&n)), Some(n));
}

#[test]
fn a_truncated_body_is_a_decode_failure_not_a_panic() {
    let full = rpm::ack(&[Results {
        object: AI16,
        properties: vec![(PropertyRef::whole(85), Ok(vec![Value::Real(1.0)]))],
    }]);
    for cut in 0..full.len() {
        let _ = rpm::decode_ack(full.get(..cut).unwrap());
    }
    assert!(rpm::decode_ack(full.get(..full.len() - 1).unwrap()).is_none());
}
