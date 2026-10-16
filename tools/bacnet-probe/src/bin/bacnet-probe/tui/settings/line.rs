// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The command line that starts the probe with the settings on screen, so a
//! configuration found by trial in the TUI can be pasted into a script or a
//! service unit rather than rediscovered next visit.

use super::fields::{FIELDS, Field};
use crate::args::{self, Net};
use std::time::Duration;

/// `bacnet-probe tui --interval 1 --target 10.0.0.255 --bbmd 10.1.0.1:47808`,
/// naming only what differs from the defaults. Network options only:
/// `--otlp` and `--label` are not edited here, and are not repeated.
pub(crate) fn command_line(net: &Net, interval: Duration) -> String {
    let defaults = args::defaults();
    let mut words = vec!["bacnet-probe".to_owned()];
    let default_interval = Duration::from_secs(2);
    if interval != default_interval {
        words.push("tui".into());
        words.push("--interval".into());
        words.push(Field::Interval.show(net, interval));
    }
    for field in FIELDS.into_iter().filter(|f| *f != Field::Interval) {
        let value = field.show(net, interval);
        if value == field.show(&defaults, default_interval) {
            continue;
        }
        let (_, flag) = field.label();
        match field {
            Field::Unicast => words.push(flag.into()),
            Field::Scan => {
                for scope in &net.scan {
                    words.push(flag.into());
                    words.push(scope.to_string());
                }
            }
            _ => {
                words.push(flag.into());
                words.push(value);
            }
        }
    }
    words.join(" ")
}
