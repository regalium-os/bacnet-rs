// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The simulator's writes, refused the way a controller refuses them, so the
//! probe's error handling meets real error codes rather than a mock's.
//!
//! A commandable object (one with a priority-array) is written through its
//! priority array (§19.2): present-value is the highest-priority slot that
//! holds a value, else relinquish-default. An input is written only while it
//! is out of service, when its value is decoupled from the physical point.

use super::Sim;
use crate::service::write::PropertyValue;
use crate::tag::{ObjectId, Value};
use std::mem::discriminant;

/// Properties no client may write: identity, structure, and what the device
/// computes for itself.
const READ_ONLY: &[u32] = &[
    11, 12, 36, 44, 62, 70, 75, 76, 79, 87, 96, 97, 98, 107, 111, 112, 120, 121, 139, 155,
];

const PRESENT_VALUE: u32 = 85;
const OUT_OF_SERVICE: u32 = 81;
const PRIORITY_ARRAY: u32 = 87;
const RELINQUISH_DEFAULT: u32 = 104;
const NUMBER_OF_STATES: u32 = 74;

/// (error-class, error-code).
type Refusal = (u32, u32);

impl Sim {
    /// Applies one write, or says why not.
    pub(super) fn write(&mut self, object: ObjectId, w: &PropertyValue) -> Result<(), Refusal> {
        if !self.props.contains_key(&(object, 77)) {
            return Err((1, 31)); // object: unknown-object
        }
        let p = w.property.property;
        let current = self.props.get(&(object, p)).ok_or((2, 32))?.clone();
        if READ_ONLY.contains(&p) || w.property.index.is_some() {
            return Err((2, 40)); // property: write-access-denied
        }
        if p == PRESENT_VALUE {
            return self.write_present(object, w);
        }
        if current.len() == 1 && w.values.len() != 1 || !same_type(&current, &w.values) {
            return Err((2, 9)); // property: invalid-data-type
        }
        if p == OUT_OF_SERVICE && w.values.first() == Some(&Value::Bool(true)) {
            self.pinned.insert(object);
        }
        self.props.insert((object, p), w.values.clone());
        Ok(())
    }

    fn write_present(&mut self, object: ObjectId, w: &PropertyValue) -> Result<(), Refusal> {
        let [value] = w.values.as_slice() else {
            return Err((2, 9));
        };
        let current = self
            .props
            .get(&(object, PRESENT_VALUE))
            .cloned()
            .unwrap_or_default();
        if let Some(mut slots) = self.props.get(&(object, PRIORITY_ARRAY)).cloned() {
            let priority = usize::from(w.priority.unwrap_or(16));
            if !(1..=16).contains(&priority) {
                return Err((2, 37)); // property: value-out-of-range
            }
            if *value != Value::Null && !same_type(&current, std::slice::from_ref(value)) {
                return Err((2, 9));
            }
            if let Some(slot) = slots.get_mut(priority - 1) {
                *slot = value.clone();
            }
            let winner = slots.iter().find(|v| **v != Value::Null).cloned();
            let fallback = self
                .props
                .get(&(object, RELINQUISH_DEFAULT))
                .and_then(|v| v.first().cloned());
            self.props.insert((object, PRIORITY_ARRAY), slots);
            self.props.insert(
                (object, PRESENT_VALUE),
                winner.or(fallback).into_iter().collect(),
            );
        } else {
            let input = matches!(object.kind, 0 | 3 | 13);
            let out_of_service =
                self.props.get(&(object, OUT_OF_SERVICE)) == Some(&vec![Value::Bool(true)]);
            if input && !out_of_service {
                return Err((2, 40)); // an input follows its sensor
            }
            if !same_type(&current, std::slice::from_ref(value)) {
                return Err((2, 9));
            }
            if let (Value::Unsigned(state), Some([Value::Unsigned(states)])) = (
                value,
                self.props
                    .get(&(object, NUMBER_OF_STATES))
                    .map(Vec::as_slice),
            ) && !(1..=*states).contains(state)
            {
                return Err((2, 37));
            }
            self.props
                .insert((object, PRESENT_VALUE), vec![value.clone()]);
        }
        self.pinned.insert(object);
        Ok(())
    }
}

/// Whether `new` holds values of the kinds `current` does: a device refuses a
/// REAL written to an enumerated property, whatever its number says.
fn same_type(current: &[Value], new: &[Value]) -> bool {
    match current.first() {
        None => true,
        Some(c) => new.iter().all(|v| discriminant(v) == discriminant(c)),
    }
}
