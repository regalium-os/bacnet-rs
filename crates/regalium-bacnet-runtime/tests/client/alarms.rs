// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! An alarm a device sends while the client is busy with something else:
//! acknowledged, kept, and handed over when asked.

use super::client;
use regalium_bacnet_core::primitive::ObjectIdentifier;
use regalium_bacnet_core::service::TimeStamp;
use regalium_bacnet_runtime::RequestError;
use std::time::Duration;

#[test]
fn an_alarm_arriving_mid_request_is_acknowledged_and_kept() {
    let mut c = client();
    c.who_is(None, Duration::from_millis(50)).unwrap();
    let ai7 = ObjectIdentifier {
        object_type: 0,
        instance: 7,
    };
    // The read itself fails — the device has no analog-input 7 to read —
    // but the alarm it sends alongside is not lost.
    let e = c.read_property(599, ai7, 85).unwrap_err();
    assert!(
        matches!(
            e,
            regalium_bacnet_runtime::Error::Request(RequestError::Error(_))
        ),
        "{e}"
    );
    let alarms: Vec<_> = c.alarms().collect();
    assert_eq!(alarms.len(), 1);
    let a = &alarms[0];
    assert_eq!(
        (a.device, a.object, a.to_state, a.ack_required),
        (599, ai7, 3, true)
    );
    assert_eq!(
        (a.time, a.message.as_deref()),
        (TimeStamp::Sequence(42), Some("High load"))
    );
    let sent = c.link().sent.lock().unwrap().clone();
    assert!(
        sent.iter().any(|(_, n)| n.ends_with(&[0x20, 88, 0x02])),
        "Simple-ACK for invoke 88"
    );
    assert_eq!(c.alarms().count(), 0, "taken once");
}

#[test]
fn an_alarm_arriving_while_idle_waits_for_the_next_listen() {
    let mut c = client();
    c.who_is(None, Duration::from_millis(50)).unwrap();
    // Nothing is listening: the alarm stays on the network until the
    // client next reads it, which is what `listen` is for.
    c.link().push(1, &super::devices::alarm_npdu());
    assert_eq!(c.alarms().count(), 0);
    c.listen(Duration::from_millis(20)).unwrap();
    assert_eq!(c.alarms().count(), 1);
}
