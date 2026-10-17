// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The object model a local device serves from (Clause 12): what objects it
//! has, what their properties hold, and what a write may change.
//!
//! [`Database`] is the boundary, declared here because the core is its
//! consumer: [`crate::server`] answers requests from any database, so a
//! device whose points live in a PLC, a file or another protocol implements
//! the trait and is served the same as [`Memory`].
//!
//! Values cross the boundary as encoded elements, as they travel: a
//! property's type is the object's business, and a database that already
//! holds octets need not decode them to serve them.

mod errors;
mod memory;
mod point;

pub use errors::{
    INVALID_ARRAY_INDEX, INVALID_DATA_TYPE, UNKNOWN_OBJECT, UNKNOWN_PROPERTY, VALUE_OUT_OF_RANGE,
    WRITE_ACCESS_DENIED,
};
pub use memory::Memory;

use crate::primitive::ObjectIdentifier;
use crate::service::ServiceError;

/// What a local device serves from.
///
/// # Concurrency
///
/// Reads take `&self` and writes `&mut self`: a server holds the database
/// and answers one request at a time, which is what keeps a write and the
/// read that follows it consistent.
pub trait Database {
    /// The Device object: the device's own identifier.
    fn device(&self) -> ObjectIdentifier;

    /// Every object, the Device object first.
    fn objects(&self) -> Vec<ObjectIdentifier>;

    /// Every property `object` has, for `ReadPropertyMultiple` of `all`.
    ///
    /// # Errors
    ///
    /// [`UNKNOWN_OBJECT`] when there is no such object.
    fn properties(&self, object: ObjectIdentifier) -> Result<Vec<u32>, ServiceError>;

    /// The value of `property` of `object`, or of one element when `index`
    /// is given (0 is the array's length), as encoded elements.
    ///
    /// # Errors
    ///
    /// [`UNKNOWN_OBJECT`], [`UNKNOWN_PROPERTY`], [`INVALID_ARRAY_INDEX`].
    fn read(
        &self,
        object: ObjectIdentifier,
        property: u32,
        index: Option<u32>,
    ) -> Result<Vec<u8>, ServiceError>;

    /// Writes `value`, encoded elements, at `priority` when the property is
    /// commandable.
    ///
    /// # Errors
    ///
    /// [`WRITE_ACCESS_DENIED`] for what is not writable, and the read errors.
    fn write(
        &mut self,
        object: ObjectIdentifier,
        property: u32,
        index: Option<u32>,
        value: &[u8],
        priority: Option<u8>,
    ) -> Result<(), ServiceError>;
}
