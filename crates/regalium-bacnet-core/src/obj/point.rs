// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! One object in a [`Memory`](super::Memory) database: its properties, which
//! of them a write may change, and — for a commandable point — its priority
//! array (§19.2).

use super::errors::{
    INVALID_ARRAY_INDEX, UNKNOWN_PROPERTY, VALUE_OUT_OF_RANGE, WRITE_ACCESS_DENIED,
};
use crate::primitive::Primitive;
use crate::service::ServiceError;
use std::collections::{BTreeMap, BTreeSet};

/// present-value, priority-array, relinquish-default.
pub(crate) const PRESENT_VALUE: u32 = 85;
pub(crate) const PRIORITY_ARRAY: u32 = 87;
pub(crate) const RELINQUISH_DEFAULT: u32 = 104;

/// The NULL that fills an empty slot, and that a write uses to relinquish.
const NULL: [u8; 1] = [0x00];

/// One object.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Point {
    pub(crate) properties: BTreeMap<u32, Vec<u8>>,
    pub(crate) writable: BTreeSet<u32>,
    /// The sixteen command slots, highest priority first, for a point whose
    /// present-value is commandable; `None` for any other.
    pub(crate) slots: Option<[Option<Vec<u8>>; 16]>,
}

impl Point {
    pub(crate) fn set(&mut self, property: u32, value: &Primitive<'_>) {
        let mut octets = Vec::new();
        value.encode(&mut octets);
        self.properties.insert(property, octets);
    }

    /// Every property it has, derived ones included.
    pub(crate) fn names(&self) -> Vec<u32> {
        let mut all: BTreeSet<u32> = self.properties.keys().copied().collect();
        if self.slots.is_some() {
            all.extend([PRESENT_VALUE, PRIORITY_ARRAY]);
        }
        all.into_iter().collect()
    }

    pub(crate) fn read(&self, property: u32, index: Option<u32>) -> Result<Vec<u8>, ServiceError> {
        match (&self.slots, property) {
            (Some(slots), PRESENT_VALUE) => {
                let winner = slots.iter().flatten().next();
                Ok(winner
                    .or_else(|| self.properties.get(&RELINQUISH_DEFAULT))
                    .cloned()
                    .unwrap_or(NULL.to_vec()))
            }
            (Some(slots), PRIORITY_ARRAY) => match index {
                None => Ok(slots
                    .iter()
                    .flat_map(|s| s.as_deref().unwrap_or(&NULL).to_vec())
                    .collect()),
                Some(0) => Ok(unsigned(16)),
                Some(i) => {
                    let slot = slots
                        .get(usize::try_from(i - 1).unwrap_or(usize::MAX))
                        .ok_or(INVALID_ARRAY_INDEX)?;
                    Ok(slot.as_deref().unwrap_or(&NULL).to_vec())
                }
            },
            _ => {
                let value = self.properties.get(&property).ok_or(UNKNOWN_PROPERTY)?;
                if index.is_some() {
                    return Err(INVALID_ARRAY_INDEX);
                }
                Ok(value.clone())
            }
        }
    }

    pub(crate) fn write(
        &mut self,
        property: u32,
        index: Option<u32>,
        value: &[u8],
        priority: Option<u8>,
    ) -> Result<(), ServiceError> {
        if let (Some(slots), PRESENT_VALUE) = (&mut self.slots, property) {
            // No priority is the lowest, 16 (§19.2.1).
            let p = priority.unwrap_or(16);
            let slot = slots
                .get_mut(usize::from(p).wrapping_sub(1))
                .ok_or(VALUE_OUT_OF_RANGE)?;
            *slot = (value != NULL).then(|| value.to_vec());
            return Ok(());
        }
        if !self.writable.contains(&property) {
            let exists =
                self.properties.contains_key(&property) || self.names().contains(&property);
            return Err(if exists {
                WRITE_ACCESS_DENIED
            } else {
                UNKNOWN_PROPERTY
            });
        }
        if index.is_some() {
            return Err(INVALID_ARRAY_INDEX);
        }
        self.properties.insert(property, value.to_vec());
        Ok(())
    }
}

/// An encoded Unsigned.
pub(crate) fn unsigned(n: u32) -> Vec<u8> {
    let mut out = Vec::new();
    Primitive::Unsigned(n.into()).encode(&mut out);
    out
}
