// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The Settings tab's form: every network option as editable text, checked
//! as a whole before anything is applied. Pure, like the rest of the app:
//! keys go in, and an [`edit::Outcome::Apply`] comes out for the app to turn
//! into worker commands.
//!
//! Nothing changes on the wire until the user applies. A half-typed address
//! never reaches the socket, and a form that fails to parse keeps every
//! field's error beside the field, rather than the first one in a log.

pub(crate) mod edit;
mod fields;
mod line;

pub(crate) use fields::{FIELDS, Field, INTERVAL};
pub(crate) use line::command_line;

use crate::args::Net;
use std::time::Duration;

/// Where the last apply got to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Status {
    /// Nothing applied since the tab opened.
    Idle,
    /// Sent to the worker; the socket is being reopened.
    Applying,
    /// The worker confirmed.
    Applied,
    /// Refused, by the form or by the network; the old settings stand.
    Failed(String),
}

/// The form. `net` and `interval` are what the worker runs with; `text`
/// is what the user has typed, one entry per [`FIELDS`].
pub(crate) struct Form {
    pub(crate) net: Net,
    pub(crate) interval: Duration,
    pub(crate) text: Vec<String>,
    pub(crate) errors: Vec<Option<String>>,
    pub(crate) selected: usize,
    /// The selected field is being typed into, and what it held before.
    pub(crate) editing: Option<String>,
    pub(crate) status: Status,
    /// Requested and not yet confirmed by the worker.
    pub(crate) pending: Option<Net>,
    /// How the socket was last opened: its address, a registration.
    pub(crate) notes: Vec<String>,
}

impl Form {
    /// A form showing `net` and `interval`, unedited.
    pub(crate) fn new(net: Net, interval: Duration) -> Self {
        let text = FIELDS.iter().map(|f| f.show(&net, interval)).collect();
        Self {
            net,
            interval,
            text,
            errors: vec![None; FIELDS.len()],
            selected: 0,
            editing: None,
            status: Status::Idle,
            pending: None,
            notes: Vec::new(),
        }
    }

    pub(crate) fn field(&self) -> Field {
        FIELDS.get(self.selected).copied().unwrap_or(Field::Target)
    }

    /// Whether field `i` differs from what is running.
    pub(crate) fn changed(&self, i: usize) -> bool {
        match (FIELDS.get(i), self.text.get(i)) {
            (Some(f), Some(t)) => f.show(&self.net, self.interval) != t.trim(),
            _ => false,
        }
    }

    pub(crate) fn dirty(&self) -> bool {
        (0..FIELDS.len()).any(|i| self.changed(i))
    }

    /// Shows `net` and `interval` in every field, discarding edits.
    pub(crate) fn reset_to(&mut self, net: &Net, interval: Duration) {
        for (text, f) in self.text.iter_mut().zip(FIELDS) {
            *text = f.show(net, interval);
        }
        self.errors.fill(None);
        self.editing = None;
    }

    /// Follows an interval changed outside the form, by `+` or `-`, unless
    /// the user has typed one of their own.
    pub(crate) fn sync_interval(&mut self, interval: Duration) {
        let i = FIELDS.iter().position(|f| *f == Field::Interval);
        if let Some(text) = i
            .filter(|i| !self.changed(*i))
            .and_then(|i| self.text.get_mut(i))
        {
            *text = Field::Interval.show(&self.net, interval);
        }
        self.interval = interval;
    }

    /// Every field read into options, or each field's error beside it. The
    /// discovery plan is checked too, so a sweep too large to send is
    /// refused here rather than by the worker.
    pub(crate) fn parse(&mut self) -> Option<(Net, Duration)> {
        let mut net = self.net.clone();
        let mut interval = self.interval;
        for ((f, text), error) in FIELDS.iter().zip(&self.text).zip(&mut self.errors) {
            *error = f.parse(text, &mut net, &mut interval).err();
        }
        if self.errors.iter().all(Option::is_none)
            && let Err(e) = crate::cli::plan(&net)
        {
            let scan = FIELDS.iter().position(|f| *f == Field::Scan);
            if let Some(error) = scan.and_then(|i| self.errors.get_mut(i)) {
                *error = Some(e.to_string());
            }
        }
        self.errors
            .iter()
            .all(Option::is_none)
            .then_some((net, interval))
    }
}

#[cfg(test)]
mod tests;
