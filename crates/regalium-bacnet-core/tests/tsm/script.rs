// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! One scenario line, run: parse the event, hand it to the machine at its
//! time, drain every step that follows, and spell them the way the corpus
//! does.

use regalium_bacnet_core::apdu::{Apdu, Segments};
use regalium_bacnet_core::tsm::{Abandoned, Config, Received, Remote, Reply, Request, Step, Tsm};
use std::fmt::Write;
use std::time::Duration;

pub(crate) fn hex(b: &[u8]) -> String {
    b.iter().fold(String::new(), |mut out, x| {
        let _ = write!(out, "{x:02x}");
        out
    })
}

fn octets(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(s.get(i * 2..i * 2 + 2).expect("hex"), 16).expect("hex"))
        .collect()
}

/// `key=value` options after an event's positional arguments.
fn option<'a>(words: &[&'a str], key: &str) -> Option<&'a str> {
    words
        .iter()
        .find_map(|w| w.strip_prefix(key)?.strip_prefix('='))
}

fn ms(s: &str) -> Duration {
    Duration::from_millis(s.parse().expect("milliseconds"))
}

pub(crate) fn config(words: &[&str]) -> Config {
    let mut c = Config::default();
    if let Some(t) = option(words, "apdu-timeout") {
        c = c.apdu_timeout(ms(t));
    }
    if let Some(t) = option(words, "segment-timeout") {
        c = c.segment_timeout(ms(t));
    }
    if let Some(n) = option(words, "retries") {
        c = c.retries(n.parse().expect("retries"));
    }
    if let Some(n) = option(words, "window") {
        c = c.window(n.parse().expect("window"));
    }
    if let Some(n) = option(words, "max-reassembly") {
        c = c.max_reassembly(n.parse().expect("octets"));
    }
    if words.contains(&"no-segmented-response") {
        c = c.segmented_response(false);
    }
    c
}

/// `request PEER SERVICE BODY [max-apdu=N] [segmented] [max-segments=N]`.
pub(crate) fn request(tsm: &mut Tsm<u32>, words: &[&str], now: Duration) -> String {
    let [peer, service, body, ..] = words else {
        panic!("request PEER SERVICE BODY")
    };
    let remote = Remote {
        max_apdu: option(words, "max-apdu").map_or(1476, |n| n.parse().expect("octets")),
        segmented_requests: words.contains(&"segmented"),
        max_segments: option(words, "max-segments").map_or(Segments::Unspecified, |n| {
            Segments::AtMost(n.parse().expect("count"))
        }),
    };
    let body = if *body == "-" {
        Vec::new()
    } else {
        octets(body)
    };
    let r = Request::new(service.parse().expect("service"), body).remote(remote);
    match tsm.request(peer.parse().expect("peer"), r, now) {
        Ok(invoke) => format!("invoke {invoke}"),
        Err(e) => format!("refused {e:?}"),
    }
}

/// `rx PEER APDU`.
pub(crate) fn receive(tsm: &mut Tsm<u32>, words: &[&str], now: Duration) -> String {
    let [peer, apdu, ..] = words else {
        panic!("rx PEER APDU")
    };
    let octets = octets(apdu);
    let apdu = Apdu::decode(&octets).expect("a scenario's APDU decodes");
    match tsm.receive(peer.parse().expect("peer"), &apdu, now) {
        Received::Matched => "matched",
        Received::Unknown => "unknown",
        Received::NotForClient => "not-for-client",
    }
    .into()
}

/// Every step until the machine is idle, joined.
pub(crate) fn drain(tsm: &mut Tsm<u32>, now: Duration) -> Vec<String> {
    let mut out = Vec::new();
    for _ in 0..1000 {
        let step = tsm.poll(now);
        let idle = matches!(step, Step::Idle { .. });
        out.push(match step {
            Step::Transmit {
                peer,
                invoke,
                attempt,
                apdu,
            } => {
                format!("tx {peer} #{invoke} a{attempt} {}", hex(&apdu))
            }
            Step::Complete {
                peer,
                invoke,
                reply,
            } => format!(
                "complete {peer} #{invoke} {}",
                match reply {
                    Reply::Simple => "simple".into(),
                    Reply::Complex(b) => format!("complex {}", hex(&b)),
                    Reply::Error(b) => format!("error {}", hex(&b)),
                    Reply::Reject(r) => format!("reject {r}"),
                    Reply::Abort(r) => format!("abort {r}"),
                }
            ),
            Step::Abandon {
                peer,
                invoke,
                reason,
            } => format!(
                "abandon {peer} #{invoke} {}",
                match reason {
                    Abandoned::Timeout { attempts } => format!("timeout after {attempts}"),
                    Abandoned::SegmentTimeout => "segment-timeout".into(),
                    Abandoned::Aborted(r) => format!("aborted {r}"),
                }
            ),
            Step::Idle { wake_at: Some(t) } => format!("idle until {}", t.as_millis()),
            Step::Idle { wake_at: None } => "idle".into(),
        });
        if idle {
            return out;
        }
    }
    panic!("the machine never went idle")
}
