// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

//! The resolved dependency graph, walked edge by edge.
//!
//! `metadata` answers "which crates are in the build"; this answers "which
//! crates does *this* member pull in". The difference matters as soon as a
//! rule is scoped to a layer: native code beneath the telemetry adapter is a
//! decision, the same native code beneath the core is a defect, and only the
//! edges tell the two apart.

use crate::features::activated;
use crate::metadata;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};
use std::path::Path;

/// One crate in the resolved graph.
pub struct Node {
    /// The crate name.
    pub name: String,
    /// The native library it links, when it declares one.
    pub links: Option<String>,
    /// Names of the crates it build-depends on.
    pub build_deps: Vec<String>,
    /// Ids of what it depends on for normal and build use, never dev.
    deps: Vec<String>,
}

/// The whole graph, and which nodes are workspace members.
pub struct Graph {
    nodes: HashMap<String, Node>,
    members: Vec<String>,
}

impl Graph {
    /// Asks cargo for the graph as resolved for one target triple.
    ///
    /// Per target, because a dependency is often platform-specific: a crate
    /// that compiles C only when building for Haiku compiles nothing here,
    /// and a gate that cannot tell the difference cries wolf until ignored.
    ///
    /// # Errors
    ///
    /// When cargo cannot be run, or answers with something unparseable.
    pub fn resolve(root: &Path, target: &str) -> Result<Self, String> {
        let args = [
            "metadata",
            "--format-version",
            "1",
            "--filter-platform",
            target,
        ];
        let json = metadata::run_cargo(root, &args)?;
        let packages = json
            .get("packages")
            .and_then(Value::as_array)
            .ok_or("cargo metadata produced no packages array")?;
        let resolved = json
            .pointer("/resolve/nodes")
            .and_then(Value::as_array)
            .ok_or("cargo metadata produced no resolve graph")?;
        let names: HashMap<&str, &Value> = packages
            .iter()
            .filter_map(|p| Some((p.get("id")?.as_str()?, p)))
            .collect();
        let mut nodes = HashMap::new();
        for node in resolved {
            let Some(id) = node.get("id").and_then(Value::as_str) else {
                continue;
            };
            let package = names.get(id).copied();
            let text = |field| package.and_then(|p| p.get(field)).and_then(Value::as_str);
            let enabled: Vec<&str> = node
                .get("features")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            let mut deps = Vec::new();
            let mut build_deps = Vec::new();
            for dep in node
                .get("deps")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let kinds = dep.get("dep_kinds").and_then(Value::as_array);
                let kind_of = |k: &Value| k.get("kind").and_then(Value::as_str).map(str::to_owned);
                let kinds: Vec<Option<String>> = kinds.into_iter().flatten().map(kind_of).collect();
                let Some(pkg) = dep.get("pkg").and_then(Value::as_str) else {
                    continue;
                };
                let extern_name = dep.get("name").and_then(Value::as_str).unwrap_or_default();
                if !activated(package, &enabled, extern_name) {
                    continue;
                }
                if kinds.iter().any(|k| k.as_deref() != Some("dev")) {
                    deps.push(pkg.to_owned());
                }
                if kinds.iter().any(|k| k.as_deref() == Some("build"))
                    && let Some(name) = dep.get("name").and_then(Value::as_str)
                {
                    build_deps.push(name.replace('_', "-"));
                }
            }
            let name = text("name").unwrap_or(id).to_owned();
            let links = text("links").map(str::to_owned);
            nodes.insert(
                id.to_owned(),
                Node {
                    name,
                    links,
                    build_deps,
                    deps,
                },
            );
        }
        let members = json
            .get("workspace_members")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|m| m.as_str().map(str::to_owned))
            .collect();
        Ok(Self { nodes, members })
    }

    /// Every workspace member's name.
    #[must_use]
    pub fn member_names(&self) -> Vec<String> {
        self.members
            .iter()
            .filter_map(|id| self.nodes.get(id))
            .map(|n| n.name.clone())
            .collect()
    }

    /// Everything `member` pulls in for normal and build use, itself
    /// excluded, sorted by name so a report reads the same everywhere.
    #[must_use]
    pub fn closure(&self, member: &str) -> Vec<&Node> {
        let Some(start) = self
            .members
            .iter()
            .find(|id| self.nodes.get(*id).is_some_and(|n| n.name == member))
        else {
            return Vec::new();
        };
        let mut seen = BTreeSet::new();
        let mut stack = vec![start.clone()];
        while let Some(id) = stack.pop() {
            if !seen.insert(id.clone()) {
                continue;
            }
            if let Some(node) = self.nodes.get(&id) {
                stack.extend(node.deps.iter().cloned());
            }
        }
        seen.remove(start);
        let mut found: Vec<&Node> = seen.iter().filter_map(|id| self.nodes.get(id)).collect();
        found.sort_by(|a, b| a.name.cmp(&b.name));
        found
    }
}
