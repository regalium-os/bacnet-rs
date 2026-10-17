// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! How the NPDU and APDU corpora spell what was decoded: one line a person
//! can check against Clause 6 and §20.1, octet by octet.

use regalium_bacnet_core::apdu::{Apdu, Pdu, Segment, Segments};
use regalium_bacnet_core::{Error, ErrorKind};

pub(crate) fn hex(b: &[u8]) -> String {
    use std::fmt::Write;
    b.iter().fold(String::new(), |mut out, x| {
        let _ = write!(out, "{x:02x}");
        out
    })
}

pub(crate) fn error(e: &Error<'_>) -> String {
    match e.kind {
        ErrorKind::Truncated { needed, available } => {
            format!("error truncated {needed}/{available}")
        }
        ErrorKind::Version(v) => format!("error version {v}"),
        ErrorKind::PduType(t) => format!("error pdu-type {t}"),
        other => format!("error {other:?}"),
    }
}

fn segment(out: &mut Vec<String>, s: Option<Segment>) {
    if let Some(s) = s {
        out.push(format!("segment {}/{}", s.sequence, s.window));
    }
}

pub(crate) fn apdu(apdu: &Apdu<'_>) -> String {
    let mut parts = vec![apdu.type_name().to_owned()];
    let mut body = None;
    let reserved = match apdu.pdu {
        Pdu::ConfirmedRequest {
            limits,
            invoke_id,
            segment: seg,
            service,
            body: octets,
        } => {
            parts.push(format!("invoke {invoke_id} service {service}"));
            parts.push(match limits.max_apdu() {
                Some(n) => format!("max-apdu {n}"),
                None => format!("max-apdu reserved {}", limits.apdu_code()),
            });
            parts.push(match limits.max_segments() {
                Segments::Unspecified => "segments unspecified".into(),
                Segments::AtMost(n) => format!("segments {n}"),
                Segments::MoreThan64 => "segments >64".into(),
            });
            if limits.reserved_bit() {
                parts.push("reserved-limit-bit".into());
            }
            if apdu.accepts_segmented_response() {
                parts.push("accepts-segmented".into());
            }
            segment(&mut parts, seg);
            body = Some(octets);
            0x01
        }
        Pdu::UnconfirmedRequest {
            service,
            body: octets,
        } => {
            parts.push(format!("service {service}"));
            body = Some(octets);
            0x0F
        }
        Pdu::SimpleAck { invoke_id, service } => {
            parts.push(format!("invoke {invoke_id} service {service}"));
            0x0F
        }
        Pdu::ComplexAck {
            invoke_id,
            segment: seg,
            service,
            body: octets,
        } => {
            parts.push(format!("invoke {invoke_id} service {service}"));
            segment(&mut parts, seg);
            body = Some(octets);
            0x03
        }
        Pdu::SegmentAck {
            invoke_id,
            sequence,
            window,
        } => {
            parts.push(format!(
                "invoke {invoke_id} sequence {sequence} window {window}"
            ));
            0x0C
        }
        Pdu::Error {
            invoke_id,
            service,
            body: octets,
        } => {
            parts.push(format!("invoke {invoke_id} service {service}"));
            body = Some(octets);
            0x0F
        }
        Pdu::Reject { invoke_id, reason } | Pdu::Abort { invoke_id, reason } => {
            parts.push(format!("invoke {invoke_id} reason {reason}"));
            if apdu.pdu_type() == 6 { 0x0F } else { 0x0E }
        }
    };
    let flags = [
        (apdu.more_follows(), "more"),
        (apdu.is_negative(), "nak"),
        (apdu.from_server(), "server"),
    ];
    parts.extend(
        flags
            .iter()
            .filter(|(on, _)| *on)
            .map(|(_, f)| (*f).to_owned()),
    );
    if apdu.flags & reserved != 0 {
        parts.push(format!("reserved-flags {}", apdu.flags & reserved));
    }
    if let Some(octets) = body.filter(|o| !o.is_empty()) {
        parts.push(format!("body {}", hex(octets)));
    }
    if !apdu.trailing.is_empty() {
        parts.push(format!("trailing {}", hex(apdu.trailing)));
    }
    parts.join(" ")
}
