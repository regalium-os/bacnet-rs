// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Change of value (§13.1, §13.14, §13.15, §13.6–§13.7): subscribing, and the
//! notifications a subscription brings.
//!
//! A subscription carries a lifetime so a device can forget a subscriber
//! that went away without saying so; the subscriber renews it before it
//! lapses. A lifetime of 0 is indefinite, which is why no device should be
//! trusted to keep one.

use super::fields::Fields;
use super::reference::{PropertyReference, PropertyValue};
use super::values::{boolean, object, put_constructed, real, unsigned};
use crate::error::Error;
use crate::primitive::{ObjectIdentifier, Primitive, tag};

/// A `SubscribeCOV`, or its cancellation. `Copy`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SubscribeCov {
    /// `[0]` the subscriber's number for this subscription, echoed in every
    /// notification so it can tell its subscriptions apart.
    pub process: u32,
    /// `[1]` the object watched.
    pub object: ObjectIdentifier,
    /// `[2]` whether notifications are confirmed. Absent with `[3]`: cancel.
    pub confirmed: Option<bool>,
    /// `[3]` seconds until the device forgets the subscription; 0 never.
    pub lifetime: Option<u32>,
}

impl SubscribeCov {
    /// Decodes the service request.
    ///
    /// # Errors
    ///
    /// A missing `[0]` or `[1]`, a mistyped field, or octets after.
    pub fn decode(body: &[u8]) -> Result<Self, Error<'_>> {
        let mut f = Fields::new(body);
        let s = Self::fields(&mut f)?;
        f.end()?;
        Ok(s)
    }

    fn fields<'a>(f: &mut Fields<'a>) -> Result<Self, Error<'a>> {
        Ok(Self {
            process: unsigned(&f.required(0, tag::UNSIGNED)?)?,
            object: object(&f.required(1, tag::OBJECT_IDENTIFIER)?)?,
            confirmed: f
                .optional(2, tag::BOOLEAN)?
                .map(|b| boolean(&b))
                .transpose()?,
            lifetime: f
                .optional(3, tag::UNSIGNED)?
                .map(|l| unsigned(&l))
                .transpose()?,
        })
    }

    /// Whether this cancels the subscription rather than making one.
    #[must_use]
    pub const fn is_cancellation(&self) -> bool {
        self.confirmed.is_none() && self.lifetime.is_none()
    }

    /// Appends the service request.
    pub fn encode(&self, out: &mut Vec<u8>) {
        Primitive::Unsigned(self.process.into()).encode_context(0, out);
        Primitive::ObjectIdentifier(self.object).encode_context(1, out);
        if let Some(c) = self.confirmed {
            Primitive::Boolean(c).encode_context(2, out);
        }
        if let Some(l) = self.lifetime {
            Primitive::Unsigned(l.into()).encode_context(3, out);
        }
    }
}

/// A `SubscribeCOVProperty`: one property, with an increment of its own.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SubscribeCovProperty {
    /// `[0]`–`[3]` as [`SubscribeCov`].
    pub subscription: SubscribeCov,
    /// `[4]` the property watched.
    pub property: PropertyReference,
    /// `[5]` how far a REAL must move to be reported — so a noisy analogue
    /// reports movement, not its last digit.
    pub increment: Option<f32>,
}

impl SubscribeCovProperty {
    /// Decodes the service request.
    ///
    /// # Errors
    ///
    /// A missing field, a mistyped one, or octets after.
    pub fn decode(body: &[u8]) -> Result<Self, Error<'_>> {
        let mut f = Fields::new(body);
        let subscription = SubscribeCov::fields(&mut f)?;
        let property = PropertyReference::read(&mut Fields::new(f.constructed(4)?), 0)?;
        let increment = f.optional(5, tag::REAL)?.map(|r| real(&r)).transpose()?;
        f.end()?;
        Ok(Self {
            subscription,
            property,
            increment,
        })
    }

    /// Appends the service request.
    pub fn encode(&self, out: &mut Vec<u8>) {
        self.subscription.encode(out);
        let mut p = Vec::new();
        self.property.write(0, &mut p);
        put_constructed(4, &p, out);
        if let Some(i) = self.increment {
            Primitive::Real(i).encode_context(5, out);
        }
    }
}

/// A COV notification, confirmed or not: the values that changed.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CovNotification<'a> {
    /// `[0]` the subscriber's process identifier.
    pub process: u32,
    /// `[1]` the device notifying.
    pub device: ObjectIdentifier,
    /// `[2]` the object whose value changed.
    pub object: ObjectIdentifier,
    /// `[3]` seconds left of the subscription; 0 when indefinite.
    pub time_remaining: u32,
    /// `[4]` the values: for most objects present-value and status-flags.
    pub values: Vec<PropertyValue<'a>>,
}

impl<'a> CovNotification<'a> {
    /// Decodes the service request. Borrows `body` for the values.
    ///
    /// # Errors
    ///
    /// A missing field or an unpaired tag.
    pub fn decode(body: &'a [u8]) -> Result<Self, Error<'a>> {
        let mut f = Fields::new(body);
        let n = Self {
            process: unsigned(&f.required(0, tag::UNSIGNED)?)?,
            device: object(&f.required(1, tag::OBJECT_IDENTIFIER)?)?,
            object: object(&f.required(2, tag::OBJECT_IDENTIFIER)?)?,
            time_remaining: unsigned(&f.required(3, tag::UNSIGNED)?)?,
            values: PropertyValue::list(f.constructed(4)?)?,
        };
        f.end()?;
        Ok(n)
    }

    /// Appends the service request.
    pub fn encode(&self, out: &mut Vec<u8>) {
        Primitive::Unsigned(self.process.into()).encode_context(0, out);
        Primitive::ObjectIdentifier(self.device).encode_context(1, out);
        Primitive::ObjectIdentifier(self.object).encode_context(2, out);
        Primitive::Unsigned(self.time_remaining.into()).encode_context(3, out);
        let mut values = Vec::new();
        self.values.iter().for_each(|v| v.write(&mut values));
        put_constructed(4, &values, out);
    }
}
