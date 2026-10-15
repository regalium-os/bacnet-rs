// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Attributes: declared keys, and the values they may carry.
//!
//! A [`Key`] is a `const` declared in [`keys`](crate::keys); an [`Attribute`]
//! can only be built from one. Labels therefore have names a reviewer can
//! find in one file, and a property *value* read from equipment cannot become
//! a label by accident — a value is what a metric records, never what it is
//! labelled with. That keeps series cardinality bounded by the building, not
//! by the data, and keeps a credential typed into a property out of every
//! exporter.

/// An attribute name, declared once as a `const`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Key(&'static str);

impl Key {
    /// Declares a key. Call only in a `const` item, so every key in the
    /// program is listed somewhere a reviewer can read.
    #[must_use]
    pub const fn declare(name: &'static str) -> Self {
        Self(name)
    }

    /// The name an exporter emits, e.g. `bacnet.device.instance`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.0
    }
}

/// What an attribute carries. Borrowed: building one allocates nothing, which
/// is what lets the no-op path cost nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttributeValue<'a> {
    /// Text, e.g. an object name or a unit.
    Str(&'a str),
    /// An integer, e.g. a device instance.
    Int(i64),
    /// A flag.
    Bool(bool),
}

/// One label on a recording.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Attribute<'a> {
    /// Which declared key.
    pub key: Key,
    /// Its value for this recording.
    pub value: AttributeValue<'a>,
}

impl<'a> Attribute<'a> {
    /// A text label.
    #[must_use]
    pub const fn str(key: Key, value: &'a str) -> Self {
        Self {
            key,
            value: AttributeValue::Str(value),
        }
    }

    /// An integer label.
    #[must_use]
    pub const fn int(key: Key, value: i64) -> Self {
        Self {
            key,
            value: AttributeValue::Int(value),
        }
    }

    /// A flag label.
    #[must_use]
    pub const fn bool(key: Key, value: bool) -> Self {
        Self {
            key,
            value: AttributeValue::Bool(value),
        }
    }
}
