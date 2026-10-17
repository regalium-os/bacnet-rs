// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used)]

//! The virtual MAC table on a fake clock: learning, ageing, resolution with
//! its retries, and a duplicate reported rather than settled.

use regalium_bacnet_ipv6::bvll::{Bvll6, Function6, Vmac};
use regalium_bacnet_ipv6::vmac_table::{Duplicate, Resolve, VmacTable};
use std::net::{Ipv6Addr, SocketAddrV6};
use std::time::Duration;

const ME: Vmac = Vmac([0, 2, 0x57]);
const HIM: Vmac = Vmac([0, 0, 10]);

fn at(host: u16) -> SocketAddrV6 {
    SocketAddrV6::new(Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, host), 47808, 0, 0)
}

fn s(secs: u64) -> Duration {
    Duration::from_secs(secs)
}

fn table() -> VmacTable {
    VmacTable::new(ME).timers(s(60), s(1), 3)
}

#[test]
fn a_frame_teaches_an_address_which_ages_out_unheard() {
    let mut t = table();
    assert_eq!(t.learn(HIM, at(1), s(0)), None);
    assert_eq!(t.lookup(HIM, s(59)), Some(at(1)));
    assert_eq!(t.lookup(HIM, s(60)), None);
    assert_eq!(
        t.learn(ME, at(9), s(0)),
        None,
        "its own address is never learnt"
    );
}

#[test]
fn an_unknown_vmac_is_asked_for_three_times_and_then_given_up() {
    let mut t = table();
    let ask = Bvll6::to_vec(ME, Function6::AddressResolution(HIM));
    assert_eq!(t.resolve(HIM, s(0)), Resolve::Ask(ask.clone()));
    assert_eq!(t.resolve(HIM, s(0)), Resolve::Waiting);
    assert_eq!(t.poll(s(1)), (vec![ask.clone()], vec![]));
    assert_eq!(t.poll(s(2)), (vec![ask], vec![]));
    assert_eq!(t.poll(s(3)), (vec![], vec![HIM]));
    assert_eq!(t.deadline(), None);
}

#[test]
fn an_answer_ends_the_resolution() {
    let mut t = table();
    let _ = t.resolve(HIM, s(0));
    t.learn(HIM, at(1), s(0));
    assert_eq!(t.resolve(HIM, s(0)), Resolve::Known(at(1)));
    assert_eq!(t.deadline(), None);
}

#[test]
fn a_second_address_for_a_fresh_vmac_is_a_duplicate_and_the_first_is_kept() {
    let mut t = table();
    t.learn(HIM, at(1), s(0));
    let d = t.learn(HIM, at(2), s(5));
    assert_eq!(
        d,
        Some(Duplicate {
            vmac: HIM,
            known: at(1),
            claimed: at(2)
        })
    );
    assert_eq!(t.lookup(HIM, s(5)), Some(at(1)));
    // Once the first has aged out, the second is simply where it is now.
    assert_eq!(t.learn(HIM, at(2), s(70)), None);
    assert_eq!(t.lookup(HIM, s(70)), Some(at(2)));
}

#[test]
fn a_configured_vmac_never_ages_and_a_claim_on_it_is_a_duplicate() {
    let mut t = table();
    t.insert(HIM, at(1));
    assert_eq!(t.lookup(HIM, s(100_000)), Some(at(1)));
    assert!(t.learn(HIM, at(3), s(100_000)).is_some());
}
