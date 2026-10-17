// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used)]

//! The master node on a fake clock: token recovery after silence, finding
//! a successor, using and passing the token, answering, and a station that
//! vanishes. No bus, no waiting.

use core::time::Duration;
use regalium_bacnet_mstp::frame::{BROADCAST, Frame, kind};
use regalium_bacnet_mstp::master::{Action, Config, Master, State};

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

fn send(kind: u8, to: u8, from: u8) -> Action {
    Action::Send(Frame::bare(kind, to, from))
}

/// Station 3 of 0..=7, already holding station 5 as its successor.
fn with_successor() -> Master {
    let mut m = Master::new(Config::new(3).max_master(7), ms(0));
    m.poll(ms(500));
    m.poll(ms(530));
    m.received(&Frame::bare(kind::REPLY_TO_POLL_FOR_MASTER, 3, 5), ms(540));
    m
}

#[test]
fn after_silence_the_token_is_recovered_by_polling_the_next_station() {
    let mut m = Master::new(Config::new(3).max_master(7), ms(0));
    assert_eq!(m.poll(ms(499)), []);
    assert_eq!(m.poll(ms(500)), []);
    assert_eq!(m.state(), State::NoToken);
    // Station 3 waits three slots, so a lower station would speak first.
    assert_eq!(m.deadline(), ms(530));
    assert_eq!(m.poll(ms(530)), [send(kind::POLL_FOR_MASTER, 4, 3)]);
    assert_eq!(m.poll(ms(580)), [send(kind::POLL_FOR_MASTER, 5, 3)]);
}

#[test]
fn a_reply_to_a_poll_makes_a_successor_and_the_token_goes_to_it() {
    let mut m = Master::new(Config::new(3).max_master(7), ms(0));
    m.poll(ms(500));
    m.poll(ms(530));
    let out = m.received(&Frame::bare(kind::REPLY_TO_POLL_FOR_MASTER, 3, 4), ms(540));
    assert_eq!(out, [send(kind::TOKEN, 4, 3)]);
    assert_eq!((m.next_station(), m.state()), (4, State::PassToken));
    // Hearing the successor use it is the end of passing.
    m.received(&Frame::bare(kind::POLL_FOR_MASTER, 5, 4), ms(545));
    assert_eq!(m.state(), State::Idle);
}

#[test]
fn a_token_held_sends_what_is_queued_then_passes_on() {
    let mut m = with_successor();
    assert_eq!(
        m.enqueue(BROADCAST, vec![1, 0, 0x10, 8], false, ms(550)),
        []
    );
    let out = m.received(&Frame::bare(kind::TOKEN, 3, 5), ms(600));
    let data = Frame {
        kind: kind::DATA_NOT_EXPECTING_REPLY,
        destination: BROADCAST,
        source: 3,
        data: vec![1, 0, 0x10, 8],
    };
    assert_eq!(out, [Action::Send(data), send(kind::TOKEN, 5, 3)]);
}

#[test]
fn a_request_waits_for_its_reply_before_the_token_moves() {
    let mut m = with_successor();
    m.enqueue(5, vec![1, 4, 0, 5], true, ms(550));
    let out = m.received(&Frame::bare(kind::TOKEN, 3, 5), ms(600));
    assert_eq!(out.len(), 1);
    assert_eq!(m.state(), State::WaitForReply);
    let reply = Frame {
        kind: kind::DATA_NOT_EXPECTING_REPLY,
        destination: 3,
        source: 5,
        data: vec![1, 0, 0x30],
    };
    let out = m.received(&reply, ms(650));
    assert_eq!(
        out,
        [
            Action::Deliver {
                source: 5,
                npdu: vec![1, 0, 0x30],
                broadcast: false
            },
            send(kind::TOKEN, 5, 3)
        ]
    );
}

#[test]
fn a_successor_that_never_takes_the_token_is_passed_over() {
    let mut m = with_successor();
    assert_eq!(
        m.poll(ms(590)),
        [send(kind::TOKEN, 5, 3)],
        "passed once more"
    );
    assert_eq!(
        m.poll(ms(640)),
        [send(kind::POLL_FOR_MASTER, 6, 3)],
        "then the next one is looked for"
    );
    assert_eq!(m.next_station(), 3);
}

#[test]
fn a_master_alone_on_the_bus_keeps_the_token_and_sends_at_once() {
    let mut m = Master::new(Config::new(1).max_master(2), ms(0));
    m.poll(ms(500));
    m.poll(ms(510));
    m.poll(ms(560));
    assert_eq!(m.poll(ms(610)), []);
    assert_eq!(m.state(), State::SoleMaster);
    let out = m.enqueue(BROADCAST, vec![1, 0], false, ms(620));
    assert_eq!(
        out,
        [Action::Send(Frame {
            kind: kind::DATA_NOT_EXPECTING_REPLY,
            destination: BROADCAST,
            source: 1,
            data: vec![1, 0]
        })]
    );
}

#[test]
fn a_request_is_answered_in_place_or_postponed() {
    let mut m = Master::new(Config::new(3), ms(0));
    let request = Frame {
        kind: kind::DATA_EXPECTING_REPLY,
        destination: 3,
        source: 9,
        data: vec![1, 4],
    };
    assert_eq!(
        m.received(&request, ms(10)),
        [Action::Deliver {
            source: 9,
            npdu: vec![1, 4],
            broadcast: false
        }]
    );
    let out = m.enqueue(9, vec![1, 0, 0x30], false, ms(20));
    assert_eq!(
        out,
        [Action::Send(Frame {
            kind: kind::DATA_NOT_EXPECTING_REPLY,
            destination: 9,
            source: 3,
            data: vec![1, 0, 0x30]
        })]
    );
    m.received(&request, ms(30));
    assert_eq!(m.poll(ms(280)), [send(kind::REPLY_POSTPONED, 9, 3)]);
}

#[test]
fn a_poll_for_this_station_is_answered_and_a_test_request_echoed() {
    let mut m = Master::new(Config::new(3), ms(0));
    assert_eq!(
        m.received(&Frame::bare(kind::POLL_FOR_MASTER, 3, 1), ms(5)),
        [send(kind::REPLY_TO_POLL_FOR_MASTER, 1, 3)]
    );
    let test = Frame {
        kind: kind::TEST_REQUEST,
        destination: 3,
        source: 1,
        data: vec![7, 7],
    };
    assert_eq!(
        m.received(&test, ms(6)),
        [Action::Send(Frame {
            kind: kind::TEST_RESPONSE,
            destination: 1,
            source: 3,
            data: vec![7, 7]
        })]
    );
}
