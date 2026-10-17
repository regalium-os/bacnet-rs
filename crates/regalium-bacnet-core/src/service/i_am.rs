// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! I-Am (§16.1): a device announcing itself, in answer to Who-Is or when it
//! starts.

use super::fields::{Fields, enumerated, object, unsigned};
use crate::enumerated::Segmentation;
use crate::error::Error;
use crate::primitive::{ObjectIdentifier, Primitive};

/// An I-Am: who the device is, and what a client must know before asking
/// it anything — how large an APDU it takes, whether it segments, and whose
/// equipment it is. This is everything [`crate::tsm::Remote`] needs.
///
/// `Copy` plain data: safe to share and send.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IAm {
    /// The Device object's identifier: type 8 and the device instance.
    pub device: ObjectIdentifier,
    /// Max-APDU-length-accepted, in octets.
    pub max_apdu: u32,
    /// Segmentation-supported.
    pub segmentation: Segmentation,
    /// Vendor-identifier, ASHRAE's registry number.
    pub vendor_id: u32,
}

impl IAm {
    /// Decodes the service request: four application-tagged values in
    /// order. Copies them; keeps no borrow.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::MissingField`](crate::ErrorKind::MissingField) or a
    /// primitive's error for a missing or mistyped value;
    /// [`ErrorKind::Trailing`](crate::ErrorKind::Trailing) after the fourth.
    pub fn decode(body: &[u8]) -> Result<Self, Error<'_>> {
        let mut f = Fields::new(body);
        let device = object(&f.application()?)?;
        let max_apdu = unsigned(&f.application()?)?;
        let segmentation = Segmentation::from_u32(enumerated(&f.application()?)?);
        let vendor_id = unsigned(&f.application()?)?;
        f.end()?;
        Ok(Self {
            device,
            max_apdu,
            segmentation,
            vendor_id,
        })
    }

    /// Appends the service request, canonically.
    pub fn encode(&self, out: &mut Vec<u8>) {
        Primitive::ObjectIdentifier(self.device).encode(out);
        Primitive::Unsigned(self.max_apdu.into()).encode(out);
        Primitive::Enumerated(self.segmentation.to_u32()).encode(out);
        Primitive::Unsigned(self.vendor_id.into()).encode(out);
    }
}
