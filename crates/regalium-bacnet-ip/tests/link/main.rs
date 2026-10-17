// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

//! [`BacnetIp`] on loopback, against fake peers that answer as a device and
//! a BBMD would: what goes on the wire, what comes back up, and a
//! `ReadProperty` driven through the transaction state machine end to end.

mod device;
mod foreign;

use regalium_bacnet_core::datalink::{Datalink, Recipient};
use regalium_bacnet_ip::BacnetIp;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

/// A peer on its own thread: hands every datagram to `answer` with the
/// socket to reply on, and reports what it received. Stops after three
/// seconds of silence.
pub(crate) fn peer(
    answer: impl Fn(&[u8], SocketAddrV4, &UdpSocket) + Send + 'static,
) -> (SocketAddrV4, Receiver<Vec<u8>>) {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let std::net::SocketAddr::V4(addr) = socket.local_addr().unwrap() else {
        panic!()
    };
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = [0u8; 1600];
        while let Ok((n, std::net::SocketAddr::V4(from))) = socket.recv_from(&mut buf) {
            answer(&buf[..n], from, &socket);
            let _ = tx.send(buf[..n].to_vec());
        }
    });
    (addr, rx)
}

pub(crate) fn link() -> BacnetIp {
    BacnetIp::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)).unwrap()
}

pub(crate) fn octets(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect()
}

#[test]
fn a_receive_with_nothing_arriving_returns_none_after_its_timeout() {
    let l = link();
    let start = Instant::now();
    assert!(
        l.receive(&mut [0; 1500], Duration::from_millis(50))
            .unwrap()
            .is_none()
    );
    assert!(start.elapsed() >= Duration::from_millis(50));
}

#[test]
fn a_plain_broadcast_goes_to_the_broadcast_address_as_original_broadcast() {
    let (to, seen) = peer(|_, _, _| {});
    let l = link().broadcast_address(to);
    l.send(
        Recipient::LocalBroadcast,
        &octets("01 20 ff ff 00 ff 10 08"),
    )
    .unwrap();
    assert_eq!(
        seen.recv_timeout(Duration::from_secs(2)).unwrap(),
        octets("81 0b 00 0c 01 20 ff ff 00 ff 10 08")
    );
}

#[test]
fn a_buffer_too_small_for_the_npdu_is_the_callers_error() {
    let (dev, _) = peer(|_, from, s| {
        s.send_to(&octets("81 0a 00 08 01 00 10 08"), from).unwrap();
    });
    let l = link();
    l.send(Recipient::Unicast(dev), &octets("01 00 10 08"))
        .unwrap();
    let e = l.receive(&mut [0; 2], Duration::from_secs(2)).unwrap_err();
    assert_eq!(e.kind(), std::io::ErrorKind::InvalidInput);
}

#[derive(Default)]
struct Recorder(std::sync::Mutex<Vec<String>>);

impl regalium_bacnet_telemetry::Tracer for Recorder {
    fn start(
        &self,
        name: &'static str,
        _: regalium_bacnet_telemetry::SpanId,
    ) -> regalium_bacnet_telemetry::SpanId {
        self.0.lock().unwrap().push(format!("start {name}"));
        regalium_bacnet_telemetry::SpanId(1)
    }
    fn attribute(
        &self,
        _: regalium_bacnet_telemetry::SpanId,
        a: regalium_bacnet_telemetry::Attribute<'_>,
    ) {
        let v = match a.value {
            regalium_bacnet_telemetry::AttributeValue::Int(n) => n.to_string(),
            regalium_bacnet_telemetry::AttributeValue::Bool(b) => b.to_string(),
            regalium_bacnet_telemetry::AttributeValue::Str(s) => s.to_owned(),
        };
        self.0.lock().unwrap().push(format!("{}={v}", a.key.name()));
    }
    fn end(&self, _: regalium_bacnet_telemetry::SpanId, error: Option<&dyn std::fmt::Display>) {
        self.0
            .lock()
            .unwrap()
            .push(error.map_or_else(|| "end".into(), |e| format!("fail {e}")));
    }
}

#[test]
fn a_send_is_one_span_naming_the_bvlc_function_and_size() {
    let (dev, _) = peer(|_, _, _| {});
    let r = std::sync::Arc::new(Recorder::default());
    let l = link().with_tracer(r.clone());
    l.send(Recipient::Unicast(dev), &octets("01 00 10 08"))
        .unwrap();
    assert_eq!(
        *r.0.lock().unwrap(),
        [
            "start bacnet.ip.send",
            "bacnet.ip.function=original-unicast-npdu",
            "bacnet.pdu.octets=8",
            "end"
        ]
    );
}
