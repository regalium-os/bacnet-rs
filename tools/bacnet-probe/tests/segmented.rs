// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

//! A segmented answer, reassembled. Before Phase 8 the probe aborted any
//! answer a device segmented; on the driver's transaction state machine it
//! acknowledges each segment and gets the whole value.

use bacnet_probe::{Client, Device, ObjectId, Value};
use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

/// A device that answers a read of its object list in two segments, and
/// reports every APDU it receives.
fn segmenting_device() -> (SocketAddr, std::sync::mpsc::Receiver<Vec<u8>>) {
    let s = UdpSocket::bind("127.0.0.1:0").unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let addr = s.local_addr().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = [0u8; 1600];
        let send = |to, apdu: &[u8]| {
            let mut d = vec![0x81, 0x0a, 0, 0, 0x01, 0x00];
            d.extend_from_slice(apdu);
            let len = u16::try_from(d.len()).unwrap().to_be_bytes();
            d[2..4].copy_from_slice(&len);
            s.send_to(&d, to).unwrap();
        };
        while let Ok((n, from)) = s.recv_from(&mut buf) {
            let apdu = buf[6..n].to_vec();
            tx.send(apdu.clone()).unwrap();
            match apdu[0] >> 4 {
                // The request: segment 0 of the ACK, more follows, window 2.
                0 => {
                    let invoke = apdu[2];
                    let mut seg = vec![0x3c, invoke, 0, 2, 0x0c];
                    seg.extend([
                        0x0c, 0x02, 0x00, 0x02, 0x57, 0x19, 0x4c, 0x3e, 0xc4, 0x02, 0x00, 0x02,
                        0x57,
                    ]);
                    send(from, &seg);
                }
                // Its Segment-ACK: the last segment.
                4 if apdu[2] == 0 => {
                    let invoke = apdu[1];
                    send(
                        from,
                        &[0x38, invoke, 1, 2, 0x0c, 0xc4, 0x00, 0x00, 0x00, 0x01, 0x3f],
                    );
                }
                _ => {}
            }
        }
    });
    (addr, rx)
}

#[test]
fn a_segmented_answer_is_acknowledged_and_reassembled_not_aborted() {
    let (addr, seen) = segmenting_device();
    let mut client = Client::bind("127.0.0.1:0".parse().unwrap())
        .unwrap()
        .with_timeout(Duration::from_millis(500));
    let device = Device {
        instance: 599,
        addr,
        route: None,
        max_apdu: 1476,
        vendor: 0,
    };
    let list = client
        .read_property(&device, ObjectId::new(8, 599), 76, None)
        .unwrap();
    assert_eq!(
        list,
        [
            Value::Object(ObjectId::new(8, 599)),
            Value::Object(ObjectId::new(0, 1))
        ]
    );
    // The client returns as it sends the last Segment-ACK; give the device
    // a moment to hear it.
    let apdus: Vec<_> = (0..3)
        .filter_map(|_| seen.recv_timeout(Duration::from_secs(2)).ok())
        .collect();
    // The request, then a Segment-ACK of each segment; never an Abort.
    assert_eq!(apdus.len(), 3, "{apdus:02x?}");
    assert_eq!(&apdus[1][..], &[0x40, apdus[1][1], 0, 2]);
    assert!(apdus.iter().all(|a| a[0] >> 4 != 7));
}
