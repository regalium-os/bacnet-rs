// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Event notifications (§13.8, §13.9): an alarm raised, cleared or
//! acknowledged, sent to every recipient of the object's notification class.
//!
//! The event values (`[12]`) are a choice of some twenty structures, one per
//! event algorithm, and vendors add their own; they stay octets here, as a
//! property value does, for whoever knows the algorithm to read.

use super::fields::Fields;
use super::values::{boolean, enumerated, object, put_constructed, text, unsigned};
use crate::error::{Error, ErrorKind};
use crate::primitive::{CharacterString, Date, ObjectIdentifier, Primitive, Time, tag};

/// When an event happened (`BACnetTimeStamp`, §21): a time of day, a
/// sequence number, or a date and time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TimeStamp {
    /// `[0]` a time of day.
    Time(Time),
    /// `[1]` a sequence number, for a device without a clock.
    Sequence(u32),
    /// `[2]` a date and a time.
    DateTime(Date, Time),
}

impl TimeStamp {
    fn read(octets: &[u8]) -> Result<Self, Error<'_>> {
        let mut f = Fields::new(octets);
        let stamp = if let Some(Primitive::Time(t)) = f.optional(0, tag::TIME)? {
            Self::Time(t)
        } else if let Some(n) = f.optional(1, tag::UNSIGNED)? {
            Self::Sequence(unsigned(&n)?)
        } else {
            let mut dt = Fields::new(f.constructed(2)?);
            let (Primitive::Date(d), Primitive::Time(t)) = (dt.application()?, dt.application()?)
            else {
                return Err(Error::new(
                    ErrorKind::InvalidValue { tag: tag::DATE },
                    octets,
                    0,
                ));
            };
            dt.end()?;
            Self::DateTime(d, t)
        };
        f.end()?;
        Ok(stamp)
    }

    fn write(&self, out: &mut Vec<u8>) {
        match self {
            Self::Time(t) => Primitive::Time(*t).encode_context(0, out),
            Self::Sequence(n) => Primitive::Unsigned((*n).into()).encode_context(1, out),
            Self::DateTime(d, t) => {
                let mut dt = Vec::new();
                Primitive::Date(*d).encode(&mut dt);
                Primitive::Time(*t).encode(&mut dt);
                put_constructed(2, &dt, out);
            }
        }
    }
}

/// An event notification. Borrows the message text and event values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EventNotification<'a> {
    /// `[0]` the recipient's process identifier.
    pub process: u32,
    /// `[1]` the device reporting.
    pub device: ObjectIdentifier,
    /// `[2]` the object the event is about.
    pub object: ObjectIdentifier,
    /// `[3]` when it happened.
    pub time: TimeStamp,
    /// `[4]` the notification class, which says who is told.
    pub class: u32,
    /// `[5]` 0 most urgent to 255.
    pub priority: u8,
    /// `[6]` `BACnetEventType`: the algorithm that raised it.
    pub event_type: u32,
    /// `[7]` text for whoever reads the alarm.
    pub message: Option<CharacterString<'a>>,
    /// `[8]` 0 alarm, 1 event, 2 acknowledgement notification.
    pub notify_type: u32,
    /// `[9]` whether an operator must acknowledge it.
    pub ack_required: Option<bool>,
    /// `[10]` the event state it left.
    pub from_state: Option<u32>,
    /// `[11]` the event state it entered: 0 normal, 1 fault, 2 off-normal,
    /// 3 high-limit, 4 low-limit, 5 life-safety-alarm.
    pub to_state: u32,
    /// `[12]` the algorithm's values, undecoded.
    pub values: Option<&'a [u8]>,
}

impl<'a> EventNotification<'a> {
    /// Decodes the service request. Borrows `body`.
    ///
    /// # Errors
    ///
    /// A missing or mistyped field, an unpaired tag, octets after.
    pub fn decode(body: &'a [u8]) -> Result<Self, Error<'a>> {
        let mut f = Fields::new(body);
        let req = |f: &mut Fields<'a>, n, t| f.required(n, t);
        let n = Self {
            process: unsigned(&req(&mut f, 0, tag::UNSIGNED)?)?,
            device: object(&req(&mut f, 1, tag::OBJECT_IDENTIFIER)?)?,
            object: object(&req(&mut f, 2, tag::OBJECT_IDENTIFIER)?)?,
            time: TimeStamp::read(f.constructed(3)?)?,
            class: unsigned(&req(&mut f, 4, tag::UNSIGNED)?)?,
            priority: u8::try_from(unsigned(&req(&mut f, 5, tag::UNSIGNED)?)?).unwrap_or(u8::MAX),
            event_type: enumerated(&req(&mut f, 6, tag::ENUMERATED)?)?,
            message: f
                .optional(7, tag::CHARACTER_STRING)?
                .map(|m| text(&m))
                .transpose()?,
            notify_type: enumerated(&req(&mut f, 8, tag::ENUMERATED)?)?,
            ack_required: f
                .optional(9, tag::BOOLEAN)?
                .map(|b| boolean(&b))
                .transpose()?,
            from_state: f
                .optional(10, tag::ENUMERATED)?
                .map(|s| enumerated(&s))
                .transpose()?,
            to_state: enumerated(&req(&mut f, 11, tag::ENUMERATED)?)?,
            values: f.optional_constructed(12)?,
        };
        f.end()?;
        Ok(n)
    }

    /// Appends the service request.
    pub fn encode(&self, out: &mut Vec<u8>) {
        Primitive::Unsigned(self.process.into()).encode_context(0, out);
        Primitive::ObjectIdentifier(self.device).encode_context(1, out);
        Primitive::ObjectIdentifier(self.object).encode_context(2, out);
        let mut stamp = Vec::new();
        self.time.write(&mut stamp);
        put_constructed(3, &stamp, out);
        Primitive::Unsigned(self.class.into()).encode_context(4, out);
        Primitive::Unsigned(self.priority.into()).encode_context(5, out);
        Primitive::Enumerated(self.event_type).encode_context(6, out);
        if let Some(m) = self.message {
            Primitive::CharacterString(m).encode_context(7, out);
        }
        Primitive::Enumerated(self.notify_type).encode_context(8, out);
        if let Some(a) = self.ack_required {
            Primitive::Boolean(a).encode_context(9, out);
        }
        if let Some(s) = self.from_state {
            Primitive::Enumerated(s).encode_context(10, out);
        }
        Primitive::Enumerated(self.to_state).encode_context(11, out);
        if let Some(v) = self.values {
            put_constructed(12, v, out);
        }
    }
}
