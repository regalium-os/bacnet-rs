// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Every key binding, as the help overlay lists it. Kept beside the
//! bindings so the two change together.

pub(crate) const HELP: &[(&str, &str)] = &[
    ("1 2 3 4", "Browse / Packets / Log / Settings tab"),
    ("s", "Settings: target, port, scan ranges, BBMD, timing"),
    ("Tab, ← →", "move focus: devices, objects, properties"),
    ("↑ ↓  j k", "move selection"),
    ("PgUp PgDn", "move ten rows"),
    ("g G  Home End", "first / last row"),
    ("Enter", "reload the selected device or object"),
    ("r", "re-run discovery (Who-Is)"),
    (
        "w",
        "start / stop following live values (COV, else polling)",
    ),
    ("+ -", "poll faster / slower (points without COV)"),
    ("f", "packets: follow the newest"),
    ("c", "packets / log: clear"),
    (
        "e",
        "write the selected property, or an object's present-value",
    ),
    ("n", "relinquish: write null at a priority"),
    ("a  x", "create an object / delete the selected object"),
    ("?", "this help"),
    ("q  Ctrl-C", "quit"),
];
