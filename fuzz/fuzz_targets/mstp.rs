// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! MS/TP from a noisy line: any octets through the reader into a master,
//! which never panics; and any frame that decodes re-encodes to one that
//! decodes the same.

#![no_main]

use core::time::Duration;
use libfuzzer_sys::fuzz_target;
use regalium_bacnet::mstp::frame::Frame;
use regalium_bacnet::mstp::master::{Config, Master};
use regalium_bacnet::mstp::reader::Reader;

fuzz_target!(|data: &[u8]| {
    if let Ok((frame, used)) = Frame::decode(data) {
        assert!(used <= data.len());
        assert_eq!(Frame::decode(&frame.encode()).map(|(f, _)| f), Ok(frame));
    }
    let mut reader = Reader::new();
    let mut master = Master::new(Config::new(3).max_master(9), Duration::ZERO);
    for (i, chunk) in data.chunks(7).enumerate() {
        let now = Duration::from_millis(u64::try_from(i).unwrap_or(0) * 40);
        for f in reader.push(chunk) {
            let _ = master.received(&f, now);
        }
        let _ = master.poll(now);
    }
});
