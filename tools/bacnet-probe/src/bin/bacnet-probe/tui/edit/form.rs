// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The two forms with fields: a write, and a new object. Each reads its text
//! the way `bacnet-probe write` does, through the same parser, so a value
//! the TUI accepts is one the CLI would.

use crate::cli::resource::input;
use crate::tui::worker::Change;
use bacnet_probe::service::write::PropertyValue;
use bacnet_probe::{Device, ObjectId, Value, api, names, props};
use ratatui::crossterm::event::KeyCode;

/// Object types the create form offers: the ones controllers commonly let a
/// commissioning tool add.
pub(crate) const CREATABLE: [u16; 3] = [2, 5, 19];

/// Writing one property.
pub(crate) struct WriteForm {
    pub(crate) device: Device,
    pub(crate) object: ObjectId,
    pub(crate) property: u32,
    /// `analog-output:1 "Machine room fan speed"`.
    pub(crate) label: String,
    /// What the property holds, decoded; it decides how `value` is read.
    pub(crate) current: Vec<Value>,
    /// The current value as a person reads it.
    pub(crate) shown: String,
    /// The object's state names, for writing a state by name.
    pub(crate) states: Vec<String>,
    /// The occupied slots of its priority array, `8=60`, if commandable.
    pub(crate) slots: String,
    pub(crate) value: String,
    pub(crate) priority: String,
    /// 0 is value, 1 is priority.
    pub(crate) field: usize,
}

impl WriteForm {
    pub(super) fn key(&mut self, code: KeyCode) {
        let text = if self.field == 0 {
            &mut self.value
        } else {
            &mut self.priority
        };
        match code {
            KeyCode::Tab | KeyCode::Down | KeyCode::Up | KeyCode::BackTab => {
                self.field = 1 - self.field;
            }
            KeyCode::Backspace => drop(text.pop()),
            KeyCode::Char(c) => text.push(c),
            _ => {}
        }
    }

    pub(super) fn change(&self) -> Result<Change, String> {
        if self.current.len() > 1 {
            return Err("this property holds a list; write it with `bacnet-probe write`".into());
        }
        let now = self.current.first().map(api::to_schema);
        let typed = input::parse(&self.value, now.as_ref(), &self.states)?;
        let values = api::from_schema(&typed).map_err(|e| e.message)?;
        let priority = match self.priority.trim() {
            "" => None,
            p => Some(
                p.parse()
                    .ok()
                    .filter(|p| (1..=16).contains(p))
                    .ok_or("priority is 1 (highest) to 16")?,
            ),
        };
        Ok(Change::Write {
            device: self.device.clone(),
            object: self.object,
            property: self.property,
            values,
            priority,
        })
    }

    pub(super) fn summary(&self) -> String {
        let at = match self.priority.trim() {
            "" => String::new(),
            p => format!(" at priority {p}"),
        };
        let what = if self.value.trim().eq_ignore_ascii_case("null") {
            "Relinquish (write null)".to_owned()
        } else {
            format!("Write {}", self.value.trim())
        };
        format!(
            "{what} to {} of {}{at}, on device {}?",
            props::name(self.property),
            self.label,
            self.device.instance
        )
    }
}

/// A new object, its instance chosen by the device.
pub(crate) struct CreateForm {
    pub(crate) device: Device,
    /// Index into [`CREATABLE`].
    pub(crate) kind: usize,
    pub(crate) name: String,
    pub(crate) value: String,
    /// 0 is the type, 1 the name, 2 the initial value.
    pub(crate) field: usize,
}

impl CreateForm {
    pub(crate) fn kind(&self) -> u16 {
        CREATABLE.get(self.kind).copied().unwrap_or(2)
    }

    pub(super) fn key(&mut self, code: KeyCode) {
        match (self.field, code) {
            (_, KeyCode::Tab | KeyCode::Down) => self.field = (self.field + 1) % 3,
            (_, KeyCode::BackTab | KeyCode::Up) => self.field = (self.field + 2) % 3,
            (0, KeyCode::Right | KeyCode::Char(' ')) => {
                self.kind = (self.kind + 1) % CREATABLE.len();
            }
            (0, KeyCode::Left) => self.kind = (self.kind + CREATABLE.len() - 1) % CREATABLE.len(),
            (1, KeyCode::Backspace) => drop(self.name.pop()),
            (2, KeyCode::Backspace) => drop(self.value.pop()),
            (1, KeyCode::Char(c)) => self.name.push(c),
            (2, KeyCode::Char(c)) => self.value.push(c),
            _ => {}
        }
    }

    pub(super) fn change(&self) -> Result<Change, String> {
        let mut initial = Vec::new();
        if !self.name.trim().is_empty() {
            initial.push(PropertyValue::new(
                77,
                vec![Value::Text(self.name.trim().to_owned())],
            ));
        }
        if !self.value.trim().is_empty() {
            // Read as the type the new object's present-value will hold.
            let (seed, states) = match self.kind() {
                2 => (Value::Real(0.0), Vec::new()),
                5 => (
                    Value::Enumerated(0),
                    vec!["inactive".to_owned(), "active".to_owned()],
                ),
                _ => (Value::Unsigned(1), Vec::new()),
            };
            let typed = input::parse(&self.value, Some(&api::to_schema(&seed)), &states)?;
            initial.push(PropertyValue::new(
                85,
                api::from_schema(&typed).map_err(|e| e.message)?,
            ));
        }
        Ok(Change::Create {
            device: self.device.clone(),
            kind: self.kind(),
            initial,
        })
    }

    pub(super) fn summary(&self) -> String {
        let named = if self.name.trim().is_empty() {
            String::new()
        } else {
            format!(" named {:?}", self.name.trim())
        };
        let valued = if self.value.trim().is_empty() {
            String::new()
        } else {
            format!(" at {}", self.value.trim())
        };
        format!(
            "Create a {}{named}{valued} on device {}?",
            names::object_type(self.kind()),
            self.device.instance
        )
    }
}
