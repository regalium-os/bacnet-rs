// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Opening the change forms from what is selected, and what happens after
//! the device answers: a notice, a log line, and a reload of what changed.

use super::{App, Focus, Load};
use crate::tui::edit::{CreateForm, Edit, Form, Outcome, WriteForm};
use crate::tui::worker::{Change, Command};
use bacnet_probe::{ObjectId, Value, explain, names};
use ratatui::crossterm::event::KeyEvent;
use std::time::Instant;

const PRESENT_VALUE: u32 = 85;
const PRIORITY_ARRAY: u32 = 87;

impl App {
    /// `e` (or `n`, relinquishing): a write of the selected property, or of
    /// the selected object's present-value.
    pub(crate) fn begin_write(&mut self, relinquish: bool) {
        let Some(device) = self.device().map(|d| d.device.clone()) else {
            return self.notify("select a device first", false);
        };
        let target = match self.focus {
            Focus::Properties => self.detail.as_ref().and_then(|d| {
                let (prop, read) = d.reads.get(self.property_table.selected()?)?;
                Some((d.id, *prop, read.clone().unwrap_or_default(), d.labels()))
            }),
            Focus::Objects => self.row().map(|r| {
                let current = r
                    .report
                    .get(PRESENT_VALUE)
                    .map(<[Value]>::to_vec)
                    .unwrap_or_default();
                (r.report.id, PRESENT_VALUE, current, r.report.labels())
            }),
            Focus::Devices => None,
        };
        let Some((object, property, current, labels)) = target else {
            return self.notify("select an object or a property to write", false);
        };
        let slots = self.slots(object);
        if relinquish && (property != PRESENT_VALUE || slots.is_empty()) {
            return self.notify(
                "only a commandable present-value has slots to relinquish",
                false,
            );
        }
        let name = self
            .row()
            .filter(|r| r.report.id == object)
            .map(|r| r.report.name())
            .unwrap_or_default();
        let units = self.row().and_then(|r| r.report.units());
        let shown = if property == PRESENT_VALUE {
            current
                .first()
                .map(|v| explain::present(object.kind, v, units, &labels))
                .unwrap_or_default()
        } else {
            explain::many(property, object.kind, &current, units)
        };
        // A relinquish most often clears the slot in control: the highest.
        let top = slots
            .split_once('=')
            .map(|(slot, _)| slot.to_owned())
            .unwrap_or_default();
        self.edit = Some(Edit::new(Form::Write(WriteForm {
            device,
            object,
            property,
            label: format!("{} {name:?}", names::object(object)),
            current,
            shown,
            states: labels
                .iter()
                .filter_map(|v| {
                    if let Value::Text(s) = v {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .collect(),
            slots,
            value: if relinquish {
                "null".into()
            } else {
                String::new()
            },
            priority: if relinquish { top } else { String::new() },
            field: 0,
        })));
    }

    /// `a`: a new object in the selected device.
    pub(crate) fn begin_create(&mut self) {
        match self.device().map(|d| d.device.clone()) {
            Some(device) => {
                let form = CreateForm {
                    device,
                    kind: 0,
                    name: String::new(),
                    value: String::new(),
                    field: 0,
                };
                self.edit = Some(Edit::new(Form::Create(form)));
            }
            None => self.notify("select a device first", false),
        }
    }

    /// `x`: delete the selected object, after confirmation.
    pub(crate) fn begin_delete(&mut self) {
        match (
            self.focus,
            self.device().map(|d| d.device.clone()),
            self.row().map(|r| r.report.id),
        ) {
            (Focus::Objects, Some(device), Some(object)) => {
                self.edit = Some(Edit::new(Form::Delete(Change::Delete { device, object })));
            }
            _ => self.notify("select an object in the objects pane to delete", false),
        }
    }

    /// A key while a form is open; every key belongs to it.
    pub(crate) fn edit_key(&mut self, key: KeyEvent) -> Vec<Command> {
        let Some(edit) = &mut self.edit else {
            return Vec::new();
        };
        match edit.key(key) {
            Outcome::Open => Vec::new(),
            Outcome::Closed => {
                self.edit = None;
                Vec::new()
            }
            Outcome::Send(change) => {
                self.edit = None;
                self.notify("sending…", true);
                vec![Command::Change(change)]
            }
        }
    }

    /// The device answered a change.
    pub(super) fn changed(
        &mut self,
        instance: u32,
        what: &str,
        structural: bool,
        result: &Result<Option<ObjectId>, String>,
    ) -> Vec<Command> {
        let ok = result.is_ok();
        let line = match result {
            Ok(Some(id)) if structural => format!("{what}: done, {}", names::object(*id)),
            Ok(_) => format!("{what}: done"),
            Err(e) => format!("{what}: refused: {e}"),
        };
        self.log(format!("{instance} {line}"));
        self.notify(&line, ok);
        if !ok {
            return Vec::new();
        }
        if structural {
            // The object list changed: read it again.
            if let Some(entry) = self.device_mut(instance) {
                entry.load = Load::NotLoaded;
                entry.objects.clear();
            }
            return self.device_selected();
        }
        self.reload_detail().into_iter().collect()
    }

    /// The occupied slots of `object`'s priority array, `8=60 16=0`, if the
    /// detail pane has read it.
    fn slots(&self, object: ObjectId) -> String {
        let Some(slots) = self
            .detail
            .as_ref()
            .filter(|d| d.id == object)
            .and_then(|d| d.get(PRIORITY_ARRAY))
        else {
            return String::new();
        };
        slots
            .iter()
            .enumerate()
            .filter(|(_, v)| **v != Value::Null)
            .map(|(i, v)| format!("{}={}", i + 1, names::value(object.kind, v, None)))
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub(crate) fn notify(&mut self, text: &str, ok: bool) {
        self.notice = Some((Instant::now(), text.to_owned(), ok));
    }
}
