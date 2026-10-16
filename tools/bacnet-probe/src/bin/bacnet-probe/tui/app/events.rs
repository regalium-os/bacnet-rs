// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The app's reaction to worker events.

use super::{App, DeviceEntry, Focus, Load, MAX_PACKETS, Row};
use crate::tui::worker::{Command, Event};
use bacnet_probe::scan::ObjectReport;
use bacnet_probe::{Device, ObjectId, Value, explain, names, property};
use std::time::Instant;

impl App {
    pub(crate) fn on_event(&mut self, event: Event) -> Vec<Command> {
        match event {
            Event::Discovering => self.discovering = true,
            Event::Found(found) => return self.found(found),
            Event::DeviceInfo(i, info) => {
                if let Some(d) = self.device_mut(i) {
                    d.info = Some(info);
                }
            }
            Event::ObjectCount(i, n) => {
                if let Some(d) = self.device_mut(i) {
                    d.count = Some(n);
                }
            }
            Event::Object(i, report) => return self.object_loaded(i, report),
            Event::DeviceDone(i, complete) => {
                if let Some(d) = self.device_mut(i) {
                    d.load = if complete { Load::Done } else { Load::Partial };
                }
                if self.device().is_some_and(|d| d.device.instance == i) {
                    return self.watch_command().into_iter().collect();
                }
            }
            Event::Detail(i, report) => {
                if self.detail_for == Some((i, report.id)) {
                    self.detail = Some(report);
                }
            }
            Event::Value(i, id, value) => self.value(i, id, value),
            Event::Packet(p) => {
                self.packets.push_back(p);
                if self.packets.len() > MAX_PACKETS {
                    self.packets.pop_front();
                }
                if self.follow {
                    self.packet_table.select(Some(self.packets.len() - 1));
                }
            }
            Event::Following { cov, polled } => self.following = Some((cov, polled)),
            Event::Log(line) => self.log(line),
            Event::Connected(notes) => self.connected(notes),
            Event::ConnectFailed(error) => return self.connect_failed(error),
        }
        Vec::new()
    }

    fn found(&mut self, found: Vec<Device>) -> Vec<Command> {
        self.discovering = false;
        for device in found {
            match self.device_mut(device.instance) {
                Some(entry) => entry.device = device,
                None => self.devices.push(DeviceEntry {
                    device,
                    info: None,
                    count: None,
                    objects: Vec::new(),
                    load: Load::NotLoaded,
                }),
            }
        }
        self.devices.sort_by_key(|d| d.device.instance);
        if self.device_table.selected().is_none() && !self.devices.is_empty() {
            self.device_table.select(Some(0));
            return self.device_selected();
        }
        Vec::new()
    }

    fn object_loaded(&mut self, instance: u32, report: ObjectReport) -> Vec<Command> {
        let selected = self.device().is_some_and(|d| d.device.instance == instance);
        let Some(entry) = self.device_mut(instance) else {
            return Vec::new();
        };
        match entry.objects.iter_mut().find(|r| r.report.id == report.id) {
            Some(row) => row.report = report,
            None => entry.objects.push(Row {
                report,
                changed: None,
            }),
        }
        if selected && self.object_table.selected().is_none() {
            self.object_table.select(Some(0));
            if self.focus != Focus::Devices {
                return self.detail_command().into_iter().collect();
            }
        }
        Vec::new()
    }

    fn value(&mut self, instance: u32, id: ObjectId, value: Result<Value, String>) {
        let now = Instant::now();
        let mut line = None;
        if let Some(row) = self
            .device_mut(instance)
            .and_then(|d| d.objects.iter_mut().find(|r| r.report.id == id))
        {
            let before = row.report.present_value().cloned();
            let show =
                |v: &Value| explain::present(id.kind, v, row.report.units(), &row.report.labels());
            match (&before, &value) {
                (Some(b), Ok(v)) if b != v => {
                    line = Some(format!(
                        "{instance} {} {}: {} -> {}",
                        names::object(id),
                        row.report.name(),
                        show(b),
                        show(v)
                    ));
                    row.changed = Some(now);
                }
                (Some(_), Err(e)) if row.report.error(property::PRESENT_VALUE).is_none() => {
                    line = Some(format!(
                        "{instance} {} {}: offline ({e})",
                        names::object(id),
                        row.report.name()
                    ));
                }
                _ => {}
            }
            row.report
                .set(property::PRESENT_VALUE, value.clone().map(|v| vec![v]));
        }
        if self.detail_for == Some((instance, id))
            && let Some(detail) = &mut self.detail
        {
            detail.set(property::PRESENT_VALUE, value.map(|v| vec![v]));
        }
        if let Some(line) = line {
            self.log(line);
        }
    }
}
