// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The APDU's fixed header (§20.1), then hands the service body to `params`.

use super::Cursor;
use super::schema::Service;
use crate::names;

pub(super) fn apdu(c: &mut Cursor<'_>) -> Option<()> {
    let first = c.byte(|b| format!("APDU type {}: {}", b >> 4, pdu_type(b)))?;
    let segmented = first & 0x08 != 0;
    let invoke = |c: &mut Cursor<'_>| {
        c.byte(|i| format!("invoke id {i}: pairs this reply with its request"))
    };
    let segment = |c: &mut Cursor<'_>| {
        c.byte(|s| format!("sequence number {s}"))?;
        c.byte(|w| format!("proposed window size {w}"))
    };
    let confirmed =
        |c: &mut Cursor<'_>| c.byte(|s| format!("service {s}: {}", names::confirmed_service(s)));
    let service = match first >> 4 {
        0 => {
            c.byte(|b| {
                format!(
                    "max segments {}, max APDU accepted {} octets",
                    max_segments(b >> 4),
                    max_apdu(b & 0x0F)
                )
            })?;
            invoke(c)?;
            if segmented {
                segment(c)?;
            }
            Service::confirmed(confirmed(c)?, false)
        }
        1 => Service::unconfirmed(
            c.byte(|s| format!("service {s}: {}", names::unconfirmed_service(s)))?,
        ),
        2 => {
            invoke(c)?;
            confirmed(c)?;
            return Some(());
        }
        3 => {
            invoke(c)?;
            if segmented {
                segment(c)?;
            }
            Service::confirmed(confirmed(c)?, true)
        }
        5 => {
            invoke(c)?;
            confirmed(c)?;
            Service::Error
        }
        6 => {
            invoke(c)?;
            c.byte(|r| format!("reject reason {r}: {}", names::reject_reason(r)))?;
            return Some(());
        }
        7 => {
            invoke(c)?;
            c.byte(|r| format!("abort reason {r}: {}", names::abort_reason(r)))?;
            return Some(());
        }
        _ => return None,
    };
    if segmented {
        return None; // A segment's body is only meaningful once reassembled.
    }
    super::params::walk(c, service)
}

fn pdu_type(b: u8) -> String {
    const TYPES: [&str; 8] = [
        "Confirmed-Request",
        "Unconfirmed-Request",
        "Simple-ACK",
        "Complex-ACK",
        "Segment-ACK",
        "Error",
        "Reject",
        "Abort",
    ];
    let name = TYPES
        .get(usize::from(b >> 4))
        .copied()
        .unwrap_or("reserved");
    let mut flags = Vec::new();
    if matches!(b >> 4, 0 | 3) {
        if b & 0x08 != 0 {
            flags.push("segmented");
        }
        if b & 0x04 != 0 {
            flags.push("more follows");
        }
    }
    if b >> 4 == 0 && b & 0x02 != 0 {
        flags.push("segmented response accepted");
    }
    if b >> 4 == 7 {
        flags.push(if b & 1 != 0 {
            "sent by server"
        } else {
            "sent by client"
        });
    }
    if flags.is_empty() {
        name.into()
    } else {
        format!("{name} ({})", flags.join(", "))
    }
}

fn max_segments(code: u8) -> &'static str {
    ["unspecified", "2", "4", "8", "16", "32", "64", ">64"]
        .get(usize::from(code & 7))
        .unwrap_or(&"?")
}

fn max_apdu(code: u8) -> &'static str {
    ["50", "128", "206", "480", "1024", "1476"]
        .get(usize::from(code))
        .unwrap_or(&"reserved")
}
