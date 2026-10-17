// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! The phase gate: every record of every corpus under `fixtures/` decodes to
//! what the record says, and re-encodes to exactly its octets.
//!
//! A record is `octets => meaning`. The meaning is rendered from what the
//! codec decoded, so a corpus states the codec's behaviour in a form a
//! person can check against the standard, octet by octet.
//!
//! The corpora are compiled in rather than read: this is the pure core, and
//! its purity gate refuses file access even in a test. `just fixtures`
//! fails when a `.hex` file on disk is missing from [`CORPORA`], so a new
//! corpus cannot sit beside the walker unread.

use regalium_bacnet_core::primitive::Primitive;
use regalium_bacnet_core::tag::{self, Class, Element, ErrorKind, Lvt};

/// Every corpus, by file name. `just fixtures` checks this list is complete.
const CORPORA: &[(&str, &str)] = &[
    (
        "application.hex",
        include_str!("../fixtures/tag/application.hex"),
    ),
    ("captured.hex", include_str!("../fixtures/tag/captured.hex")),
    ("context.hex", include_str!("../fixtures/tag/context.hex")),
    (
        "malformed.hex",
        include_str!("../fixtures/tag/malformed.hex"),
    ),
    ("quirks.hex", include_str!("../fixtures/tag/quirks.hex")),
];

fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).expect("hex octet"))
        .collect()
}

fn octets(b: &[u8]) -> String {
    use std::fmt::Write;
    b.iter().fold(String::new(), |mut out, x| {
        let _ = write!(out, "{x:02x}");
        out
    })
}

fn value(p: &Primitive<'_>) -> String {
    match p {
        Primitive::Null => "null".into(),
        Primitive::Boolean(b) => format!("boolean {b}"),
        Primitive::Unsigned(v) => format!("unsigned {v}"),
        Primitive::Signed(v) => format!("signed {v}"),
        Primitive::Real(v) => format!("real {v:?}"),
        Primitive::Double(v) => format!("double {v:?}"),
        Primitive::OctetString([]) => "octets (empty)".into(),
        Primitive::OctetString(o) => format!("octets {}", octets(o)),
        Primitive::CharacterString(s) if s.charset == 0 => {
            format!("string {:?}", s.text().unwrap())
        }
        Primitive::CharacterString(s) => format!(
            "string charset {} {:?}",
            s.charset,
            s.text().unwrap_or_default()
        ),
        Primitive::BitString(b) if b.is_empty() => "bits (none)".into(),
        Primitive::BitString(b) => format!(
            "bits {}",
            (0..b.len())
                .map(|i| if b.get(i).unwrap() { '1' } else { '0' })
                .collect::<String>()
        ),
        Primitive::Enumerated(v) => format!("enumerated {v}"),
        Primitive::Date(d) => {
            let f = |v: u8, w: usize, base: u16| {
                if v == 255 {
                    "*".into()
                } else {
                    format!("{:0w$}", u16::from(v) + base)
                }
            };
            format!(
                "date {}-{}-{} weekday {}",
                f(d.year, 4, 1900),
                f(d.month, 2, 0),
                f(d.day, 2, 0),
                d.weekday
            )
        }
        Primitive::Time(t) => format!("time {}:{}:{}.{}", t.hour, t.minute, t.second, t.hundredths),
        Primitive::ObjectIdentifier(id) => format!("object {}:{}", id.object_type, id.instance),
    }
}

const NAMES: [&str; 13] = [
    "null",
    "boolean",
    "unsigned",
    "signed",
    "real",
    "double",
    "octets",
    "string",
    "bits",
    "enumerated",
    "date",
    "time",
    "object",
];

/// What one element means, as the corpus spells it.
fn meaning(e: &Element<'_>) -> String {
    let n = e.header.number;
    let text = match (e.header.class, e.header.lvt) {
        (_, Lvt::Opening) => return format!("[{n}]{{"),
        (_, Lvt::Closing) => return format!("}}[{n}]"),
        (Class::Context, _) => format!("[{n}] {}", octets(e.content)),
        (Class::Application, _) => match Primitive::decode(e) {
            Ok(p) => {
                let mut canonical = Vec::new();
                p.encode(&mut canonical);
                let mut original = Vec::new();
                e.encode(&mut original);
                let long = e.header.is_canonical() && canonical != original;
                format!("{}{}", value(&p), if long { " (long)" } else { "" })
            }
            Err(_) => {
                return format!(
                    "{} invalid",
                    NAMES.get(usize::from(n)).unwrap_or(&"application")
                );
            }
        },
    };
    if e.header.is_canonical() {
        text
    } else {
        format!("{text} (non-canonical)")
    }
}

fn decode(octets: &[u8]) -> String {
    match tag::decode_all(octets) {
        Ok(elements) => {
            assert_eq!(
                tag::encode_all(&elements),
                octets,
                "re-encoding is not lossless"
            );
            elements.iter().map(meaning).collect::<Vec<_>>().join(", ")
        }
        Err(e) => match e.kind {
            ErrorKind::Truncated { needed, available } => {
                format!("error truncated {needed}/{available}")
            }
            ErrorKind::ReservedLvt(v) => format!("error reserved-lvt {v}"),
            ErrorKind::Unbalanced => "error unbalanced".into(),
            other => format!("error {other:?}"),
        },
    }
}

#[test]
fn every_fixture_record_decodes_to_its_meaning_and_round_trips() {
    assert!(CORPORA.len() >= 5, "a walker that finds nothing must fail");
    let mut failures = Vec::new();
    let mut total = 0;
    for (file, text) in CORPORA {
        let mut records = 0;
        for (n, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (input, want) = line
                .split_once(" => ")
                .unwrap_or_else(|| panic!("{}:{}: no ' => '", file, n + 1));
            let got = decode(&hex(input));
            if got != want.trim() {
                failures.push(format!(
                    "{}:{}\n  octets {input}\n  want   {want}\n  got    {got}",
                    file,
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
