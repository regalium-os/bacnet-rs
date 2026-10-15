// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `bacnet-probe dissect`: break a hex-dumped datagram down offline, e.g. one
//! copied out of Wireshark or a vendor's log. No network involved.

use super::table;
use crate::Result;
use crate::args::Format;
use bacnet_probe::dissect;
use bacnet_probe::json::Json;
use clap::Args;
use std::io::Read;

#[derive(Args)]
pub(crate) struct DissectArgs {
    /// Hex octets: `810a0011…`, `81 0a 00 11`, `81:0a:…`, `0x81,0x0a`. Reads
    /// stdin when omitted.
    hex: Vec<String>,
    #[arg(long, value_enum, default_value_t)]
    format: Format,
}

pub(crate) fn run(args: &DissectArgs) -> Result {
    let text = if args.hex.is_empty() {
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s)?;
        s
    } else {
        args.hex.join(" ")
    };
    let bytes = parse_hex(&text)?;
    let fields = dissect::frame(&bytes);
    if args.format == Format::Json {
        let list = fields.iter().map(|f| {
            let octets: String = bacnet_probe::dissect::hex(&f.bytes);
            Json::obj([
                ("octets", Json::Str(octets)),
                ("depth", Json::UInt(f.depth as u64)),
                ("field", Json::str(f.text.clone())),
            ])
        });
        let doc = Json::obj([
            ("summary", Json::Str(dissect::summary(&bytes))),
            ("fields", Json::Arr(list.collect())),
        ]);
        println!("{}", doc.pretty());
    } else {
        let rows = fields
            .iter()
            .map(|f| {
                let hex: Vec<String> = f
                    .bytes
                    .iter()
                    .map(|b| bacnet_probe::dissect::hex(&[*b]))
                    .collect();
                vec![hex.join(" "), format!("{}{}", "· ".repeat(f.depth), f.text)]
            })
            .collect();
        println!("{} ({} octets)", dissect::summary(&bytes), bytes.len());
        print!("{}", table::render(&["OCTETS", "FIELD"], rows));
    }
    Ok(())
}

/// Accepts any common hex dump spelling; rejects anything that is not hex.
pub(crate) fn parse_hex(text: &str) -> std::result::Result<Vec<u8>, String> {
    let digits: String = text
        .split(|c: char| c.is_whitespace() || matches!(c, ':' | ',' | '-'))
        .map(|t| t.trim_start_matches("0x").trim_start_matches("0X"))
        .collect();
    if digits.is_empty() {
        return Err("no hex octets given".into());
    }
    if let Some(bad) = digits.chars().find(|c| !c.is_ascii_hexdigit()) {
        return Err(format!("{bad:?} is not a hex digit"));
    }
    if !digits.len().is_multiple_of(2) {
        return Err(format!("odd number of hex digits ({})", digits.len()));
    }
    (0..digits.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(digits.get(i..i + 2).unwrap_or_default(), 16)
                .map_err(|e| e.to_string())
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::parse_hex;

    #[test]
    fn every_common_hex_dump_spelling_parses_to_the_same_octets() {
        let want = vec![0x81, 0x0a, 0x00, 0x11];
        for text in [
            "810a0011",
            "81 0a 00 11",
            "81:0a:00:11",
            "0x81, 0x0a, 0x00, 0x11",
            "81-0A-00-11\n",
        ] {
            assert_eq!(parse_hex(text).unwrap(), want, "{text:?}");
        }
    }

    #[test]
    fn odd_or_non_hex_input_is_refused_rather_than_guessed() {
        assert!(parse_hex("810").is_err());
        assert!(parse_hex("81 zz").is_err());
        assert!(parse_hex("  ").is_err());
    }
}
