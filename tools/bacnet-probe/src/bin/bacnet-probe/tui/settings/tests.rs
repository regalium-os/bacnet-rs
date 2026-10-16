// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

//! The form as a state machine: keys in, an apply or an error out, with no
//! terminal and no network.

use super::edit::Outcome;
use super::{FIELDS, Field, Form, Status, command_line};
use crate::args::{self, Net};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

fn form() -> Form {
    Form::new(args::defaults(), Duration::from_secs(2))
}

fn press(form: &mut Form, code: KeyCode) -> Outcome {
    form.on_key(KeyEvent::from(code))
}

/// Selects `field`, clears it, types `text` and keeps it.
fn set(form: &mut Form, field: Field, text: &str) {
    form.selected = FIELDS.iter().position(|f| *f == field).unwrap();
    press(form, KeyCode::Enter);
    form.on_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
    text.chars()
        .for_each(|c| drop(press(form, KeyCode::Char(c))));
    press(form, KeyCode::Enter);
}

fn error(form: &Form, field: Field) -> Option<String> {
    let i = FIELDS.iter().position(|f| *f == field).unwrap();
    form.errors[i].clone()
}

fn applied(form: &mut Form) -> (Net, Duration) {
    match press(form, KeyCode::Char('a')) {
        Outcome::Apply(net, interval) => (net, interval),
        other => panic!("expected an apply, got {other:?}; errors {:?}", form.errors),
    }
}

#[test]
fn an_unedited_form_applies_exactly_what_it_was_given() {
    let mut form = form();
    assert!(!form.dirty());
    assert_eq!(
        applied(&mut form),
        (args::defaults(), Duration::from_secs(2))
    );
}

#[test]
fn every_setting_reads_back_into_the_option_it_mirrors() {
    let mut form = form();
    set(&mut form, Field::Target, "10.0.0.255");
    set(&mut form, Field::Port, "47809");
    set(&mut form, Field::Bind, "0.0.0.0:47809");
    set(&mut form, Field::Scan, "192.168.1.*, 10.1.0.0/16");
    set(&mut form, Field::Bbmd, "10.1.0.1");
    set(&mut form, Field::Timeout, "0.5");
    set(&mut form, Field::Interval, "1");
    form.selected = FIELDS.iter().position(|f| *f == Field::Unicast).unwrap();
    press(&mut form, KeyCode::Char(' '));
    assert!(form.dirty());
    let (net, interval) = applied(&mut form);
    assert_eq!(net.target.to_string(), "10.0.0.255");
    assert_eq!((net.port, net.unicast), (47809, true));
    assert_eq!(net.bind, Some("0.0.0.0:47809".parse().unwrap()));
    assert_eq!(crate::cli::scopes(&net.scan), "192.168.1.0/24, 10.1.0.0/16");
    assert_eq!(net.bbmd, Some("10.1.0.1:47808".parse().unwrap()));
    assert_eq!(net.timeout, Some(Duration::from_millis(500)));
    assert_eq!(interval, Duration::from_secs(1));
}

#[test]
fn an_ipv6_address_is_refused_by_name_rather_than_half_used() {
    let mut form = form();
    set(&mut form, Field::Target, "fe80::1");
    set(&mut form, Field::Bbmd, "[2001:db8::1]:47808");
    set(&mut form, Field::Bind, "[::]:47808");
    for field in [Field::Target, Field::Bbmd, Field::Bind] {
        let why = error(&form, field).unwrap();
        assert!(why.contains("Annex U"), "{field:?}: {why}");
    }
    assert_eq!(press(&mut form, KeyCode::Char('a')), Outcome::Handled);
    assert!(matches!(form.status, Status::Failed(_)));
}

#[test]
fn a_unicast_sweep_too_large_to_send_is_refused_beside_the_ranges() {
    let mut form = form();
    set(&mut form, Field::Scan, "10.0.0.0/8");
    form.selected = FIELDS.iter().position(|f| *f == Field::Unicast).unwrap();
    press(&mut form, KeyCode::Enter);
    assert_eq!(press(&mut form, KeyCode::Char('a')), Outcome::Handled);
    assert!(error(&form, Field::Scan).unwrap().contains("limited to"));
}

#[test]
fn while_typing_every_key_belongs_to_the_field_and_escape_undoes_it() {
    let mut form = form();
    press(&mut form, KeyCode::Enter);
    for c in ['q', '1', 'a'] {
        assert_eq!(press(&mut form, KeyCode::Char(c)), Outcome::Handled);
    }
    assert_eq!(form.text[0], "255.255.255.255q1a");
    press(&mut form, KeyCode::Esc);
    assert_eq!(form.text[0], "255.255.255.255");
    assert_eq!(press(&mut form, KeyCode::Char('q')), Outcome::Unhandled);
}

#[test]
fn undo_discards_edits_and_defaults_fill_in_what_clap_would() {
    let mut net = args::defaults();
    net.port = 47809;
    let mut form = Form::new(net, Duration::from_secs(2));
    set(&mut form, Field::Target, "10.0.0.1");
    press(&mut form, KeyCode::Char('u'));
    assert!(!form.dirty());
    press(&mut form, KeyCode::Char('d'));
    assert_eq!(applied(&mut form).0, args::defaults());
}

#[test]
fn an_interval_changed_by_plus_and_minus_follows_into_the_form() {
    let mut form = form();
    form.sync_interval(Duration::from_secs(1));
    assert!(!form.dirty());
    assert_eq!(applied(&mut form).1, Duration::from_secs(1));
}

#[test]
fn the_command_line_names_only_what_differs_from_the_defaults() {
    assert_eq!(
        command_line(&args::defaults(), Duration::from_secs(2)),
        "bacnet-probe"
    );
    let mut net = args::defaults();
    net.scan = vec!["10.1.2.*".parse().unwrap(), "10.1.3.4".parse().unwrap()];
    net.unicast = true;
    net.bbmd = Some("10.1.0.1:47808".parse().unwrap());
    assert_eq!(
        command_line(&net, Duration::from_secs(1)),
        "bacnet-probe tui --interval 1 --scan 10.1.2.0/24 --scan 10.1.3.4/32 \
         --unicast --bbmd 10.1.0.1:47808"
    );
}
