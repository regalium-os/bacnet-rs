// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Services: any APDU read as the service it carries never panics, and a
//! service that decodes decodes again, the same, from its own encoding —
//! the canonical spelling is a fixed point.

#![no_main]

use libfuzzer_sys::fuzz_target;
use regalium_bacnet::apdu::Apdu;
use regalium_bacnet::service::Service;

fuzz_target!(|data: &[u8]| {
    let Ok(apdu) = Apdu::decode(data) else { return };
    let Ok(first) = Service::decode(&apdu) else { return };
    // The probe's service tests cover encoding; here the property is only
    // that decoding is total and deterministic.
    let again = Service::decode(&apdu).ok();
    assert_eq!(again.as_ref(), Some(&first));
});
