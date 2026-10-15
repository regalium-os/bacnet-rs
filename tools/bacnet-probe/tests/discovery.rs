// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

//! Range sweeps, Who-Has, and foreign device registration.

mod common;

use bacnet_probe::frame::Delivery;
use bacnet_probe::service::who::Wanted;
use bacnet_probe::sim::{self, Sim};
use bacnet_probe::{Discovery, ForeignError, Scope};
use std::net::SocketAddr;
use std::time::Duration;

fn plan(addr: SocketAddr) -> Discovery {
    Discovery::to(addr, Delivery::Unicast).with_wait(Duration::from_millis(300))
}

#[test]
fn a_who_is_limited_to_another_range_finds_nothing() {
    let addr = Sim::lift_gateway().spawn().unwrap();
    let mut client = common::client();
    assert_eq!(
        client
            .discover(&plan(addr).with_range(Some((1, 100))))
            .unwrap(),
        [] as [bacnet_probe::Device; 0]
    );
    assert_eq!(
        client
            .discover(&plan(addr).with_range(Some((599, 599))))
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn a_unicast_sweep_of_a_range_finds_the_device_inside_it() {
    let addr = Sim::lift_gateway().spawn().unwrap();
    let scope: Scope = "127.0.0.0/30".parse().unwrap();
    let sweep = Discovery::sweep(&[scope], addr.port(), true)
        .unwrap()
        .with_wait(Duration::from_millis(300))
        .with_rate(1000);
    assert_eq!(sweep.len(), 2, "127.0.0.1 and 127.0.0.2");
    let found = common::client().discover(&sweep).unwrap();
    assert_eq!(found.iter().map(|d| d.instance).collect::<Vec<_>>(), [599]);
}

#[test]
fn answers_from_outside_the_allowed_range_are_dropped() {
    let addr = Sim::lift_gateway().spawn().unwrap();
    let mut client = common::client();
    // The simulator answers from 127.0.0.1; only 10.0.0.0/8 is accepted.
    let elsewhere = plan(addr).allowing(vec!["10.*".parse().unwrap()]);
    assert_eq!(
        client.discover(&elsewhere).unwrap(),
        [] as [bacnet_probe::Device; 0]
    );
    let loopback = plan(addr).allowing(vec!["127.*".parse().unwrap()]);
    assert_eq!(client.discover(&loopback).unwrap().len(), 1);
}

#[test]
fn a_broadcast_sweep_sends_one_datagram_per_slash_24() {
    let sweep = Discovery::sweep(&["10.*".parse().unwrap()], 47808, false).unwrap();
    assert_eq!(sweep.len(), 65_536);
}

#[test]
fn an_oversized_unicast_sweep_is_refused_before_anything_is_sent() {
    let err = Discovery::sweep(&["10.0.0.0/8".parse().unwrap()], 47808, true).unwrap_err();
    assert!(err.contains("limited to"), "{err}");
}

#[test]
fn who_has_finds_an_object_by_name_and_by_identifier() {
    let addr = Sim::lift_gateway().spawn().unwrap();
    let mut client = common::client();
    let by_name = client
        .who_has(&plan(addr), &Wanted::Name("Car 1 door".into()))
        .unwrap();
    assert_eq!(by_name.len(), 1);
    assert_eq!(
        (by_name[0].0.instance, by_name[0].1.object),
        (599, sim::DOOR)
    );
    let by_id = client
        .who_has(&plan(addr), &Wanted::Object(sim::FAN))
        .unwrap();
    assert_eq!(by_id[0].1.name, "Machine room fan speed");
    assert_eq!(
        client
            .who_has(&plan(addr), &Wanted::Name("no such point".into()))
            .unwrap(),
        [] as [(bacnet_probe::Device, bacnet_probe::service::who::IHave); 0]
    );
}

#[test]
fn a_registered_foreign_device_discovers_through_its_bbmd() {
    let bbmd = Sim::lift_gateway().spawn().unwrap();
    let mut client = common::client();
    client.register_foreign_device(bbmd, 300).unwrap();
    assert_eq!(client.bbmd(), Some(bbmd));
    // A broadcast to a subnet this host is not on goes to the BBMD instead.
    let far = Discovery::to("192.0.2.255:47808".parse().unwrap(), Delivery::Broadcast)
        .with_wait(Duration::from_millis(300));
    assert_eq!(client.discover(&far).unwrap().len(), 1);
}

#[test]
fn a_bbmd_that_refuses_the_registration_says_so() {
    let bbmd = Sim::lift_gateway()
        .refusing_foreign_devices()
        .spawn()
        .unwrap();
    let err = common::client()
        .register_foreign_device(bbmd, 300)
        .unwrap_err();
    assert!(matches!(err, ForeignError::Refused(0x0030)), "got {err:?}");
}
