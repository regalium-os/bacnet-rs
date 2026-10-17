// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

//! The forms on their own: typing, review, `y`, and the ways out.

use super::{CreateForm, Edit, Form, Outcome, WriteForm};
use crate::tui::worker::Change;
use bacnet_probe::{Device, ObjectId, Value};
use ratatui::crossterm::event::{KeyCode, KeyEvent};

fn device() -> Device {
    Device {
        instance: 599,
        addr: "10.0.0.5:47808".parse().unwrap(),
        route: None,
        max_apdu: 1476,
        vendor: 999,
    }
}

fn fan(current: Value, states: &[&str]) -> Edit {
    Edit::new(Form::Write(WriteForm {
        device: device(),
        object: ObjectId::new(1, 1),
        property: 85,
        label: "analog-output:1 \"Fan\"".into(),
        current: vec![current],
        shown: String::new(),
        states: states.iter().map(|s| (*s).to_owned()).collect(),
        slots: "8=60".into(),
        value: String::new(),
        priority: String::new(),
        field: 0,
    }))
}

fn keys(edit: &mut Edit, keys: &str) -> Outcome {
    let mut last = Outcome::Open;
    for c in keys.chars() {
        last = edit.key(KeyEvent::from(match c {
            '\n' => KeyCode::Enter,
            '\t' => KeyCode::Tab,
            '\u{1b}' => KeyCode::Esc,
            '→' => KeyCode::Right,
            c => KeyCode::Char(c),
        }));
    }
    last
}

#[test]
fn nothing_is_sent_until_the_summary_has_been_seen_and_confirmed() {
    let mut edit = fan(Value::Real(60.0), &[]);
    assert_eq!(keys(&mut edit, "75\t8\n"), Outcome::Open);
    assert!(edit.confirming);
    assert!(
        edit.summary()
            .contains("Write 75 to present-value of analog-output:1"),
        "{}",
        edit.summary()
    );
    let Outcome::Send(Change::Write {
        values, priority, ..
    }) = keys(&mut edit, "y")
    else {
        panic!("y should send");
    };
    assert_eq!((values, priority), (vec![Value::Real(75.0)], Some(8)));
}

#[test]
fn any_key_but_y_goes_back_to_the_form_and_escape_closes_it() {
    let mut edit = fan(Value::Real(60.0), &[]);
    keys(&mut edit, "75\n");
    assert_eq!(keys(&mut edit, "n"), Outcome::Open);
    assert!(!edit.confirming, "back to typing");
    assert_eq!(keys(&mut edit, "\u{1b}"), Outcome::Closed);
}

#[test]
fn text_the_property_cannot_hold_is_refused_before_review() {
    let mut edit = fan(Value::Real(60.0), &[]);
    keys(&mut edit, "fast\n");
    assert!(!edit.confirming);
    assert!(edit.error.as_deref().unwrap().contains("not a number"));
    let mut edit = fan(Value::Real(60.0), &[]);
    keys(&mut edit, "1\t17\n");
    assert!(edit.error.as_deref().unwrap().contains("1 (highest) to 16"));
}

#[test]
fn a_state_is_written_by_name_and_q_types_rather_than_quits() {
    let mut edit = fan(
        Value::Unsigned(2),
        &["Out of service", "Normal", "Independent"],
    );
    let Outcome::Send(Change::Write { values, .. }) = keys(&mut edit, "Independent\ny") else {
        panic!("should send");
    };
    assert_eq!(values, [Value::Unsigned(3)]);
}

#[test]
fn a_create_cycles_its_type_and_reads_the_value_as_that_type() {
    let form = CreateForm {
        device: device(),
        kind: 0,
        name: String::new(),
        value: String::new(),
        field: 0,
    };
    let mut edit = Edit::new(Form::Create(form));
    let out = keys(&mut edit, "→\tDoor\tactive\ny");
    let Outcome::Send(Change::Create { kind, initial, .. }) = out else {
        panic!("should send");
    };
    assert_eq!(kind, 5, "analog-value then binary-value");
    assert_eq!(initial.len(), 2);
    assert_eq!(initial[1].values, [Value::Enumerated(1)]);
}

#[test]
fn a_delete_opens_on_its_confirmation_and_any_other_key_closes_it() {
    let change = Change::Delete {
        device: device(),
        object: ObjectId::new(2, 1),
    };
    let mut edit = Edit::new(Form::Delete(change.clone()));
    assert!(edit.confirming);
    assert_eq!(keys(&mut edit, "n"), Outcome::Closed);
    let mut edit = Edit::new(Form::Delete(change.clone()));
    assert_eq!(keys(&mut edit, "y"), Outcome::Send(change));
}
