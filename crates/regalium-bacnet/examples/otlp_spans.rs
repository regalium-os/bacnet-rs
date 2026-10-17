// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Sends the tag layer's spans to an OTLP collector: every record of the
//! fixture corpus decoded through the traced codec, so a collector shows a
//! `bacnet.tag.decode` span per record — the malformed ones failed.
//!
//! ```sh
//! cargo run -p regalium-bacnet --features otel --example otlp_spans -- localhost:6009 [run-label]
//! ```

use regalium_bacnet::{Otel, tag};

/// The corpus the phase gate walks, compiled in.
const CORPORA: [&str; 5] = [
    include_str!("../../regalium-bacnet-core/fixtures/tag/application.hex"),
    include_str!("../../regalium-bacnet-core/fixtures/tag/captured.hex"),
    include_str!("../../regalium-bacnet-core/fixtures/tag/context.hex"),
    include_str!("../../regalium-bacnet-core/fixtures/tag/malformed.hex"),
    include_str!("../../regalium-bacnet-core/fixtures/tag/quirks.hex"),
];

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let target = args.next().unwrap_or_else(|| "localhost:4317".into());
    let run = args.next().unwrap_or_else(|| "otlp-spans".into());
    let (host, port) = target
        .rsplit_once(':')
        .and_then(|(h, p)| Some((h.to_owned(), p.parse().ok()?)))
        .ok_or_else(|| format!("{target:?}: expected HOST:PORT"))?;
    let otel = Otel::builder("bacnet-otlp-spans", env!("CARGO_PKG_VERSION"))
        .otlp(&host, port)
        .label("run", &run)
        .build()?;
    let (mut ok, mut failed) = (0, 0);
    for line in CORPORA.iter().flat_map(|c| c.lines()) {
        let Some((octets, _)) = line.split_once(" => ").filter(|_| !line.starts_with('#')) else {
            continue;
        };
        let octets: Vec<u8> = octets
            .split_whitespace()
            .filter_map(|b| u8::from_str_radix(b, 16).ok())
            .collect();
        match tag::decode_all_traced(&octets, &otel) {
            Ok(_) => ok += 1,
            Err(_) => failed += 1,
        }
    }
    otel.flush()?;
    println!(
        "sent {} spans to {target} (run={run}): {ok} decoded, {failed} failed",
        ok + failed
    );
    Ok(())
}
