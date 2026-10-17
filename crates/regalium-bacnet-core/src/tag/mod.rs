// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! ASHRAE 135 §20.2 tags: the framing every value inside an APDU wears.
//!
//! This layer knows how long an element is and whether it opens or closes
//! a constructed value; it does not know what the contents mean. That is
//! [`crate::primitive`]'s job for application tags, and the service grammar's
//! for context tags, whose type only the grammar knows.
//!
//! # Lossless
//!
//! An [`Element`] keeps its header's spelling — an escaped tag number, a
//! length written long — and borrows its contents, so [`encode_all`] of what
//! [`decode_all`] returned reproduces the input octet for octet. Equipment
//! that does not encode canonically is the normal case, and a codec that
//! tidied it on the way through would hide the difference a site engineer
//! is looking for. [`Header::is_canonical`] says when there is one.

mod element;
mod error;
mod header;
mod traced;
mod wire;

pub use element::{Element, Elements, decode_all, elements, encode_all};
pub use error::{Error, ErrorKind, Reason};
pub use header::{Class, Header, Length, Lvt};
pub use traced::{Decoded, decode_all_traced, encode_all_traced};
