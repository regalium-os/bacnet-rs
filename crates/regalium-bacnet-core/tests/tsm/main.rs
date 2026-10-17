// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Phase 5's gate: every scenario under `fixtures/tsm/` replays to exactly
//! the steps it states, on a fake clock.
//!
//! A record is `TIME EVENT => WHAT FOLLOWS`. The time is milliseconds on the
//! fake clock; the event is `config …` (a fresh machine), `request …`, `rx …`
//! or `poll`; what follows is the event's own result, then every step the
//! machine returns until it is idle, separated by ` | `. Transmitted APDUs
//! are spelled in full, so a scenario states the octets on the wire.

mod script;

use regalium_bacnet_core::tsm::{Config, Tsm};
use std::time::Duration;

/// Every scenario file. `just fixtures` fails when one on disk is missing.
const CORPORA: &[(&str, &str)] = &[
    (
        "answers.kat",
        include_str!("../../fixtures/tsm/answers.kat"),
    ),
    (
        "retries.kat",
        include_str!("../../fixtures/tsm/retries.kat"),
    ),
    (
        "segmented-request.kat",
        include_str!("../../fixtures/tsm/segmented-request.kat"),
    ),
    (
        "segmented-answer.kat",
        include_str!("../../fixtures/tsm/segmented-answer.kat"),
    ),
    ("aborts.kat", include_str!("../../fixtures/tsm/aborts.kat")),
];

fn run(tsm: &mut Tsm<u32>, line: &str) -> String {
    let (time, event) = line.split_once(' ').expect("TIME EVENT");
    let now = Duration::from_millis(time.parse().expect("milliseconds"));
    let words: Vec<&str> = event.split_whitespace().collect();
    let (verb, args) = words.split_first().expect("an event");
    let head = match *verb {
        "config" => {
            *tsm = Tsm::new(script::config(args));
            return "ok".into();
        }
        "request" => Some(script::request(tsm, args, now)),
        "rx" => Some(script::receive(tsm, args, now)),
        "poll" => None,
        other => panic!("unknown event {other}"),
    };
    head.into_iter()
        .chain(script::drain(tsm, now))
        .collect::<Vec<_>>()
        .join(" | ")
}

#[test]
fn every_tsm_fixture_scenario_replays_to_the_steps_it_states() {
    assert!(CORPORA.len() >= 5, "a walker that finds nothing must fail");
    let mut failures = Vec::new();
    let mut total = 0;
    for (file, text) in CORPORA {
        let mut tsm = Tsm::new(Config::default());
        let mut records = 0;
        for (n, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (event, want) = line
                .split_once(" => ")
                .unwrap_or_else(|| panic!("{file}:{}: no ' => '", n + 1));
            let got = run(&mut tsm, event);
            if got != want.trim() {
                failures.push(format!(
                    "{file}:{}\n  event {event}\n  want  {want}\n  got   {got}",
                    n + 1
                ));
            }
            records += 1;
        }
        assert!(records > 0, "{file} has no records");
        total += records;
    }
    assert!(
        failures.is_empty(),
        "{} of {total} records failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
