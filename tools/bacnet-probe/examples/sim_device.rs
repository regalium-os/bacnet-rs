// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Runs the simulated lift gateway on 127.0.0.1, so the probe has something
//! to talk to without hardware:
//!
//! ```sh
//! cargo run -p regalium-bacnet-probe --example sim_device   # serves on 127.0.0.1:47809
//! bacnet-probe --target 127.0.0.1 --port 47809 --bind 127.0.0.1:0
//! ```

use bacnet_probe::sim::Sim;
use std::net::UdpSocket;

fn main() -> std::io::Result<()> {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:47809".into());
    let socket = UdpSocket::bind(&addr)?;
    println!("simulated lift gateway (device 599) on {addr}; Ctrl-C to stop");
    Sim::lift_gateway().live().serve(&socket)
}
