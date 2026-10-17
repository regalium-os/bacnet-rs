// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! How the NPDU corpora spell the network layer: addresses, hop count,
//! control bits and network messages, then the APDU inside.

use regalium_bacnet_core::apdu::Apdu;
use regalium_bacnet_core::npdu::{Address, Body, Message, NetworkMessage, Npdu, Priority};

fn address(a: &Address<'_>) -> String {
    match (a.is_global(), a.is_broadcast()) {
        (true, true) => "global".into(),
        (_, true) => format!("{}:*", a.network),
        _ => format!("{}:{}", a.network, super::render::hex(a.mac)),
    }
}

pub(crate) fn npdu(n: &Npdu<'_>) -> String {
    let mut parts = Vec::new();
    if let Some(d) = &n.destination {
        parts.push(format!("to {}", address(&d.address)));
    }
    if let Some(s) = &n.source {
        parts.push(format!("from {}", address(s)));
    }
    if let Some(d) = &n.destination {
        parts.push(format!("hops {}", d.hop_count));
    }
    if n.expects_reply() {
        parts.push("expects-reply".into());
    }
    match n.priority() {
        Priority::Normal => {}
        Priority::Urgent => parts.push("priority urgent".into()),
        Priority::CriticalEquipment => parts.push("priority critical-equipment".into()),
        Priority::LifeSafety => parts.push("priority life-safety".into()),
    }
    if n.reserved_bits() != 0 {
        parts.push(format!("reserved-bits {:#04x}", n.reserved_bits()));
    }
    if parts.is_empty() {
        parts.push("local".into());
    }
    let body = match n.body {
        Body::Apdu(octets) => match Apdu::decode(octets) {
            Ok(a) => super::render::apdu(&a),
            Err(e) => super::render::error(&e),
        },
        Body::Message(m) => message(&m),
    };
    format!("{}; {body}", parts.join(", "))
}

fn networks(list: impl Iterator<Item = u16>) -> String {
    let all: Vec<_> = list.map(|n| n.to_string()).collect();
    if all.is_empty() {
        "all".into()
    } else {
        all.join(",")
    }
}

fn message(m: &Message<'_>) -> String {
    let head = match m.vendor {
        Some(v) => format!("message {} vendor {v}", m.name()),
        None => format!("message {}", m.name()),
    };
    let tail = match m.decode() {
        Err(_) => "invalid".into(),
        Ok(NetworkMessage::WhoIsRouter(None)) => "any".into(),
        Ok(NetworkMessage::WhoIsRouter(Some(n))) => format!("network {n}"),
        Ok(
            NetworkMessage::IAmRouter(l)
            | NetworkMessage::RouterBusy(l)
            | NetworkMessage::RouterAvailable(l),
        ) => networks(l.iter()),
        Ok(NetworkMessage::ICouldBeRouter {
            network,
            performance,
        }) => {
            format!("network {network} performance {performance}")
        }
        Ok(NetworkMessage::Reject { reason, network }) => {
            format!("reason {reason} network {network}")
        }
        Ok(NetworkMessage::WhatIsNetworkNumber) => String::new(),
        Ok(NetworkMessage::NetworkNumberIs { network, flag }) => {
            format!("network {network} flag {flag}")
        }
        Ok(_) if m.data.is_empty() => String::new(),
        Ok(_) => format!("data {}", super::render::hex(m.data)),
    };
    if tail.is_empty() {
        head
    } else {
        format!("{head} {tail}")
    }
}
