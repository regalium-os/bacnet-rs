// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used, clippy::panic)]

//! Changes from the TUI, performed by the worker against the simulated
//! gateway over real UDP, and reported back as the device answered them.

use super::{discover, start, until};
use crate::tui::worker::{Change, Command, Event};
use bacnet_probe::sim;
use bacnet_probe::{ObjectId, Value};

fn outcome(seen: &[Event]) -> (bool, Result<Option<ObjectId>, String>) {
    match seen.last() {
        Some(Event::Changed {
            structural, result, ..
        }) => (*structural, result.clone()),
        other => panic!("expected Changed, got {other:?}"),
    }
}

#[test]
fn a_write_and_a_create_are_applied_and_reported() {
    let (commands, events) = start();
    let device = discover(&commands, &events);
    let write = Change::Write {
        device: device.clone(),
        object: sim::FAN,
        property: 85,
        values: vec![Value::Real(75.0)],
        priority: Some(8),
    };
    commands.send(Command::Change(write)).unwrap();
    let seen = until(&events, |e| matches!(e, Event::Changed { .. }));
    assert_eq!(outcome(&seen), (false, Ok(Some(sim::FAN))));

    let create = Change::Create {
        device,
        kind: 2,
        initial: Vec::new(),
    };
    commands.send(Command::Change(create)).unwrap();
    let seen = until(&events, |e| matches!(e, Event::Changed { .. }));
    assert_eq!(outcome(&seen), (true, Ok(Some(ObjectId::new(2, 1)))));
}

#[test]
fn a_refusal_is_reported_in_the_devices_own_words() {
    let (commands, events) = start();
    let device = discover(&commands, &events);
    commands
        .send(Command::Change(Change::Delete {
            device,
            object: sim::FAN,
        }))
        .unwrap();
    let seen = until(&events, |e| matches!(e, Event::Changed { .. }));
    let (_, result) = outcome(&seen);
    assert!(
        result
            .unwrap_err()
            .contains("object-deletion-not-permitted")
    );
}
