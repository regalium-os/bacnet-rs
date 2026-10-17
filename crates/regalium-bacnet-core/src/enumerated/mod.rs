// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Names for the open-ended `BACnet` enumerations (§21): object types,
//! property identifiers, error classes and codes — spelled as the standard
//! spells them, `analog-input`, `present-value`, `unknown-object`.
//!
//! # Why names, and why a fallback
//!
//! These enumerations are open: numbers above a reserved range belong to
//! vendors, and a revision of the standard adds more. A closed Rust `enum`
//! would refuse a lift controller's proprietary object type, so a number
//! stays a number and these functions name it when they can. A number they
//! cannot name is spelled `type-N`, `property-N`, `class-N` or `code-N`, and
//! every `parse_*` reads that spelling back, so a name is never lossy.
//!
//! The tables are partial where the standard is large: every object type
//! through `lift`, the properties and error codes a client meets, and the
//! event types, states and notify types an alarm is described in.

mod errors;
mod events;
mod object_types;
mod properties;

/// An object type's name, or `None` for one this build cannot name.
#[must_use]
pub fn object_type(number: u16) -> Option<&'static str> {
    object_types::NAMES.get(usize::from(number)).copied()
}

/// An object type by name: a standard name, or `type-N`.
#[must_use]
pub fn parse_object_type(name: &str) -> Option<u16> {
    if let Some(i) = object_types::NAMES.iter().position(|n| *n == name) {
        return u16::try_from(i).ok();
    }
    let n: u16 = name.strip_prefix("type-")?.parse().ok()?;
    (n < 1024).then_some(n)
}

/// A property identifier's name, or `None` for one this build cannot name.
#[must_use]
pub fn property(number: u32) -> Option<&'static str> {
    let names = properties::NAMES;
    let i = names.binary_search_by_key(&number, |(n, _)| *n).ok()?;
    names.get(i).map(|(_, name)| *name)
}

/// A property identifier by name: a standard name, or `property-N`.
#[must_use]
pub fn parse_property(name: &str) -> Option<u32> {
    // A standard name may itself begin `property-` (`property-list`), so the
    // table is consulted first and the numeric spelling only after.
    if let Some((id, _)) = properties::NAMES.iter().find(|(_, n)| *n == name) {
        return Some(*id);
    }
    let n: u32 = name.strip_prefix("property-")?.parse().ok()?;
    (n < 1 << 22).then_some(n)
}

/// An error class's name (§18), or `None` beyond the eight defined.
#[must_use]
pub fn error_class(number: u32) -> Option<&'static str> {
    errors::CLASSES.get(usize::try_from(number).ok()?).copied()
}

/// An error code's name, or `None` for one this build cannot name.
#[must_use]
pub fn error_code(number: u32) -> Option<&'static str> {
    let codes = errors::CODES;
    let i = codes.binary_search_by_key(&number, |(n, _)| *n).ok()?;
    codes.get(i).map(|(_, name)| *name)
}

/// An event type's name (`BACnetEventType`), or `None` for one this build
/// cannot name.
#[must_use]
pub fn event_type(number: u32) -> Option<&'static str> {
    let types = events::EVENT_TYPES;
    let i = types.binary_search_by_key(&number, |(n, _)| *n).ok()?;
    types.get(i).map(|(_, name)| *name)
}

/// An event state's name (`BACnetEventState`).
#[must_use]
pub fn event_state(number: u32) -> Option<&'static str> {
    events::EVENT_STATES
        .get(usize::try_from(number).ok()?)
        .copied()
}

/// A notify type's name (`BACnetNotifyType`).
#[must_use]
pub fn notify_type(number: u32) -> Option<&'static str> {
    events::NOTIFY_TYPES
        .get(usize::try_from(number).ok()?)
        .copied()
}

/// `BACnetSegmentation` (§21), from an I-Am or the Device object's
/// segmentation-supported: which directions a device segments in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Segmentation {
    /// 0: sends and receives segmented messages.
    Both,
    /// 1: sends them only.
    Transmit,
    /// 2: receives them only.
    Receive,
    /// 3: neither.
    NoSegmentation,
    /// A value the standard does not define, kept as sent.
    Other(u32),
}

impl Segmentation {
    /// From the enumerated value.
    #[must_use]
    pub const fn from_u32(n: u32) -> Self {
        match n {
            0 => Self::Both,
            1 => Self::Transmit,
            2 => Self::Receive,
            3 => Self::NoSegmentation,
            n => Self::Other(n),
        }
    }

    /// The enumerated value.
    #[must_use]
    pub const fn to_u32(self) -> u32 {
        match self {
            Self::Both => 0,
            Self::Transmit => 1,
            Self::Receive => 2,
            Self::NoSegmentation => 3,
            Self::Other(n) => n,
        }
    }

    /// Whether the device reassembles a segmented request sent to it.
    #[must_use]
    pub const fn receives(self) -> bool {
        matches!(self, Self::Both | Self::Receive)
    }
}
