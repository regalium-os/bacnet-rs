// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! What a change of selection asks the worker for: load, follow, detail.

use super::{App, Focus, Load};
use crate::tui::worker::Command;
use bacnet_probe::{ObjectId, names, property};
use ratatui::widgets::TableState;

impl App {
    /// The device selection changed: load it if needed, watch it, detail it.
    pub(crate) fn device_selected(&mut self) -> Vec<Command> {
        let mut commands = Vec::new();
        self.object_table = TableState::default();
        let Some(index) = self.device_table.selected() else {
            return commands;
        };
        if let Some(entry) = self.devices.get_mut(index) {
            if !entry.objects.is_empty() {
                self.object_table.select(Some(0));
            }
            if matches!(entry.load, Load::NotLoaded | Load::Partial) {
                entry.load = Load::Loading;
                commands.push(Command::LoadDevice(entry.device.clone()));
            }
        }
        commands.extend(self.watch_command());
        commands.extend(self.detail_command());
        commands
    }

    /// Asks for the detail of whatever the focus points at, if not already shown.
    pub(crate) fn detail_command(&mut self) -> Option<Command> {
        let device = self.device()?.device.clone();
        let id = match self.focus {
            Focus::Devices => ObjectId::new(8, device.instance),
            Focus::Objects | Focus::Properties => self.row()?.report.id,
        };
        if self.detail_for == Some((device.instance, id)) {
            return None;
        }
        self.detail_for = Some((device.instance, id));
        self.detail = None;
        self.property_table = TableState::default().with_selected(Some(0));
        Some(Command::LoadDetail(device, id))
    }

    /// Re-requests the current detail even if it is already shown.
    pub(crate) fn reload_detail(&mut self) -> Option<Command> {
        self.detail_for = None;
        self.detail_command()
    }

    /// What the worker should poll: the selected device's points.
    pub(crate) fn watch_command(&self) -> Option<Command> {
        if !self.watching {
            return Some(Command::Watch(None));
        }
        let entry = self.device()?;
        let points: Vec<ObjectId> = entry
            .objects
            .iter()
            .filter(|r| {
                names::is_point(r.report.id.kind) && r.report.get(property::PRESENT_VALUE).is_some()
            })
            .map(|r| r.report.id)
            .collect();
        Some(Command::Watch(
            (!points.is_empty()).then(|| (entry.device.clone(), points)),
        ))
    }
}
