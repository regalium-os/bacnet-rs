// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

//! The worker against the simulated gateway, over real UDP on localhost.

use super::{Command, Event, Worker};
use bacnet_probe::frame::Delivery;
use bacnet_probe::sim::{self, Sim};
use bacnet_probe::{Client, Device, Direction, Discovery, Value};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

pub(super) fn start() -> (Sender<Command>, Receiver<Event>) {
    start_with(Sim::lift_gateway())
}

fn start_with(sim: Sim) -> (Sender<Command>, Receiver<Event>) {
    let target = sim.spawn().unwrap();
    let client = Client::bind("127.0.0.1:0".parse().unwrap())
        .unwrap()
        .with_timeout(Duration::from_millis(300))
        .with_retries(0);
    let (commands, worker_commands) = channel();
    let (worker_events, events) = channel();
    let plan = Discovery::to(target, Delivery::Unicast).with_wait(Duration::from_millis(200));
    let worker = Worker::new(client, plan, worker_commands, worker_events);
    std::thread::spawn(move || worker.run());
    (commands, events)
}

/// Collects events (packets included) until `done` matches one; fails after 5 s.
pub(super) fn until(events: &Receiver<Event>, done: impl Fn(&Event) -> bool) -> Vec<Event> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut seen = Vec::new();
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let e = events
            .recv_timeout(left)
            .unwrap_or_else(|_| panic!("timed out; saw {} events: {seen:#?}", seen.len()));
        let stop = done(&e);
        seen.push(e);
        if stop {
            return seen;
        }
    }
}

pub(super) fn discover(commands: &Sender<Command>, events: &Receiver<Event>) -> Device {
    commands.send(Command::Discover).unwrap();
    let seen = until(events, |e| matches!(e, Event::Found(_)));
    match seen.into_iter().last() {
        Some(Event::Found(mut devices)) => {
            assert_eq!(devices.len(), 1);
            devices.remove(0)
        }
        _ => unreachable!(),
    }
}

fn objects(seen: &[Event]) -> usize {
    seen.iter()
        .filter(|e| matches!(e, Event::Object(..)))
        .count()
}

#[test]
fn loading_a_device_streams_every_object_then_reports_it_complete() {
    let (commands, events) = start();
    let device = discover(&commands, &events);
    commands.send(Command::LoadDevice(device)).unwrap();
    let seen = until(&events, |e| matches!(e, Event::DeviceDone(..)));
    assert!(seen.iter().any(|e| matches!(e, Event::ObjectCount(599, 6))));
    assert_eq!(objects(&seen), 5, "every object except the device itself");
    assert!(matches!(seen.last(), Some(Event::DeviceDone(599, true))));
}

#[test]
fn a_detail_request_is_served_while_the_object_list_is_still_loading() {
    let (commands, events) = start();
    let device = discover(&commands, &events);
    commands.send(Command::LoadDevice(device.clone())).unwrap();
    commands
        .send(Command::LoadDetail(device, sim::CAR_LOAD))
        .unwrap();
    let seen = until(&events, |e| matches!(e, Event::DeviceDone(..)));
    let detail = seen
        .iter()
        .position(|e| matches!(e, Event::Detail(599, r) if r.id == sim::CAR_LOAD));
    assert!(
        detail.is_some(),
        "the detail should arrive before the list finishes"
    );
    assert!(
        matches!(seen.last(), Some(Event::DeviceDone(599, true))),
        "and the list should still complete"
    );
    assert_eq!(objects(&seen), 5);
}

#[test]
fn a_new_discovery_supersedes_a_device_load_in_flight() {
    let (commands, events) = start();
    let device = discover(&commands, &events);
    commands.send(Command::LoadDevice(device)).unwrap();
    commands.send(Command::Discover).unwrap();
    let seen = until(&events, |e| matches!(e, Event::Found(_)));
    let done = seen
        .iter()
        .position(|e| matches!(e, Event::DeviceDone(599, false)));
    let rediscover = seen.iter().position(|e| matches!(e, Event::Discovering));
    assert!(
        done.is_some() && done < rediscover,
        "load must stop, incomplete, before the new Who-Is"
    );
}

#[test]
fn following_a_point_subscribes_to_cov_and_reports_its_value() {
    let (commands, events) = start();
    let device = discover(&commands, &events);
    commands
        .send(Command::Watch(Some((device, vec![sim::CAR_LOAD]))))
        .unwrap();
    let seen = until(&events, |e| matches!(e, Event::Value(..)));
    assert!(
        seen.iter()
            .any(|e| matches!(e, Event::Following { cov: 1, polled: 0 }))
    );
    assert!(
        matches!(seen.last(), Some(Event::Value(599, id, Ok(Value::Real(v)))) if *id == sim::CAR_LOAD && *v == 420.0)
    );
}

#[test]
fn a_device_without_cov_is_polled_instead() {
    let (commands, events) = start_with(Sim::lift_gateway().without_cov());
    let device = discover(&commands, &events);
    commands
        .send(Command::SetInterval(Duration::from_millis(20)))
        .unwrap();
    commands
        .send(Command::Watch(Some((device, vec![sim::CAR_LOAD]))))
        .unwrap();
    let seen = until(&events, |e| matches!(e, Event::Value(..)));
    assert!(
        matches!(seen.last(), Some(Event::Value(599, id, Ok(Value::Real(v)))) if *id == sim::CAR_LOAD && *v == 420.0)
    );
}

#[test]
fn every_datagram_in_both_directions_is_reported_as_a_packet() {
    let (commands, events) = start();
    commands.send(Command::Discover).unwrap();
    let seen = until(&events, |e| matches!(e, Event::Found(_)));
    let dir = |d| {
        seen.iter()
            .any(|e| matches!(e, Event::Packet(p) if p.direction == d))
    };
    assert!(dir(Direction::Sent) && dir(Direction::Received));
}

mod reopen;
