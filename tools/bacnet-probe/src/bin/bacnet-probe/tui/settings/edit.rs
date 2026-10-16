// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Keys on the Settings tab. Two modes: moving between fields, where the
//! app's own keys (tabs, quit, help) still work, and typing into one, where
//! every key belongs to the field — an address is made of the digits that
//! otherwise switch tabs.

use super::{FIELDS, Form, Status};
use crate::args::{self, Net};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

/// What a key did to the form.
#[derive(Debug, PartialEq)]
pub(crate) enum Outcome {
    /// Not a settings key; the app's own bindings decide.
    Unhandled,
    /// Consumed, with nothing for the worker.
    Handled,
    /// Every field parsed; reopen with these.
    Apply(Net, Duration),
}

/// The tab's bindings, for the help overlay and the tab's own key hints.
pub(crate) const KEYS: &[(&str, &str)] = &[
    ("↑ ↓  j k", "move between settings"),
    (
        "Enter  e",
        "edit a setting; Enter again to keep it, Esc to undo",
    ),
    ("Space", "toggle an on/off setting"),
    ("Ctrl-U", "while editing: clear the field"),
    ("a", "apply: reopen the socket and rediscover"),
    ("u  d", "undo every edit / fill in the defaults"),
];

impl Form {
    pub(crate) fn on_key(&mut self, key: KeyEvent) -> Outcome {
        if self.editing.is_some() {
            self.type_key(key);
            return Outcome::Handled;
        }
        match key.code {
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => self.step(1),
            KeyCode::Up | KeyCode::Char('k') | KeyCode::BackTab => self.step(-1),
            KeyCode::Enter | KeyCode::Char('e' | ' ') if self.field().is_toggle() => self.toggle(),
            KeyCode::Enter | KeyCode::Char('e') => {
                self.editing = self.text.get(self.selected).cloned();
            }
            KeyCode::Char('a') => return self.apply(),
            KeyCode::Char('u') => {
                let (net, interval) = (self.net.clone(), self.interval);
                self.reset_to(&net, interval);
            }
            KeyCode::Char('d') => self.reset_to(&args::defaults(), Duration::from_secs(2)),
            _ => return Outcome::Unhandled,
        }
        Outcome::Handled
    }

    fn type_key(&mut self, key: KeyEvent) {
        let Some(text) = self.text.get_mut(self.selected) else {
            self.editing = None;
            return;
        };
        match key.code {
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => text.clear(),
            KeyCode::Char(c) => text.push(c),
            KeyCode::Backspace => drop(text.pop()),
            KeyCode::Esc => {
                if let Some(before) = self.editing.take() {
                    *text = before;
                }
            }
            KeyCode::Enter | KeyCode::Tab | KeyCode::Down | KeyCode::Up => {
                self.editing = None;
                self.check();
                match key.code {
                    KeyCode::Tab | KeyCode::Down => self.step(1),
                    KeyCode::Up => self.step(-1),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    /// Re-checks the selected field alone, so its error shows as it is left.
    fn check(&mut self) {
        let (mut net, mut interval) = (self.net.clone(), self.interval);
        let field = self.field();
        let error = self
            .text
            .get(self.selected)
            .and_then(|t| field.parse(t, &mut net, &mut interval).err());
        if let Some(slot) = self.errors.get_mut(self.selected) {
            *slot = error;
        }
    }

    fn toggle(&mut self) {
        if let Some(text) = self.text.get_mut(self.selected) {
            *text = if text == "on" { "off" } else { "on" }.into();
        }
    }

    fn step(&mut self, delta: isize) {
        let last = FIELDS.len().saturating_sub(1);
        self.selected = self.selected.saturating_add_signed(delta).min(last);
    }

    fn apply(&mut self) -> Outcome {
        if let Some((net, interval)) = self.parse() {
            return Outcome::Apply(net, interval);
        }
        let n = self.errors.iter().flatten().count();
        self.status = Status::Failed(format!("{n} setting(s) need fixing first"));
        Outcome::Handled
    }
}
