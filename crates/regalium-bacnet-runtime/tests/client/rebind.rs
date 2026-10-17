// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! A device that went quiet and came back somewhere else: sought by Who-Is
//! while the client listens, bound at its new address, reported reachable.

use super::client;
use regalium_bacnet_core::primitive::{ObjectIdentifier, Primitive};
use regalium_bacnet_runtime::Event;
use std::sync::atomic::Ordering;
use std::time::Duration;

const AI1: ObjectIdentifier = ObjectIdentifier {
    object_type: 0,
    instance: 1,
};

#[test]
fn a_device_that_moved_is_found_again_and_read_at_its_new_address() {
    let mut c = client();
    c.who_is(None, Duration::from_millis(50)).unwrap();
    assert_eq!(c.device(599).unwrap().address, 1);
    let _ = c.events().count();

    c.link().silent.store(true, Ordering::SeqCst);
    assert!(c.read_property(599, AI1, 85).is_err());
    c.link().silent.store(false, Ordering::SeqCst);
    c.link().moved.store(true, Ordering::SeqCst);
    // The test client seeks a quiet device 50 ms after it goes quiet.
    c.listen(Duration::from_millis(120)).unwrap();

    assert_eq!(c.device(599).unwrap().address, 3, "bound where it now is");
    let events: Vec<_> = c.events().collect();
    assert!(
        matches!(
            events[..],
            [Event::Offline { instance: 599, .. }, Event::Reachable(599)]
        ),
        "{events:?}"
    );
    assert_eq!(
        c.read_property(599, AI1, 85).unwrap().primitive(),
        Some(Primitive::Real(21.5))
    );
}

#[test]
fn a_device_never_found_is_sought_less_and_less_often() {
    let mut c = client();
    c.who_is(None, Duration::from_millis(50)).unwrap();
    c.link().silent.store(true, Ordering::SeqCst);
    assert!(c.read_property(599, AI1, 85).is_err());
    let before = c.link().sent.lock().unwrap().len();
    // Sought at 50 ms and at 150 ms (50 + 100), not again before 350 ms.
    c.listen(Duration::from_millis(250)).unwrap();
    let asked = c.link().sent.lock().unwrap().len() - before;
    assert_eq!(asked, 2, "backed off");
}
