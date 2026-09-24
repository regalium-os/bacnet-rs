// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! Reading the dependency graph cargo has already resolved.
//!
//! The gates read the graph rather than our own manifests, because a banned
//! crate usually arrives transitively. No amount of reading what we wrote
//! would reveal it; asking cargo what it actually resolved does.

use serde_json::Value;
use std::path::Path;
use std::process::Command;

/// A workspace crate and what it depends on.
pub struct Member {
    /// The crate name.
    pub name: String,
    /// Workspace siblings it depends on, any kind.
    pub workspace_deps: Vec<String>,
    /// Every dependency it names directly, including build and dev.
    pub direct_deps: Vec<Dependency>,
}

/// One declared dependency.
pub struct Dependency {
    /// The crate depended on.
    pub name: String,
    /// `normal`, `build` or `dev`.
    pub kind: String,
}

/// The workspace crates and their declared dependencies.
///
/// # Errors
///
/// When cargo cannot be run, or answers with something unparseable.
pub fn workspace_members(root: &Path) -> Result<Vec<Member>, String> {
    let json = run_cargo(root, &["metadata", "--no-deps", "--format-version", "1"])?;
    let packages = packages_of(&json)?;
    let names: Vec<String> = packages.iter().filter_map(|p| string(p, "name")).collect();

    Ok(packages
        .iter()
        .filter_map(|package| {
            let name = string(package, "name")?;
            let direct_deps: Vec<Dependency> = package
                .get("dependencies")
                .and_then(Value::as_array)?
                .iter()
                .filter_map(|dep| {
                    Some(Dependency {
                        name: string(dep, "name")?,
                        kind: string(dep, "kind").unwrap_or_else(|| "normal".to_owned()),
                    })
                })
                .collect();
            let workspace_deps = direct_deps
                .iter()
                .filter(|dep| names.contains(&dep.name))
                .map(|dep| dep.name.clone())
                .collect();
            Some(Member {
                name,
                workspace_deps,
                direct_deps,
            })
        })
        .collect())
}

/// A crate in the resolved graph.
pub struct Resolved {
    /// The crate name.
    pub name: String,
    /// The native library it links, when it declares one.
    ///
    /// This is cargo's own answer to "does building this need a C toolchain",
    /// and it is why the gate does not guess from the name. `js-sys` ends in
    /// `-sys` and binds JavaScript; `libc` does not and binds a C library.
    pub links: Option<String>,
}

/// Every crate in the fully resolved graph, transitive dependencies included.
///
/// # Errors
///
/// When cargo cannot be run, or answers with something unparseable.
pub fn resolved_crates(root: &Path) -> Result<Vec<Resolved>, String> {
    let json = run_cargo(root, &["metadata", "--format-version", "1"])?;
    let packages = packages_of(&json)?;
    Ok(packages
        .iter()
        .filter_map(|p| {
            Some(Resolved {
                name: string(p, "name")?,
                links: string(p, "links"),
            })
        })
        .collect())
}

/// Runs cargo and parses its JSON, reporting its own words on failure.
fn run_cargo(root: &Path, args: &[&str]) -> Result<Value, String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let output = Command::new(cargo)
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|err| format!("could not run cargo {}: {err}", args.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "cargo {} failed:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|err| format!("cargo {} produced unparseable JSON: {err}", args.join(" ")))
}

/// The packages array, or cargo's answer was not what it claims to produce.
fn packages_of(json: &Value) -> Result<&Vec<Value>, String> {
    json.get("packages")
        .and_then(Value::as_array)
        .ok_or_else(|| "cargo metadata produced no packages array".to_owned())
}

/// A string field, or `None` when it is absent or not a string.
fn string(value: &Value, field: &str) -> Option<String> {
    value.get(field).and_then(Value::as_str).map(str::to_owned)
}
