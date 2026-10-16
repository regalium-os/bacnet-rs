// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Reopening from the Settings tab, against two simulated gateways: the new
//! settings take, and settings that cannot be opened leave the old working.

use super::{discover, start, until};
use crate::args::{self, Net};
use crate::connect::{Connector, Opened};
use crate::tui::worker::{Command, Event, Worker};
use bacnet_probe::frame::Delivery;
use bacnet_probe::sim::Sim;
use bacnet_probe::{Client, Discovery};
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

/// Opens loopback clients that ask `target:port`; port 1 is refused, as a
/// port held by another process would be.
fn connector() -> Connector {
    Box::new(|net: &Net| {
        if net.port == 1 {
            return Err("port 1 is refused by this test".into());
        }
        let client = Client::bind("127.0.0.1:0".parse().unwrap())
            .map_err(|e| e.to_string())?
            .with_timeout(Duration::from_millis(300))
            .with_retries(0);
        let to = SocketAddr::from((net.target, net.port));
        let plan = Discovery::to(to, Delivery::Unicast).with_wait(Duration::from_millis(200));
        Ok(Opened {
            client,
            plan,
            notes: vec![format!("opened for :{}", net.port)],
        })
    })
}

fn settings_for(sim: SocketAddr) -> Net {
    Net {
        target: Ipv4Addr::LOCALHOST,
        port: sim.port(),
        ..args::defaults()
    }
}

/// A reopenable worker, first pointed at its own simulated gateway.
fn start_reopenable() -> (Sender<Command>, Receiver<Event>, SocketAddr) {
    let first = Sim::lift_gateway().spawn().unwrap();
    let net = settings_for(first);
    let Opened { client, plan, .. } = connector()(&net).unwrap();
    let (commands, worker_commands) = channel();
    let (worker_events, events) = channel();
    let worker =
        Worker::new(client, plan, worker_commands, worker_events).with_connector(net, connector());
    std::thread::spawn(move || worker.run());
    (commands, events, first)
}

#[test]
fn new_settings_are_confirmed_then_discovered_through() {
    let (commands, events, first) = start_reopenable();
    assert_eq!(discover(&commands, &events).addr, first);
    let second = Sim::lift_gateway().spawn().unwrap();
    commands
        .send(Command::Reconfigure(settings_for(second)))
        .unwrap();
    let seen = until(&events, |e| matches!(e, Event::Found(_)));
    let connected = seen.iter().position(
        |e| matches!(e, Event::Connected(notes) if notes[0].contains(&second.port().to_string())),
    );
    let found = seen.iter().position(|e| matches!(e, Event::Found(_)));
    // Both must be seen: `None < Some(_)` would pass an absent `Connected`.
    let (connected, found) = (connected.expect("Connected"), found.expect("Found"));
    assert!(connected < found, "confirmed before anything is found");
    assert!(matches!(seen.last(), Some(Event::Found(d)) if d[0].addr == second));
}

#[test]
fn settings_that_cannot_be_opened_leave_the_previous_ones_working() {
    let (commands, events, first) = start_reopenable();
    let refused = Net {
        port: 1,
        ..settings_for(first)
    };
    commands.send(Command::Reconfigure(refused)).unwrap();
    let seen = until(&events, |e| matches!(e, Event::ConnectFailed(_)));
    assert!(
        matches!(seen.last(), Some(Event::ConnectFailed(why)) if why.contains("previous settings are back")),
        "{seen:#?}"
    );
    assert_eq!(discover(&commands, &events).addr, first);
}

#[test]
fn a_worker_given_no_connector_refuses_to_reconfigure() {
    let (commands, events) = start();
    commands
        .send(Command::Reconfigure(args::defaults()))
        .unwrap();
    let seen = until(&events, |e| matches!(e, Event::ConnectFailed(_)));
    assert!(matches!(seen.last(), Some(Event::ConnectFailed(_))));
}
