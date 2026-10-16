// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The app's side of the Settings tab: turning an applied form into worker
//! commands, and the worker's answer back into state.
//!
//! The devices on screen were found with the old settings, so they are
//! forgotten only once the new socket is confirmed open. A refused change
//! leaves the browse tab exactly as it was, still live.

use super::{App, Focus, Tab};
use crate::args::Net;
use crate::tui::settings::{Form, Status, edit::Outcome};
use crate::tui::worker::Command;
use ratatui::crossterm::event::KeyEvent;
use ratatui::widgets::TableState;
use std::time::Duration;

impl App {
    /// Starts the form at the options the TUI was opened with.
    #[must_use]
    pub(crate) fn configured(mut self, net: Net, interval: Duration, notes: Vec<String>) -> Self {
        self.target = crate::connect::label(&net);
        self.interval = interval;
        self.settings = Form::new(net, interval);
        for note in &notes {
            self.log(note.clone());
        }
        self.settings.notes = notes;
        self
    }

    /// A key on the Settings tab; `None` when it is not the form's.
    pub(crate) fn settings_key(&mut self, key: KeyEvent) -> Option<Vec<Command>> {
        if self.tab != Tab::Settings {
            return None;
        }
        match self.settings.on_key(key) {
            Outcome::Unhandled => None,
            Outcome::Handled => Some(Vec::new()),
            Outcome::Apply(net, interval) => Some(self.apply_settings(net, interval)),
        }
    }

    fn apply_settings(&mut self, net: Net, interval: Duration) -> Vec<Command> {
        let mut commands = Vec::new();
        if interval != self.interval {
            commands.extend(self.set_interval(interval));
        }
        if net == self.settings.net {
            self.settings.status = Status::Applied;
            return commands;
        }
        self.log(format!(
            "applying settings: {}",
            crate::tui::settings::command_line(&net, interval)
        ));
        self.settings.status = Status::Applying;
        self.settings.pending = Some(net.clone());
        commands.push(Command::Reconfigure(net));
        commands
    }

    /// The worker reopened the socket with the pending settings and is
    /// discovering through it.
    pub(super) fn connected(&mut self, notes: Vec<String>) {
        if let Some(net) = self.settings.pending.take() {
            self.target = crate::connect::label(&net);
            self.settings.net = net;
        }
        self.settings.status = Status::Applied;
        self.devices.clear();
        self.device_table = TableState::default();
        self.object_table = TableState::default();
        self.property_table = TableState::default();
        self.focus = Focus::Devices;
        self.detail = None;
        self.detail_for = None;
        self.following = None;
        for note in &notes {
            self.log(note.clone());
        }
        self.settings.notes = notes;
    }

    /// The worker could not open the new settings and went back to the old
    /// ones; following resumes, since reopening stopped it.
    pub(super) fn connect_failed(&mut self, error: String) -> Vec<Command> {
        self.settings.pending = None;
        self.log(format!("settings not applied: {error}"));
        self.settings.status = Status::Failed(error);
        self.watch_command().into_iter().collect()
    }
}
