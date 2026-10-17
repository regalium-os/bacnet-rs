// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used)]

//! Foreign device registration on a fake clock: when it asks, what an
//! answer does, and that it renews before the BBMD forgets it.

use regalium_bacnet_ip::foreign::{Outcome, Registration};
use std::net::SocketAddrV4;
use std::time::Duration;

const BBMD: SocketAddrV4 = SocketAddrV4::new(std::net::Ipv4Addr::new(10, 0, 0, 1), 47808);

fn s(secs: f64) -> Duration {
    Duration::from_secs_f64(secs)
}

#[test]
fn it_asks_at_once_retries_on_silence_and_renews_thirty_seconds_early() {
    let mut r = Registration::new(BBMD, 300);
    assert_eq!(
        r.poll(s(0.0)),
        Some(vec![0x81, 0x05, 0x00, 0x06, 0x01, 0x2c])
    );
    assert_eq!(r.poll(s(1.0)), None, "waiting for the answer");
    assert!(r.poll(s(2.0)).is_some(), "silence: ask again");
    assert_eq!(
        r.result(BBMD, 0, s(2.5)),
        Some(Outcome::Registered { expires: s(302.5) })
    );
    assert_eq!(r.deadline(), s(272.5));
    assert!(r.is_registered(s(272.0)));
    assert_eq!(r.poll(s(272.0)), None);
    assert!(r.poll(s(272.5)).is_some(), "renewal");
    assert!(!r.is_registered(s(302.5)), "a lost renewal lets it lapse");
}

#[test]
fn a_short_ttl_renews_half_way() {
    let mut r = Registration::new(BBMD, 60);
    r.poll(s(0.0));
    r.result(BBMD, 0, s(0.0));
    assert_eq!(r.deadline(), s(30.0));
}

#[test]
fn a_refusal_is_reported_and_asked_again_later() {
    let mut r = Registration::new(BBMD, 300);
    r.poll(s(0.0));
    assert_eq!(
        r.result(BBMD, 0x0030, s(0.1)),
        Some(Outcome::Refused(0x0030))
    );
    assert!(!r.is_registered(s(1.0)));
    assert_eq!(r.deadline(), s(30.1));
}

#[test]
fn a_result_from_another_host_or_unasked_is_not_an_answer() {
    let mut r = Registration::new(BBMD, 300);
    assert_eq!(r.result(BBMD, 0, s(0.0)), None, "nothing was asked");
    r.poll(s(0.0));
    let other = SocketAddrV4::new(std::net::Ipv4Addr::new(10, 0, 0, 9), 47808);
    assert_eq!(r.result(other, 0, s(0.1)), None);
    assert!(r.result(BBMD, 0, s(0.2)).is_some());
}
