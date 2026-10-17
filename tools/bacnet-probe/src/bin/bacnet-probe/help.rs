// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The examples `--help` ends with: one line per thing a person usually
//! wants, discovery first and the commands that change a device last.

pub(crate) const EXAMPLES: &str = "\
Examples:
  bacnet-probe                                   interactive TUI, broadcast discovery
  bacnet-probe --scan '192.168.*' whois          sweep every /24 in 192.168.0.0/16
  bacnet-probe --bbmd 10.1.0.1 whois             reach another subnet through its BBMD
  bacnet-probe read 599 analog-input:1           present-value, with units / state names
  bacnet-probe find 'Car 1 load'                 which device holds this object (Who-Has)
  bacnet-probe watch 599 --every 1               live values: COV where allowed, else polled
  bacnet-probe scan --detail --format json > scan.json
  bacnet-probe --otlp localhost:4317 export      every point, continuously, as metrics
  bacnet-probe dissect '81 0a 00 11 01 04 00 05 01 0c 0c 02 00 02 57 19 4d'

Resources, by name (writes change the device):
  bacnet-probe list devices                       discover, then each device
  bacnet-probe get devices/599/objects/analog-output-1
  bacnet-probe write devices/599/objects/analog-output-1 75 --priority 8
  bacnet-probe write devices/599/objects/analog-output-1 null --priority 8   relinquish
  bacnet-probe create devices/599 --type analog-value --display-name Setpoint --value 21.5
  bacnet-probe delete devices/599/objects/analog-value-1

Without hardware:
  cargo run -p regalium-bacnet-probe --example sim_device
  bacnet-probe --target 127.0.0.1 --port 47809 --bind 127.0.0.1:0";
