// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The envelope: BVLL header (Annex J.2) and NPDU (Clause 6).

use super::Cursor;

pub(super) fn bvll(c: &mut Cursor<'_>) -> Option<()> {
    let kind = c.byte(|b| match b {
        0x81 => "BVLC type 0x81: BACnet/IP (Annex J)".into(),
        b => format!("BVLC type 0x{b:02x}: not BACnet/IP"),
    })?;
    (kind == 0x81).then_some(())?;
    let function = c.byte(|f| format!("BVLC function 0x{f:02x}: {}", bvlc_function(f)))?;
    c.u16(|n| format!("BVLC length {n}: the whole datagram, header included"))?;
    match function {
        0x09..=0x0B => npdu(c),
        0x04 => {
            let o = c.peek(6)?;
            let port = u16::from_be_bytes([*o.get(4)?, *o.get(5)?]);
            let text = format!(
                "original source {}.{}.{}.{}:{port}: a BBMD relayed this; reply to the original",
                o.first()?,
                o.get(1)?,
                o.get(2)?,
                o.get(3)?
            );
            c.take(6, text)?;
            npdu(c)
        }
        0x00 => c
            .u16(|r| {
                format!(
                    "result code 0x{r:04x}: {}",
                    if r == 0 { "successful" } else { "NAK" }
                )
            })
            .map(drop),
        0x05 => c
            .u16(|t| format!("time-to-live {t} s: re-register before it lapses"))
            .map(drop),
        _ => None,
    }
}

fn npdu(c: &mut Cursor<'_>) -> Option<()> {
    c.byte(|v| format!("NPDU version {v} (always 1)"))?;
    let control = c.byte(|b| format!("NPDU control 0b{b:08b}: {}", control_bits(b)))?;
    if control & 0x20 != 0 {
        address(c, "DNET", "DLEN", "DADR", "destination network")?;
    }
    if control & 0x08 != 0 {
        address(
            c,
            "SNET",
            "SLEN",
            "SADR",
            "source network: the sender is behind a router",
        )?;
    }
    if control & 0x20 != 0 {
        c.byte(|h| {
            format!("hop count {h}: each router decrements it; 0 drops the frame (loop guard)")
        })?;
    }
    if control & 0x80 != 0 {
        c.byte(|t| format!("network message 0x{t:02x}: {}", network_message(t)))?;
        return Some(());
    }
    super::apdu::apdu(c)
}

fn address(c: &mut Cursor<'_>, net: &str, len: &str, mac: &str, what: &str) -> Option<()> {
    c.u16(|n| match n {
        0xFFFF => format!("{net} 65535: global broadcast, every network ({what})"),
        n => format!("{net} {n}: {what}"),
    })?;
    let n = c.byte(|l| match l {
        0 => format!("{len} 0: broadcast on that network"),
        l => format!("{len} {l}: MAC address length"),
    })?;
    if n > 0 {
        c.take(
            n.into(),
            format!("{mac}: the station's MAC on that network"),
        )?;
    }
    Some(())
}

fn control_bits(b: u8) -> String {
    const PRIORITY: [&str; 4] = [
        "priority normal",
        "priority urgent",
        "priority critical",
        "priority life-safety",
    ];
    let mut parts = vec![if b & 0x80 != 0 {
        "network-layer message"
    } else {
        "carries an APDU"
    }];
    if b & 0x20 != 0 {
        parts.push("DNET present");
    }
    if b & 0x08 != 0 {
        parts.push("SNET present");
    }
    if b & 0x04 != 0 {
        parts.push("expecting reply");
    }
    parts.extend(PRIORITY.get(usize::from(b & 3)));
    parts.join(", ")
}

fn bvlc_function(f: u8) -> &'static str {
    match f {
        0x00 => "BVLC-Result",
        0x01 => "Write-Broadcast-Distribution-Table",
        0x02 => "Read-Broadcast-Distribution-Table",
        0x03 => "Read-Broadcast-Distribution-Table-Ack",
        0x04 => "Forwarded-NPDU (relayed by a BBMD)",
        0x05 => "Register-Foreign-Device",
        0x06 => "Read-Foreign-Device-Table",
        0x07 => "Read-Foreign-Device-Table-Ack",
        0x08 => "Delete-Foreign-Device-Table-Entry",
        0x09 => "Distribute-Broadcast-To-Network",
        0x0A => "Original-Unicast-NPDU",
        0x0B => "Original-Broadcast-NPDU",
        0x0C => "Secure-BVLL",
        _ => "unknown",
    }
}

fn network_message(t: u8) -> &'static str {
    match t {
        0x00 => "Who-Is-Router-To-Network",
        0x01 => "I-Am-Router-To-Network",
        0x02 => "I-Could-Be-Router-To-Network",
        0x03 => "Reject-Message-To-Network",
        0x04 => "Router-Busy-To-Network",
        0x05 => "Router-Available-To-Network",
        0x06 => "Initialize-Routing-Table",
        0x07 => "Initialize-Routing-Table-Ack",
        0x12 => "What-Is-Network-Number",
        0x13 => "Network-Number-Is",
        _ => "unknown / proprietary",
    }
}
