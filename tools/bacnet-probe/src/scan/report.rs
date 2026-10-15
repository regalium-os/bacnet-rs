// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! One object's properties as read, ready for any renderer.

use crate::property;
use crate::tag::{ObjectId, Value};

/// What was read from one object. Properties the device does not implement
/// (unknown-property) are omitted; any other failure is kept as an error.
#[derive(Clone, Debug, Default)]
pub struct ObjectReport {
    /// The object.
    pub id: ObjectId,
    /// Each property read, in request order, with its values or why not.
    pub reads: Vec<(u32, Result<Vec<Value>, String>)>,
}

impl ObjectReport {
    /// The values of `prop`, if it was read successfully.
    #[must_use]
    pub fn get(&self, prop: u32) -> Option<&[Value]> {
        self.reads
            .iter()
            .find(|(p, _)| *p == prop)
            .and_then(|(_, r)| r.as_deref().ok())
    }

    /// Why `prop` could not be read, if it failed.
    #[must_use]
    pub fn error(&self, prop: u32) -> Option<&str> {
        self.reads
            .iter()
            .find(|(p, _)| *p == prop)
            .and_then(|(_, r)| r.as_ref().err())
            .map(String::as_str)
    }

    /// `object-name`, or empty.
    #[must_use]
    pub fn name(&self) -> String {
        match self.get(property::OBJECT_NAME) {
            Some([Value::Text(s)]) => s.clone(),
            _ => String::new(),
        }
    }

    /// `units`, for an analog point.
    #[must_use]
    pub fn units(&self) -> Option<u32> {
        match self.get(property::UNITS) {
            Some([Value::Enumerated(u)]) => Some(*u),
            _ => None,
        }
    }

    /// The device's own words for each state: state-text for a multi-state
    /// point (1-based), or `[inactive-text, active-text]` for a binary one.
    #[must_use]
    pub fn labels(&self) -> Vec<Value> {
        match self.id.kind {
            3..=5 => match (self.get(46), self.get(4)) {
                (Some([off]), Some([on])) => vec![off.clone(), on.clone()],
                _ => Vec::new(),
            },
            13 | 14 | 19 => self.get(110).map(<[Value]>::to_vec).unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    /// The first value of `present-value`.
    #[must_use]
    pub fn present_value(&self) -> Option<&Value> {
        self.get(property::PRESENT_VALUE).and_then(<[Value]>::first)
    }

    /// Replaces (or adds) one property's result, e.g. a polled present-value.
    pub fn set(&mut self, prop: u32, result: Result<Vec<Value>, String>) {
        match self.reads.iter_mut().find(|(p, _)| *p == prop) {
            Some(slot) => slot.1 = result,
            None => self.reads.push((prop, result)),
        }
    }
}
