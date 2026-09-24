// Copyright 2026 RegaliumOS™.
// SPDX-License-Identifier: Apache-2.0

// A test asserts by panicking, so the workspace's never-panic lints are not
// the rule here: an expect that cannot fire is how a test states what it
// requires, and one that fires is the test doing its job.
#![allow(clippy::expect_used, clippy::panic)]

//! The size and licence gates, exercised against trees that are wrong.

mod common;

use common::{NOTICE, Tree};
use xtask::{licence, size};

#[test]
fn size_rejects_a_file_over_the_limit() {
    let tree = Tree::new("size-over");
    let body = "// a line\n".repeat(size::MAX_LINES);
    tree.file("crates/a/src/lib.rs", &format!("{NOTICE}{body}"));

    let error = size::run(tree.root()).expect_err("a file over the limit must fail the gate");
    assert!(error.contains("over the 200-line limit"), "{error}");
}

#[test]
fn size_accepts_a_file_exactly_at_the_limit() {
    let tree = Tree::new("size-exact");
    let body = "// a line\n".repeat(size::MAX_LINES - 3);
    tree.file("crates/a/src/lib.rs", &format!("{NOTICE}{body}"));

    size::run(tree.root()).expect("exactly 200 lines is within the limit, not over it");
}

#[test]
fn size_checks_documentation_too() {
    let tree = Tree::new("size-markdown");
    tree.file("docs/long.md", &"a line\n".repeat(size::MAX_LINES + 1));

    let error = size::run(tree.root()).expect_err("markdown is subject to the limit");
    assert!(error.contains("long.md"), "{error}");
}

#[test]
fn size_refuses_to_pass_on_an_empty_tree() {
    let tree = Tree::new("size-empty");

    let error = size::run(tree.root()).expect_err("a gate that examined nothing has not passed");
    assert!(error.contains("checking nothing"), "{error}");
}

#[test]
fn licence_rejects_a_file_without_the_notice() {
    let tree = Tree::new("licence-missing");
    tree.file("crates/a/src/lib.rs", "pub fn decode() {}\n");

    let error = licence::run(tree.root()).expect_err("an unlicensed file must fail the gate");
    assert!(error.contains("missing the licence notice"), "{error}");
}

#[test]
fn licence_rejects_a_notice_below_the_header() {
    let tree = Tree::new("licence-buried");
    tree.file("crates/a/src/lib.rs", &format!("\n\n\n\n\n{NOTICE}"));

    let error = licence::run(tree.root()).expect_err("a buried notice is not a notice");
    assert!(error.contains("first 4 lines"), "{error}");
}

#[test]
fn licence_rejects_the_wrong_comment_syntax() {
    let tree = Tree::new("licence-syntax");
    tree.file("config/a.yaml", &NOTICE.replace("//", "--"));

    let error = licence::run(tree.root()).expect_err("a YAML notice is written with #");
    assert!(error.contains("a.yaml"), "{error}");
}

#[test]
fn licence_refuses_to_pass_on_an_empty_tree() {
    let tree = Tree::new("licence-empty");

    let error = licence::run(tree.root()).expect_err("a gate that examined nothing has not passed");
    assert!(error.contains("checking nothing"), "{error}");
}

#[test]
fn generated_trees_are_exempt_from_both() {
    let tree = Tree::new("generated");
    tree.file("crates/a/src/lib.rs", NOTICE);
    tree.file(
        "protobuf/generated/huge.rs",
        &"// generated\n".repeat(size::MAX_LINES + 1),
    );

    size::run(tree.root()).expect("generated output is rewritten wholesale, not authored here");
    licence::run(tree.root()).expect("generated output carries no notice of ours");
}
