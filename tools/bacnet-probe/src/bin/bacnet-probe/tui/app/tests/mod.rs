// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp
)]

//! The app as a state machine: events and keys in, commands out. Then one
//! frame rendered to an in-memory terminal to prove the panes draw.

#[allow(unused_imports)]
use super::Load;
use super::{App, Focus};
use crate::tui::{worker::Command, worker::Event};
use bacnet_probe::scan::ObjectReport;
use bacnet_probe::{Device, ObjectId, Value, property};
use ratatui::crossterm::event::{KeyCode, KeyEvent};

const LOAD: ObjectId = ObjectId::new(0, 1);
const DOOR: ObjectId = ObjectId::new(3, 1);

pub(super) fn device() -> Device {
    Device {
        instance: 599,
        addr: "10.0.0.5:47808".parse().unwrap(),
        route: None,
        max_apdu: 1476,
        vendor: 999,
    }
}

pub(super) fn report(id: ObjectId, name: &str, value: Value) -> ObjectReport {
    ObjectReport {
        id,
        reads: vec![
            (property::OBJECT_NAME, Ok(vec![Value::Text(name.into())])),
            (property::PRESENT_VALUE, Ok(vec![value])),
        ],
    }
}

fn key(app: &mut App, code: KeyCode) -> Vec<Command> {
    app.on_key(KeyEvent::from(code))
}

/// An app that has found device 599 and loaded two points.
pub(super) fn loaded() -> App {
    let (mut app, _) = App::new("10.0.0.255:47808".into());
    app.on_event(Event::Found(vec![device()]));
    app.on_event(Event::DeviceInfo(
        599,
        report(ObjectId::new(8, 599), "Lift gateway", Value::Null),
    ));
    app.on_event(Event::ObjectCount(599, 3));
    app.on_event(Event::Object(
        599,
        report(LOAD, "Car 1 load", Value::Real(420.0)),
    ));
    app.on_event(Event::Object(
        599,
        report(DOOR, "Car 1 door", Value::Enumerated(0)),
    ));
    app.on_event(Event::DeviceDone(599, true));
    app
}

#[test]
fn the_app_starts_by_discovering() {
    let (_, commands) = App::new("x".into());
    assert!(matches!(commands[..], [Command::Discover]));
}

#[test]
fn the_first_device_found_is_selected_loaded_and_its_detail_requested() {
    let (mut app, _) = App::new("x".into());
    let commands = app.on_event(Event::Found(vec![device()]));
    assert_eq!(app.device_table.selected(), Some(0));
    assert!(
        commands
            .iter()
            .any(|c| matches!(c, Command::LoadDevice(d) if d.instance == 599))
    );
    assert!(
        commands
            .iter()
            .any(|c| matches!(c, Command::LoadDetail(_, id) if *id == ObjectId::new(8, 599)))
    );
}

#[test]
fn a_finished_device_is_watched_for_its_points() {
    let (mut app, _) = App::new("x".into());
    app.on_event(Event::Found(vec![device()]));
    app.on_event(Event::Object(
        599,
        report(LOAD, "Car 1 load", Value::Real(1.0)),
    ));
    let commands = app.on_event(Event::DeviceDone(599, true));
    assert!(matches!(&commands[..], [Command::Watch(Some((_, ids)))] if ids == &vec![LOAD]));
}

#[test]
fn moving_through_objects_requests_each_objects_detail() {
    let mut app = loaded();
    key(&mut app, KeyCode::Tab);
    assert_eq!(app.focus, Focus::Objects);
    let commands = key(&mut app, KeyCode::Down);
    assert!(matches!(&commands[..], [Command::LoadDetail(_, id)] if *id == DOOR));
    assert!(
        key(&mut app, KeyCode::Down).is_empty(),
        "the last row does not move further"
    );
}

#[test]
fn a_late_detail_for_an_earlier_selection_is_ignored() {
    let mut app = loaded();
    key(&mut app, KeyCode::Tab);
    key(&mut app, KeyCode::Down);
    app.on_event(Event::Detail(599, report(LOAD, "stale", Value::Null)));
    assert!(app.detail.is_none());
    app.on_event(Event::Detail(
        599,
        report(DOOR, "Car 1 door", Value::Enumerated(0)),
    ));
    assert_eq!(
        app.detail.as_ref().map(ObjectReport::name).as_deref(),
        Some("Car 1 door")
    );
}

#[test]
fn a_changed_value_is_highlighted_and_logged_and_an_unchanged_one_is_not() {
    let mut app = loaded();
    app.on_event(Event::Value(599, LOAD, Ok(Value::Real(420.0))));
    assert!(app.log.is_empty());
    app.on_event(Event::Value(599, LOAD, Ok(Value::Real(610.0))));
    let row = &app.devices[0].objects[0];
    assert!(row.changed.is_some());
    assert_eq!(row.report.present_value(), Some(&Value::Real(610.0)));
    assert!(
        app.log
            .back()
            .is_some_and(|(_, l)| l.contains("420.00 -> 610.00")),
        "{:?}",
        app.log
    );
}

#[test]
fn a_point_going_quiet_is_logged_once_as_offline() {
    let mut app = loaded();
    app.on_event(Event::Value(599, LOAD, Err("no reply".into())));
    app.on_event(Event::Value(599, LOAD, Err("no reply".into())));
    let offline = app
        .log
        .iter()
        .filter(|(_, l)| l.contains("offline"))
        .count();
    assert_eq!(offline, 1);
}

#[test]
fn polling_can_be_switched_off_and_on() {
    let mut app = loaded();
    assert!(matches!(
        &key(&mut app, KeyCode::Char('w'))[..],
        [Command::Watch(None)]
    ));
    assert!(matches!(
        &key(&mut app, KeyCode::Char('w'))[..],
        [Command::Watch(Some(_))]
    ));
}

#[test]
fn the_poll_interval_is_clamped() {
    let mut app = loaded();
    for _ in 0..20 {
        key(&mut app, KeyCode::Char('+'));
    }
    assert_eq!(app.interval, std::time::Duration::from_millis(250));
}

mod edit;
mod render;
mod settings;
