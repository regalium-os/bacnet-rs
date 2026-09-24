// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Imports only ever point inward.

use crate::metadata;
use crate::scan::Findings;
use std::path::Path;

/// The hexagon, stated once.
///
/// Each entry is a crate, the workspace siblings it may depend on, and whether
/// it is pure. `regalium-bacnet-telemetry` is importable everywhere and so is
/// never listed: the seam is what every layer records through, and a rule
/// forbidding it would only be worked around.
///
/// This table is the authority. Change it here, deliberately, and say why in
/// the commit — never by routing an import around it.
///
/// One layer per line, against rustfmt's preference. The value of this
/// declaration is that it reads as a table: a reader checking whether the
/// runtime may reach a datalink scans one column. Expanded to five lines per
/// entry it is the same information nobody can see at once.
#[rustfmt::skip]
pub const LAYERS: &[Layer] = &[
    Layer { krate: "regalium-bacnet-telemetry", allowed: &[], pure: true },
    Layer { krate: "regalium-bacnet-core", allowed: &[], pure: true },
    Layer { krate: "regalium-bacnet-ip", allowed: &[CORE], pure: false },
    Layer { krate: "regalium-bacnet-mstp", allowed: &[CORE], pure: false },
    Layer { krate: "regalium-bacnet-sc", allowed: &[CORE], pure: false },
    Layer { krate: "regalium-bacnet-ipv6", allowed: &[CORE], pure: false },
    Layer { krate: "regalium-bacnet-schema", allowed: &[], pure: false },
    Layer { krate: "regalium-bacnet-telemetry-otel", allowed: &[], pure: false },
    Layer { krate: "regalium-bacnet-runtime", allowed: DATALINKS, pure: false },
    Layer { krate: "regalium-bacnet", allowed: EVERYTHING, pure: false },
];

const CORE: &str = "regalium-bacnet-core";

/// What the runtime may reach: every datalink, the core, and the schema.
const DATALINKS: &[&str] = &[
    CORE,
    "regalium-bacnet-ip",
    "regalium-bacnet-mstp",
    "regalium-bacnet-sc",
    "regalium-bacnet-ipv6",
    "regalium-bacnet-schema",
];

/// The facade re-exports, so it reaches everything.
const EVERYTHING: &[&str] = &[
    CORE,
    "regalium-bacnet-ip",
    "regalium-bacnet-mstp",
    "regalium-bacnet-sc",
    "regalium-bacnet-ipv6",
    "regalium-bacnet-schema",
    "regalium-bacnet-runtime",
    "regalium-bacnet-telemetry-otel",
];

/// One crate's position in the hexagon.
pub struct Layer {
    /// The crate this rule governs.
    pub krate: &'static str,
    /// Workspace siblings it may depend on, besides the telemetry seam.
    pub allowed: &'static [&'static str],
    /// Whether it must perform no I/O and read no clock.
    pub pure: bool,
}

/// The seam every layer may reach.
pub const SEAM: &str = "regalium-bacnet-telemetry";

/// Checks that every declared layer exists and depends only inward.
///
/// # Errors
///
/// When a layer is missing, an undeclared crate appears in the workspace, or a
/// dependency inverts the hexagon.
pub fn run(root: &Path) -> Result<String, String> {
    let members = metadata::workspace_members(root)?;
    let mut findings = Findings::default();

    for layer in LAYERS {
        findings.checked();
        let Some(member) = members.iter().find(|m| m.name == layer.krate) else {
            findings.violation(format!(
                "{} is declared in the layer table but absent from the workspace; a \
                 table describing a tree that is not there checks nothing",
                layer.krate
            ));
            continue;
        };
        for dep in &member.workspace_deps {
            if dep == SEAM || layer.allowed.contains(&dep.as_str()) {
                continue;
            }
            findings.violation(format!(
                "{} depends on {dep}, which it may not: a dependency in this direction \
                 inverts the hexagon. Permitted here: {:?} (plus {SEAM}, always)",
                layer.krate, layer.allowed
            ));
        }
    }

    for member in &members {
        if member.name == "xtask" || LAYERS.iter().any(|l| l.krate == member.name) {
            continue;
        }
        findings.violation(format!(
            "{} is in the workspace but not in the layer table; add it there with the \
             imports it may make, rather than leaving it ungoverned",
            member.name
        ));
    }

    findings.verdict("arch")
}
