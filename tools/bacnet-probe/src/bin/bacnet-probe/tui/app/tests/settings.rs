// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The Settings tab through the app: keys become a `Reconfigure`, and the
//! worker's answer decides whether the devices on screen are forgotten.

use super::loaded;
use crate::tui::app::{App, Tab};
use crate::tui::settings::Status;
use crate::tui::ui;
use crate::tui::worker::{Command, Event};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::{Terminal, backend::TestBackend};
use std::time::Duration;

fn keys(app: &mut App, codes: &[KeyCode]) -> Vec<Command> {
    codes
        .iter()
        .flat_map(|c| app.on_key(KeyEvent::from(*c)))
        .collect()
}

fn typed(text: &str) -> Vec<KeyCode> {
    text.chars().map(KeyCode::Char).collect()
}

/// Opens the tab and retypes the target as `target`.
fn retarget(app: &mut App, target: &str) -> Vec<Command> {
    let mut codes = vec![KeyCode::Char('s'), KeyCode::Enter];
    codes.extend(std::iter::repeat_n(KeyCode::Backspace, 20));
    codes.extend(typed(target));
    codes.extend([KeyCode::Enter, KeyCode::Char('a')]);
    keys(app, &codes)
}

#[test]
fn a_typed_target_is_applied_as_a_reconfigure_without_switching_tabs() {
    let mut app = loaded();
    let commands = retarget(&mut app, "10.0.0.255");
    assert_eq!(
        app.tab,
        Tab::Settings,
        "the 1 typed into the address stayed there"
    );
    assert!(
        matches!(&commands[..], [Command::Reconfigure(net)] if net.target.to_string() == "10.0.0.255"),
        "{commands:?}"
    );
    assert_eq!(app.settings.status, Status::Applying);
}

#[test]
fn a_confirmed_change_forgets_the_old_networks_devices_and_names_the_new_target() {
    let mut app = loaded();
    retarget(&mut app, "10.0.0.255");
    app.on_event(Event::Connected(vec!["listening on 0.0.0.0:47808".into()]));
    assert!(app.devices.is_empty() && app.detail.is_none());
    assert_eq!(app.target, "10.0.0.255");
    assert_eq!(app.settings.net.target.to_string(), "10.0.0.255");
    assert_eq!(app.settings.status, Status::Applied);
    assert!(!app.settings.dirty());
}

#[test]
fn a_refused_change_keeps_the_devices_and_resumes_following_them() {
    let mut app = loaded();
    retarget(&mut app, "10.0.0.255");
    let commands = app.on_event(Event::ConnectFailed("port in use".into()));
    assert_eq!(app.devices.len(), 1);
    assert!(matches!(&commands[..], [Command::Watch(Some(_))]));
    assert!(matches!(&app.settings.status, Status::Failed(e) if e == "port in use"));
    assert_ne!(app.settings.net.target.to_string(), "10.0.0.255");
}

#[test]
fn changing_only_the_interval_does_not_reopen_the_socket() {
    let mut app = loaded();
    let mut codes = vec![KeyCode::Char('s'), KeyCode::End];
    codes.extend(std::iter::repeat_n(KeyCode::Down, 20));
    codes.push(KeyCode::Enter);
    codes.extend(std::iter::repeat_n(KeyCode::Backspace, 5));
    codes.extend([KeyCode::Char('1'), KeyCode::Enter, KeyCode::Char('a')]);
    let commands = keys(&mut app, &codes);
    assert!(matches!(&commands[..], [Command::SetInterval(d)] if *d == Duration::from_secs(1)));
    assert_eq!(app.interval, Duration::from_secs(1));
}

#[test]
fn the_settings_tab_draws_every_setting_and_the_command_line() {
    let mut app = loaded();
    keys(&mut app, &[KeyCode::Char('s')]);
    let mut terminal = Terminal::new(TestBackend::new(160, 40)).unwrap();
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let screen: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect();
    for want in [
        "4 Settings",
        "Who-Is target",
        "255.255.255.255",
        "Scan ranges",
        "BBMD",
        "Running as",
        "bacnet-probe",
    ] {
        assert!(screen.contains(want), "missing {want:?}");
    }
}
