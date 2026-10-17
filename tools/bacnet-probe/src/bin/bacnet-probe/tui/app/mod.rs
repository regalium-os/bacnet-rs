// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! TUI state. Pure: events and keys go in, worker commands come out, so the
//! whole interaction is testable without a terminal or a network.

use super::edit::Edit;
use super::settings::Form;
use super::worker::{Command, Packet};

mod edit;
mod events;
mod select;
mod settings;
use bacnet_probe::scan::ObjectReport;
use bacnet_probe::{Device, ObjectId};
use ratatui::widgets::TableState;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

const MAX_PACKETS: usize = 2000;
const MAX_LOG: usize = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Focus {
    Devices,
    Objects,
    Properties,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tab {
    Browse,
    Packets,
    Log,
    Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Load {
    NotLoaded,
    Loading,
    /// Superseded part-way; reloads when selected again.
    Partial,
    Done,
}

pub(crate) struct DeviceEntry {
    pub(crate) device: Device,
    pub(crate) info: Option<ObjectReport>,
    pub(crate) count: Option<u32>,
    pub(crate) objects: Vec<Row>,
    pub(crate) load: Load,
}

pub(crate) struct Row {
    pub(crate) report: ObjectReport,
    /// When present-value last changed, for highlighting.
    pub(crate) changed: Option<Instant>,
}

#[allow(clippy::struct_excessive_bools)] // Independent UI toggles, not a state machine.
pub(crate) struct App {
    pub(crate) target: String,
    pub(crate) started: Instant,
    pub(crate) devices: Vec<DeviceEntry>,
    pub(crate) device_table: TableState,
    pub(crate) object_table: TableState,
    pub(crate) property_table: TableState,
    pub(crate) packet_table: TableState,
    pub(crate) focus: Focus,
    pub(crate) tab: Tab,
    /// What the properties pane shows, and which object it is waiting for.
    pub(crate) detail: Option<ObjectReport>,
    pub(crate) detail_for: Option<(u32, ObjectId)>,
    pub(crate) packets: VecDeque<Packet>,
    pub(crate) follow: bool,
    pub(crate) log: VecDeque<(Instant, String)>,
    pub(crate) log_scroll: usize,
    pub(crate) discovering: bool,
    /// Followed points fed by COV and by polling, once subscribed.
    pub(crate) following: Option<(usize, usize)>,
    pub(crate) watching: bool,
    pub(crate) interval: Duration,
    /// The Settings tab: the network options, editable.
    pub(crate) settings: Form,
    /// A change being written, created or deleted, if a form is open.
    pub(crate) edit: Option<Edit>,
    /// The last change's outcome, shown in the footer for a few seconds.
    pub(crate) notice: Option<(Instant, String, bool)>,
    pub(crate) help: bool,
    pub(crate) quit: bool,
}

impl App {
    /// A fresh app and the commands that start it: a Who-Is. Its settings
    /// are the defaults until [`App::configured`] says otherwise.
    pub(crate) fn new(target: String) -> (Self, Vec<Command>) {
        let app = Self {
            target,
            started: Instant::now(),
            devices: Vec::new(),
            device_table: TableState::default(),
            object_table: TableState::default(),
            property_table: TableState::default(),
            packet_table: TableState::default(),
            focus: Focus::Devices,
            tab: Tab::Browse,
            detail: None,
            detail_for: None,
            packets: VecDeque::new(),
            follow: true,
            log: VecDeque::new(),
            log_scroll: 0,
            discovering: false,
            following: None,
            watching: true,
            interval: Duration::from_secs(2),
            settings: Form::new(crate::args::defaults(), Duration::from_secs(2)),
            edit: None,
            notice: None,
            help: false,
            quit: false,
        };
        (app, vec![Command::Discover])
    }

    pub(crate) fn device(&self) -> Option<&DeviceEntry> {
        self.devices.get(self.device_table.selected()?)
    }

    pub(super) fn device_mut(&mut self, instance: u32) -> Option<&mut DeviceEntry> {
        self.devices
            .iter_mut()
            .find(|d| d.device.instance == instance)
    }

    pub(crate) fn row(&self) -> Option<&Row> {
        self.device()?.objects.get(self.object_table.selected()?)
    }

    pub(crate) fn log(&mut self, line: String) {
        self.log.push_back((Instant::now(), line));
        if self.log.len() > MAX_LOG {
            self.log.pop_front();
        }
    }

    /// `+12.345s` since the app started.
    pub(crate) fn stamp(&self, at: Instant) -> String {
        format!(
            "+{:.3}s",
            at.saturating_duration_since(self.started).as_secs_f64()
        )
    }
}

#[cfg(test)]
mod tests;
