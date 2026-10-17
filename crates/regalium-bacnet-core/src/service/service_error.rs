// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The error a service returns in an Error PDU (§18, §21 `Error`): a class
//! and a code, for `ReadProperty` and every service whose error is plain.

use super::fields::Fields;
use super::values::enumerated;
use crate::enumerated::{error_class, error_code};
use crate::error::Error;
use crate::primitive::Primitive;

/// An error class and code. Classes say where the fault lies — device,
/// object, property, resources, security, services — so a client can tell
/// "ask again later" from "never ask that".
///
/// `Copy` plain data: safe to share and send.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ServiceError {
    /// `BACnetErrorClass`.
    pub class: u32,
    /// `BACnetErrorCode`.
    pub code: u32,
}

impl ServiceError {
    /// Decodes two application-tagged enumerated values. Keeps no borrow.
    ///
    /// # Errors
    ///
    /// A missing or mistyped value, or octets after the code.
    pub fn decode(body: &[u8]) -> Result<Self, Error<'_>> {
        let mut f = Fields::new(body);
        let class = enumerated(&f.application()?)?;
        let code = enumerated(&f.application()?)?;
        f.end()?;
        Ok(Self { class, code })
    }

    /// Appends the error, canonically.
    pub fn encode(&self, out: &mut Vec<u8>) {
        Primitive::Enumerated(self.class).encode(out);
        Primitive::Enumerated(self.code).encode(out);
    }

    /// `property: unknown-property`, or `class-N: code-N` for numbers
    /// this build cannot name.
    #[must_use]
    pub fn describe(&self) -> String {
        let class =
            error_class(self.class).map_or_else(|| format!("class-{}", self.class), str::to_owned);
        let code =
            error_code(self.code).map_or_else(|| format!("code-{}", self.code), str::to_owned);
        format!("{class}: {code}")
    }
}
