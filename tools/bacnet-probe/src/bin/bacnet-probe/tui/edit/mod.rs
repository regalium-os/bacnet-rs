// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The forms that change a device from the TUI: write a property, create an
//! object, delete one. Pure: keys go in, and at most one [`Change`] comes
//! out, only after the user has seen what it will do and pressed `y`.
//!
//! The confirmation is not ceremony. A TUI is browsed with single keys, and
//! a stray `e` and Enter on a live controller would otherwise be a write to
//! a lift; the summary says what, where, and at what priority.

mod form;

pub(crate) use form::{CREATABLE, CreateForm, WriteForm};

use super::worker::Change;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

/// An open form.
pub(crate) struct Edit {
    pub(crate) form: Form,
    /// The summary is showing, waiting for `y`.
    pub(crate) confirming: bool,
    /// Why the last Enter could not be accepted.
    pub(crate) error: Option<String>,
}

/// Which form.
pub(crate) enum Form {
    Write(WriteForm),
    Create(CreateForm),
    /// Nothing to type: it opens on its confirmation.
    Delete(Change),
}

/// What a key did.
#[derive(Debug, PartialEq)]
pub(crate) enum Outcome {
    Open,
    Closed,
    Send(Change),
}

impl Edit {
    pub(crate) fn new(form: Form) -> Self {
        let confirming = matches!(form, Form::Delete(_));
        Self {
            form,
            confirming,
            error: None,
        }
    }

    pub(crate) fn key(&mut self, key: KeyEvent) -> Outcome {
        if self.confirming {
            return match key.code {
                KeyCode::Char('y' | 'Y') => match self.change() {
                    Ok(c) => Outcome::Send(c),
                    Err(e) => self.refuse(e),
                },
                _ if matches!(self.form, Form::Delete(_)) => Outcome::Closed,
                _ => {
                    self.confirming = false;
                    Outcome::Open
                }
            };
        }
        match key.code {
            KeyCode::Esc => return Outcome::Closed,
            KeyCode::Enter => match self.change() {
                Ok(_) => {
                    self.confirming = true;
                    self.error = None;
                }
                Err(e) => return self.refuse(e),
            },
            code => match &mut self.form {
                Form::Write(w) => w.key(code),
                Form::Create(c) => c.key(code),
                Form::Delete(_) => {}
            },
        }
        Outcome::Open
    }

    fn refuse(&mut self, e: String) -> Outcome {
        self.error = Some(e);
        self.confirming = false;
        Outcome::Open
    }

    /// The change the form describes, or why it cannot be sent.
    pub(crate) fn change(&self) -> Result<Change, String> {
        match &self.form {
            Form::Write(w) => w.change(),
            Form::Create(c) => c.change(),
            Form::Delete(c) => Ok(c.clone()),
        }
    }

    /// What `y` will do, in a sentence.
    pub(crate) fn summary(&self) -> String {
        match (&self.form, self.change()) {
            (Form::Write(w), Ok(_)) => w.summary(),
            (Form::Create(c), Ok(_)) => c.summary(),
            (Form::Delete(Change::Delete { device, object }), _) => format!(
                "Delete {} from device {}? Objects cannot be undeleted.",
                bacnet_probe::names::object(*object),
                device.instance
            ),
            (_, Err(e)) => e,
            _ => String::new(),
        }
    }
}

#[cfg(test)]
mod tests;
