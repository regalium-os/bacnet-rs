// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! What a type may put on a span, declared beside the type with
//! [`trace_attrs!`](crate::trace_attrs).
//!
//! There is no way to attach an arbitrary value to a span: only a field a
//! declaration names, under the key it names. A property value read from
//! equipment cannot reach a trace through a well-meaning change in a hot
//! path, because nothing in the seam would accept it.

use crate::attribute::{Attribute, Key};

/// A type whose declared fields can be recorded on a span, a log line or a
/// metric's labels.
pub trait Traced {
    /// Calls `out` once per declared attribute. Each borrows from `self`,
    /// so a caller may keep them as long as `self` lives.
    fn attributes<'s>(&'s self, out: &mut dyn FnMut(Attribute<'s>));

    /// Every declared attribute, collected: a metric's label set, or the
    /// attributes of a log line.
    fn attribute_list(&self) -> Vec<Attribute<'_>> {
        let mut list = Vec::new();
        self.attributes(&mut |a| list.push(a));
        list
    }
}

/// A field type an attribute can carry: integers, flags and text.
pub trait Field {
    /// This value as an attribute under `key`.
    fn attribute(&self, key: Key) -> Attribute<'_>;
}

macro_rules! int_field {
    ($($t:ty),*) => {$(
        impl Field for $t {
            fn attribute(&self, key: Key) -> Attribute<'_> {
                Attribute::int(key, i64::try_from(*self).unwrap_or(i64::MAX))
            }
        }
    )*};
}

int_field!(u8, u16, u32, u64, usize, i32, i64);

impl Field for bool {
    fn attribute(&self, key: Key) -> Attribute<'_> {
        Attribute::bool(key, *self)
    }
}

impl Field for &str {
    fn attribute(&self, key: Key) -> Attribute<'_> {
        Attribute::str(key, self)
    }
}

impl Field for String {
    fn attribute(&self, key: Key) -> Attribute<'_> {
        Attribute::str(key, self)
    }
}

/// Declares which fields of a type a span may record, and under which key:
///
/// ```
/// use regalium_bacnet_telemetry::{keys, trace_attrs};
///
/// struct Decoded {
///     octets: usize,
///     elements: usize,
/// }
///
/// trace_attrs! { Decoded => { octets: keys::TAG_OCTETS, elements: keys::TAG_ELEMENTS } }
/// ```
///
/// A field not listed is never recorded. That is the safety mechanism, not
/// an oversight: there is no other way onto a span.
#[macro_export]
macro_rules! trace_attrs {
    ($ty:ty => { $($field:ident : $key:expr),* $(,)? }) => {
        impl $crate::Traced for $ty {
            fn attributes<'s>(&'s self, out: &mut dyn FnMut($crate::Attribute<'s>)) {
                $( out($crate::Field::attribute(&self.$field, $key)); )*
            }
        }
    };
}
