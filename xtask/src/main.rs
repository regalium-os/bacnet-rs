// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! `cargo xtask <gate>` — the conformance gates.
//!
//! Run them all with `just gates`. Each prints what it examined, because a
//! gate that reports success without saying how much it looked at is a gate
//! nobody can tell apart from one that found nothing to look at.

use std::process::ExitCode;
use xtask::{GATES, scan};

fn main() -> ExitCode {
    let root = match scan::repo_root() {
        Ok(root) => root,
        Err(message) => {
            eprintln!("error: {message}");
            return ExitCode::FAILURE;
        }
    };

    let requested: Vec<String> = std::env::args().skip(1).collect();
    let wanted: Vec<&str> = match requested.first().map(String::as_str) {
        None | Some("all") => GATES.iter().map(|(name, _)| *name).collect(),
        Some(name) => vec![name],
    };

    let mut failed = false;
    for name in wanted {
        let Some((_, gate)) = GATES.iter().find(|(gate, _)| *gate == name) else {
            eprintln!("error: no gate named {name}; try one of {}", names());
            return ExitCode::FAILURE;
        };
        match gate(&root) {
            Ok(summary) => println!("  ok   {summary}"),
            Err(message) => {
                eprintln!("  FAIL {message}");
                failed = true;
            }
        }
    }

    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// The gate names, for an error message that tells the reader what to type.
fn names() -> String {
    GATES
        .iter()
        .map(|(name, _)| *name)
        .collect::<Vec<_>>()
        .join(", ")
}
