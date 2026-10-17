// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The first live read: find a device over BACnet/IP and read one property,
//! entirely through the driver — the codecs, the transaction state machine,
//! the datalink and the client.
//!
//! ```sh
//! cargo run -p regalium-bacnet --example read -- ADDRESS DEVICE OBJECT PROPERTY
//! cargo run -p regalium-bacnet --example read -- 127.0.0.1:47809 599 analog-input-1 present-value
//! ```
//!
//! `ADDRESS` is where the Who-Is goes: the device itself, a router, or a
//! subnet's broadcast address. `OBJECT` and `PROPERTY` are spelled as the
//! protobuf resource names spell them.

use regalium_bacnet::Client;
use regalium_bacnet::enumerated::parse_property;
use regalium_bacnet::ip::BacnetIp;
use regalium_bacnet::runtime::Config;
use regalium_bacnet::schema::convert::parse_object_segment;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::time::Duration;

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run() -> Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [address, device, object, property] = args.as_slice() else {
        return Err("usage: read ADDRESS DEVICE OBJECT PROPERTY".into());
    };
    let address: SocketAddrV4 = address.parse()?;
    let device: u32 = device.parse()?;
    let object = parse_object_segment(object).ok_or("OBJECT is a name like analog-input-1")?;
    let property = parse_property(property).ok_or("PROPERTY is a name like present-value")?;

    let link = BacnetIp::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0))?;
    let mut client = Client::new(link, Config::default());
    let found = client.who_is_at(address, Some((device, device)), Duration::from_secs(2))?;
    let Some(d) = found.first() else {
        return Err(format!("device {device} did not answer a Who-Is sent to {address}").into());
    };
    println!(
        "found device {} at {}{}: max APDU {}, vendor {}",
        d.instance,
        d.address,
        d.route
            .as_ref()
            .map(|r| format!(" (network {})", r.network))
            .unwrap_or_default(),
        d.max_apdu,
        d.vendor_id
    );

    let reading = client.read_property(device, object, property)?;
    let model = reading.property()?;
    println!("{}", model.name);
    match reading.primitive() {
        Some(p) => println!("  value   {p:?}"),
        None => println!("  value   constructed, {} octets", reading.value.len()),
    }
    if let Some(v) = &model.value {
        println!("  model   {:?}", v.kind);
        println!("  encoded {:02x?}", v.encoded);
    }
    for event in client.events() {
        println!("  event   {event:?}");
    }
    Ok(())
}
