// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Which optional dependencies an enabled feature set actually builds.

use serde_json::Value;

/// Whether `package`'s dependency known in code as `extern_name` is built.
///
/// cargo's resolve graph lists an optional dependency whenever anything
/// could enable it — a weak `dep?/feature` is enough — even when no enabled
/// feature does. Counting those would charge a crate with code it never
/// compiles, so an optional dependency counts only when it is enabled: by a
/// feature listing `dep:name`, or by its own implicit feature.
pub(crate) fn activated(package: Option<&Value>, enabled: &[&str], extern_name: &str) -> bool {
    let Some(package) = package else { return true };
    let declared: Vec<&Value> = package
        .get("dependencies")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|d| code_name(d).is_some_and(|n| n.replace('-', "_") == extern_name))
        .collect();
    if declared
        .iter()
        .any(|d| d.get("optional").and_then(Value::as_bool) != Some(true))
        || declared.is_empty()
    {
        return true;
    }
    let names: Vec<String> = declared
        .iter()
        .filter_map(|d| code_name(d))
        .map(str::to_owned)
        .collect();
    let features = package.get("features");
    enabled.iter().any(|f| {
        names.iter().any(|n| n == f)
            || features
                .and_then(|all| all.get(*f))
                .and_then(Value::as_array)
                .is_some_and(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .any(|i| names.iter().any(|n| i == format!("dep:{n}")))
                })
    })
}

/// The name a dependency goes by in code: its `rename`, else its package
/// name. cargo writes `"rename": null` when there is none, so a present key
/// is not enough.
fn code_name(dep: &Value) -> Option<&str> {
    dep.get("rename")
        .and_then(Value::as_str)
        .or_else(|| dep.get("name").and_then(Value::as_str))
}
